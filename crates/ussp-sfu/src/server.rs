//! SFU server implementation.

use crate::config::SfuConfig;
use crate::error::{Error, Result};
use crate::forwarder::Forwarder;
use crate::participant::{Participant, ParticipantId, ParticipantRole, ParticipantState};
use crate::room::{Room, RoomId, RoomState, RoomStats, RoomSummary};
use crate::stats::{SfuStats, SfuStatsSnapshot};
use bytes::Bytes;
use dashmap::DashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tokio::time::interval;
use tracing::{debug, error, info, warn};
use ussp::protocol::{Packet, PacketType, Payload};

/// Maximum UDP packet size.
const MAX_PACKET_SIZE: usize = 65535;

/// SFU server.
pub struct SfuServer {
    /// Server configuration.
    config: SfuConfig,
    /// UDP socket for media.
    socket: Arc<UdpSocket>,
    /// Active rooms (wrapped in Arc for sharing with background tasks).
    rooms: Arc<DashMap<RoomId, Arc<Room>>>,
    /// Participant to room mapping.
    participant_rooms: DashMap<SocketAddr, (RoomId, ParticipantId)>,
    /// Packet forwarder.
    forwarder: Forwarder,
    /// Server statistics.
    stats: Arc<SfuStats>,
    /// Server running state.
    running: RwLock<bool>,
}

impl SfuServer {
    /// Create a new SFU server.
    pub async fn new(config: SfuConfig) -> Result<Self> {
        let socket = UdpSocket::bind(&config.media_bind_addr).await?;

        info!(
            server_id = %config.server_id,
            bind_addr = %config.media_bind_addr,
            "SFU server created"
        );

        Ok(Self {
            config,
            socket: Arc::new(socket),
            rooms: Arc::new(DashMap::new()),
            participant_rooms: DashMap::new(),
            forwarder: Forwarder::new(),
            stats: Arc::new(SfuStats::new()),
            running: RwLock::new(false),
        })
    }

    /// Get server ID.
    pub fn server_id(&self) -> &str {
        &self.config.server_id
    }

    /// Get the socket (for external use).
    pub fn socket(&self) -> Arc<UdpSocket> {
        Arc::clone(&self.socket)
    }

    /// Create a new room.
    pub fn create_room(&self, room_id: RoomId) -> Result<Arc<Room>> {
        if self.rooms.len() >= self.config.max_rooms {
            return Err(Error::Config(format!(
                "Maximum rooms reached: {}",
                self.config.max_rooms
            )));
        }

        if self.rooms.contains_key(&room_id) {
            return Err(Error::RoomAlreadyExists(room_id));
        }

        let room = Arc::new(Room::new(
            room_id.clone(),
            self.config.room.clone(),
            self.config.max_participants_per_room,
        ));

        self.rooms.insert(room_id.clone(), Arc::clone(&room));
        self.stats.record_room_created();

        info!(room_id = %room_id, "Room created");
        Ok(room)
    }

    /// Get a room by ID.
    pub fn get_room(&self, room_id: &RoomId) -> Option<Arc<Room>> {
        self.rooms.get(room_id).map(|r| Arc::clone(&r))
    }

    /// Get or create a room.
    pub fn get_or_create_room(&self, room_id: RoomId) -> Result<Arc<Room>> {
        if let Some(room) = self.get_room(&room_id) {
            Ok(room)
        } else {
            self.create_room(room_id)
        }
    }

    /// Close a room.
    pub async fn close_room(&self, room_id: &RoomId) -> Result<()> {
        let room = self
            .rooms
            .remove(room_id)
            .map(|(_, r)| r)
            .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

        room.set_state(RoomState::Closed).await;
        self.stats.record_room_closed();

        // Remove all participant mappings for this room
        self.participant_rooms.retain(|_, (rid, _)| rid != room_id);

        info!(room_id = %room_id, "Room closed");
        Ok(())
    }

    /// Add a participant to a room.
    pub async fn add_participant(
        &self,
        room_id: &RoomId,
        participant_id: ParticipantId,
        addr: SocketAddr,
        role: ParticipantRole,
        session_id: u32,
    ) -> Result<()> {
        let room = self
            .get_room(room_id)
            .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

        let participant = Arc::new(Participant::new(
            participant_id.clone(),
            addr,
            role,
            session_id,
            Arc::clone(&self.socket),
        ));

        participant.set_state(ParticipantState::Active).await;
        room.add_participant(participant).await?;

        self.participant_rooms
            .insert(addr, (room_id.clone(), participant_id));
        self.stats.record_participant_joined();

        Ok(())
    }

    /// Remove a participant.
    pub async fn remove_participant(&self, addr: &SocketAddr) -> Result<()> {
        let (room_id, participant_id) = self
            .participant_rooms
            .remove(addr)
            .map(|(_, v)| v)
            .ok_or_else(|| Error::ParticipantNotFound(format!("{}", addr)))?;

        if let Some(room) = self.get_room(&room_id) {
            room.remove_participant(&participant_id).await?;

            // Check if failover is needed
            if room.state().await == RoomState::Failover {
                self.trigger_failover(&room_id).await?;
            }
        }

        self.stats.record_participant_left();
        Ok(())
    }

    /// Trigger failover for a room.
    pub async fn trigger_failover(&self, room_id: &RoomId) -> Result<()> {
        let room = self
            .get_room(room_id)
            .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

        if !self.config.failover.enabled {
            warn!(room_id = %room_id, "Failover disabled, room will be paused");
            room.set_state(RoomState::Paused).await;
            return Ok(());
        }

        self.stats.record_failover_triggered();
        info!(room_id = %room_id, "Triggering failover");

        // Wait for promotion delay
        tokio::time::sleep(self.config.failover.promotion_delay).await;

        // Promote backup if available
        match room.promote_backup().await {
            Ok(()) => {
                self.stats.record_failover_succeeded();
                info!(room_id = %room_id, "Failover succeeded");
            }
            Err(e) => {
                warn!(room_id = %room_id, error = %e, "Failover failed");
                room.set_state(RoomState::Paused).await;
            }
        }

        Ok(())
    }

    /// Handle an incoming packet.
    async fn handle_packet(&self, data: Bytes, addr: SocketAddr) -> Result<()> {
        // Update stats
        self.stats.record_received(1, data.len() as u64);

        // Find participant's room
        let (room_id, participant_id) = match self.participant_rooms.get(&addr) {
            Some(entry) => entry.value().clone(),
            None => {
                // Unknown participant - try to handle session init
                return self.handle_new_participant(data, addr).await;
            }
        };

        // Get room
        let room = match self.get_room(&room_id) {
            Some(r) => r,
            None => return Err(Error::RoomNotFound(room_id)),
        };

        // Get participant and update activity
        if let Some(participant) = room.get_participant(&participant_id) {
            participant.touch().await;
            participant.record_received(data.len());
        }

        // Forward packet if from sender
        if let Some(participant) = room.get_participant(&participant_id) {
            if participant.is_primary_sender().await {
                self.forwarder
                    .forward(&room, &data, Some(&participant_id))
                    .await?;
                self.stats.record_forwarded(1, data.len() as u64);
            }
        }

        Ok(())
    }

    /// Handle a packet from an unknown participant.
    async fn handle_new_participant(&self, data: Bytes, addr: SocketAddr) -> Result<()> {
        // Parse packet
        let packet = Packet::decode(data)?;

        // Only handle session init from unknown participants
        if packet.header.packet_type != PacketType::SessionInit {
            debug!(addr = %addr, "Ignoring packet from unknown participant");
            return Ok(());
        }

        // Extract session info
        if let Payload::SessionInit(init) = packet.payload {
            info!(
                addr = %addr,
                session_id = init.session_id,
                "New participant session init"
            );

            // For now, auto-create room and add as receiver
            // In production, this would go through the control plane
            let room_id = format!("auto-{}", init.session_id);
            let participant_id = format!("participant-{}", addr);

            let _room = self.get_or_create_room(room_id.clone())?;

            // Add as receiver by default
            self.add_participant(
                &room_id,
                participant_id,
                addr,
                ParticipantRole::Receiver,
                init.session_id,
            )
            .await?;
        }

        Ok(())
    }

    /// Run the SFU server.
    pub async fn run(&self) -> Result<()> {
        *self.running.write().await = true;
        info!(server_id = %self.config.server_id, "SFU server starting");

        let mut buf = vec![0u8; MAX_PACKET_SIZE];

        // Health check task
        let rooms = Arc::clone(&self.rooms);
        let health_interval = self.config.room.heartbeat_interval;
        tokio::spawn(async move {
            let mut ticker = interval(health_interval);
            loop {
                ticker.tick().await;
                for entry in rooms.iter() {
                    entry.value().check_health().await;
                }
            }
        });

        // Main receive loop
        loop {
            if !*self.running.read().await {
                break;
            }

            match self.socket.recv_from(&mut buf).await {
                Ok((len, addr)) => {
                    let data = Bytes::copy_from_slice(&buf[..len]);
                    if let Err(e) = self.handle_packet(data, addr).await {
                        debug!(addr = %addr, error = %e, "Error handling packet");
                    }
                }
                Err(e) => {
                    error!(error = %e, "Socket receive error");
                }
            }
        }

        info!(server_id = %self.config.server_id, "SFU server stopped");
        Ok(())
    }

    /// Stop the server.
    pub async fn stop(&self) {
        *self.running.write().await = false;
    }

    /// Get server statistics.
    pub fn stats(&self) -> SfuStatsSnapshot {
        self.stats.snapshot()
    }

    /// Get all room summaries.
    pub async fn room_summaries(&self) -> Vec<RoomSummary> {
        let mut summaries = Vec::new();
        for entry in self.rooms.iter() {
            let room = entry.value();
            summaries.push(RoomSummary {
                id: room.id.clone(),
                state: room.state().await,
                participant_count: room.participant_count(),
            });
        }
        summaries
    }

    /// Get room statistics.
    pub async fn room_stats(&self, room_id: &RoomId) -> Option<RoomStats> {
        self.get_room(room_id).map(|r| {
            tokio::runtime::Handle::current().block_on(async { r.stats().await })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_server_creation() {
        let config = SfuConfig::test();
        let server = SfuServer::new(config).await;
        assert!(server.is_ok());
    }

    #[tokio::test]
    async fn test_room_creation() {
        let config = SfuConfig::test();
        let server = SfuServer::new(config).await.unwrap();

        let room = server.create_room("test-room".to_string());
        assert!(room.is_ok());

        // Duplicate should fail
        let room2 = server.create_room("test-room".to_string());
        assert!(room2.is_err());
    }
}

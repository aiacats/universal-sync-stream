//! Room (session) management.

use crate::config::RoomConfig;
use crate::error::{Error, Result};
use crate::participant::{
    Participant, ParticipantId, ParticipantRole, ParticipantState, ParticipantSummary,
};
use bytes::Bytes;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Room ID.
pub type RoomId = String;

/// Room state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoomState {
    /// Room is active and streaming.
    Active,
    /// Room is paused (no active sender).
    Paused,
    /// Failover in progress.
    Failover,
    /// Room is closing.
    Closing,
    /// Room is closed.
    Closed,
}

/// A room containing participants.
pub struct Room {
    /// Unique room ID.
    pub id: RoomId,
    /// Room configuration.
    config: RoomConfig,
    /// Room state.
    state: RwLock<RoomState>,
    /// Primary sender ID.
    primary_sender_id: RwLock<Option<ParticipantId>>,
    /// Backup sender ID.
    backup_sender_id: RwLock<Option<ParticipantId>>,
    /// All participants.
    participants: DashMap<ParticipantId, Arc<Participant>>,
    /// Creation time.
    created_at: Instant,
    /// Last activity time.
    last_activity: RwLock<Instant>,
    /// Total packets forwarded.
    packets_forwarded: AtomicU64,
    /// Total bytes forwarded.
    bytes_forwarded: AtomicU64,
    /// Maximum participants allowed.
    max_participants: usize,
}

impl Room {
    /// Create a new room.
    pub fn new(id: RoomId, config: RoomConfig, max_participants: usize) -> Self {
        Self {
            id,
            config,
            state: RwLock::new(RoomState::Paused),
            primary_sender_id: RwLock::new(None),
            backup_sender_id: RwLock::new(None),
            participants: DashMap::new(),
            created_at: Instant::now(),
            last_activity: RwLock::new(Instant::now()),
            packets_forwarded: AtomicU64::new(0),
            bytes_forwarded: AtomicU64::new(0),
            max_participants,
        }
    }

    /// Get room state.
    pub async fn state(&self) -> RoomState {
        *self.state.read().await
    }

    /// Set room state.
    pub async fn set_state(&self, state: RoomState) {
        *self.state.write().await = state;
    }

    /// Add a participant to the room.
    pub async fn add_participant(&self, participant: Arc<Participant>) -> Result<()> {
        // Check capacity
        if self.participants.len() >= self.max_participants {
            return Err(Error::MaxParticipantsReached(self.max_participants));
        }

        // Check for duplicate
        if self.participants.contains_key(&participant.id) {
            return Err(Error::ParticipantAlreadyExists);
        }

        let role = participant.role().await;

        // Handle sender roles
        match role {
            ParticipantRole::PrimarySender => {
                let mut primary = self.primary_sender_id.write().await;
                if primary.is_some() {
                    return Err(Error::MultipleSendersNotAllowed);
                }
                *primary = Some(participant.id.clone());
                self.set_state(RoomState::Active).await;
            }
            ParticipantRole::BackupSender => {
                if !self.config.allow_backup_sender {
                    return Err(Error::Config("Backup sender not allowed".to_string()));
                }
                let mut backup = self.backup_sender_id.write().await;
                *backup = Some(participant.id.clone());
            }
            ParticipantRole::Receiver => {}
        }

        self.participants.insert(participant.id.clone(), participant);
        self.touch().await;

        info!(room_id = %self.id, "Participant added");
        Ok(())
    }

    /// Remove a participant from the room.
    pub async fn remove_participant(&self, participant_id: &ParticipantId) -> Result<()> {
        let participant = self
            .participants
            .remove(participant_id)
            .map(|(_, p)| p)
            .ok_or_else(|| Error::ParticipantNotFound(participant_id.clone()))?;

        let role = participant.role().await;

        // Handle sender removal
        match role {
            ParticipantRole::PrimarySender => {
                let mut primary = self.primary_sender_id.write().await;
                *primary = None;

                // Trigger failover if backup exists
                if self.backup_sender_id.read().await.is_some() {
                    self.set_state(RoomState::Failover).await;
                } else {
                    self.set_state(RoomState::Paused).await;
                }
            }
            ParticipantRole::BackupSender => {
                let mut backup = self.backup_sender_id.write().await;
                *backup = None;
            }
            ParticipantRole::Receiver => {}
        }

        self.touch().await;
        info!(room_id = %self.id, participant_id = %participant_id, "Participant removed");
        Ok(())
    }

    /// Get a participant by ID.
    pub fn get_participant(&self, participant_id: &ParticipantId) -> Option<Arc<Participant>> {
        self.participants.get(participant_id).map(|p| p.clone())
    }

    /// Get the primary sender.
    pub async fn primary_sender(&self) -> Option<Arc<Participant>> {
        let primary_id = self.primary_sender_id.read().await.clone();
        primary_id.and_then(|id| self.get_participant(&id))
    }

    /// Get the backup sender.
    pub async fn backup_sender(&self) -> Option<Arc<Participant>> {
        let backup_id = self.backup_sender_id.read().await.clone();
        backup_id.and_then(|id| self.get_participant(&id))
    }

    /// Promote backup sender to primary.
    pub async fn promote_backup(&self) -> Result<()> {
        let backup_id = self.backup_sender_id.write().await.take();

        let backup_id = backup_id.ok_or_else(|| Error::NoSender(self.id.clone()))?;

        let participant = self
            .get_participant(&backup_id)
            .ok_or_else(|| Error::ParticipantNotFound(backup_id.clone()))?;

        // Promote to primary
        participant.set_role(ParticipantRole::PrimarySender).await;
        participant.set_state(ParticipantState::Active).await;

        // Update room state
        *self.primary_sender_id.write().await = Some(backup_id.clone());
        self.set_state(RoomState::Active).await;

        info!(room_id = %self.id, participant_id = %backup_id, "Backup promoted to primary");
        Ok(())
    }

    /// Forward packet to all receivers.
    pub async fn forward_to_receivers(&self, data: &Bytes, exclude: Option<&ParticipantId>) {
        for entry in self.participants.iter() {
            let participant = entry.value();

            // Skip excluded participant
            if let Some(excluded) = exclude {
                if &participant.id == excluded {
                    continue;
                }
            }

            // Skip senders
            if participant.is_sender().await {
                continue;
            }

            // Skip inactive participants
            if !participant.is_active().await {
                continue;
            }

            // Send packet
            if let Err(e) = participant.send(data).await {
                warn!(
                    room_id = %self.id,
                    participant_id = %participant.id,
                    error = %e,
                    "Failed to forward packet"
                );
            }
        }

        // Update stats
        self.packets_forwarded.fetch_add(1, Ordering::Relaxed);
        self.bytes_forwarded
            .fetch_add(data.len() as u64, Ordering::Relaxed);
    }

    /// Update last activity timestamp.
    pub async fn touch(&self) {
        *self.last_activity.write().await = Instant::now();
    }

    /// Get time since last activity.
    pub async fn idle_time(&self) -> std::time::Duration {
        self.last_activity.read().await.elapsed()
    }

    /// Get number of participants.
    pub fn participant_count(&self) -> usize {
        self.participants.len()
    }

    /// Get number of receivers.
    pub async fn receiver_count(&self) -> usize {
        let mut count = 0;
        for entry in self.participants.iter() {
            if entry.value().role().await == ParticipantRole::Receiver {
                count += 1;
            }
        }
        count
    }

    /// Get participant summaries.
    pub async fn participant_summaries(&self) -> Vec<ParticipantSummary> {
        let mut summaries = Vec::new();
        for entry in self.participants.iter() {
            let p = entry.value();
            summaries.push(ParticipantSummary {
                id: p.id.clone(),
                addr: p.addr,
                role: p.role().await,
                state: p.state().await,
                session_id: p.session_id,
            });
        }
        summaries
    }

    /// Get room statistics.
    pub async fn stats(&self) -> RoomStats {
        RoomStats {
            id: self.id.clone(),
            state: self.state().await,
            participant_count: self.participant_count(),
            receiver_count: self.receiver_count().await,
            has_primary_sender: self.primary_sender_id.read().await.is_some(),
            has_backup_sender: self.backup_sender_id.read().await.is_some(),
            packets_forwarded: self.packets_forwarded.load(Ordering::Relaxed),
            bytes_forwarded: self.bytes_forwarded.load(Ordering::Relaxed),
            uptime_secs: self.created_at.elapsed().as_secs(),
        }
    }

    /// Check health of all participants.
    pub async fn check_health(&self) {
        let timeout = self.config.heartbeat_timeout;

        for entry in self.participants.iter() {
            let participant = entry.value();
            let idle = participant.idle_time().await;

            if idle > timeout {
                if participant.state().await == ParticipantState::Active {
                    participant.set_state(ParticipantState::Suspected).await;
                    debug!(
                        room_id = %self.id,
                        participant_id = %participant.id,
                        idle_ms = idle.as_millis(),
                        "Participant suspected"
                    );
                } else if idle > timeout * 2 {
                    participant.set_state(ParticipantState::Disconnected).await;
                    warn!(
                        room_id = %self.id,
                        participant_id = %participant.id,
                        "Participant disconnected"
                    );
                }
            }
        }
    }
}

/// Room statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStats {
    /// Room ID.
    pub id: RoomId,
    /// Room state.
    pub state: RoomState,
    /// Total participants.
    pub participant_count: usize,
    /// Receiver count.
    pub receiver_count: usize,
    /// Has primary sender.
    pub has_primary_sender: bool,
    /// Has backup sender.
    pub has_backup_sender: bool,
    /// Packets forwarded.
    pub packets_forwarded: u64,
    /// Bytes forwarded.
    pub bytes_forwarded: u64,
    /// Room uptime in seconds.
    pub uptime_secs: u64,
}

/// Room summary (for API responses).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSummary {
    /// Room ID.
    pub id: RoomId,
    /// Room state.
    pub state: RoomState,
    /// Participant count.
    pub participant_count: usize,
}

//! Participant (sender/receiver) management.

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;

/// Participant ID.
pub type ParticipantId = String;

/// Participant role in a room.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantRole {
    /// Primary sender (active).
    PrimarySender,
    /// Backup sender (standby).
    BackupSender,
    /// Receiver (subscriber).
    Receiver,
}

/// Participant state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ParticipantState {
    /// Connecting.
    Connecting,
    /// Active and healthy.
    Active,
    /// Suspected unhealthy (heartbeat delayed).
    Suspected,
    /// Disconnected.
    Disconnected,
}

/// A participant in a room.
pub struct Participant {
    /// Unique participant ID.
    pub id: ParticipantId,
    /// Remote address.
    pub addr: SocketAddr,
    /// Role in the room.
    role: RwLock<ParticipantRole>,
    /// Current state.
    state: RwLock<ParticipantState>,
    /// USSP session ID.
    pub session_id: u32,
    /// Last activity time.
    last_activity: RwLock<Instant>,
    /// Packets sent to this participant.
    packets_sent: AtomicU64,
    /// Packets received from this participant.
    packets_received: AtomicU64,
    /// Bytes sent to this participant.
    bytes_sent: AtomicU64,
    /// Bytes received from this participant.
    bytes_received: AtomicU64,
    /// Socket for sending (shared).
    socket: Arc<UdpSocket>,
}

impl Participant {
    /// Create a new participant.
    pub fn new(
        id: ParticipantId,
        addr: SocketAddr,
        role: ParticipantRole,
        session_id: u32,
        socket: Arc<UdpSocket>,
    ) -> Self {
        Self {
            id,
            addr,
            role: RwLock::new(role),
            state: RwLock::new(ParticipantState::Connecting),
            session_id,
            last_activity: RwLock::new(Instant::now()),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            socket,
        }
    }

    /// Get the participant's role.
    pub async fn role(&self) -> ParticipantRole {
        *self.role.read().await
    }

    /// Set the participant's role.
    pub async fn set_role(&self, role: ParticipantRole) {
        *self.role.write().await = role;
    }

    /// Get the participant's state.
    pub async fn state(&self) -> ParticipantState {
        *self.state.read().await
    }

    /// Set the participant's state.
    pub async fn set_state(&self, state: ParticipantState) {
        *self.state.write().await = state;
    }

    /// Check if this participant is a sender (primary or backup).
    pub async fn is_sender(&self) -> bool {
        matches!(
            self.role().await,
            ParticipantRole::PrimarySender | ParticipantRole::BackupSender
        )
    }

    /// Check if this participant is the primary sender.
    pub async fn is_primary_sender(&self) -> bool {
        self.role().await == ParticipantRole::PrimarySender
    }

    /// Check if this participant is active.
    pub async fn is_active(&self) -> bool {
        self.state().await == ParticipantState::Active
    }

    /// Update last activity timestamp.
    pub async fn touch(&self) {
        *self.last_activity.write().await = Instant::now();
    }

    /// Get time since last activity.
    pub async fn idle_time(&self) -> std::time::Duration {
        self.last_activity.read().await.elapsed()
    }

    /// Record received packet.
    pub fn record_received(&self, bytes: usize) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Record sent packet.
    pub fn record_sent(&self, bytes: usize) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Send data to this participant.
    pub async fn send(&self, data: &Bytes) -> std::io::Result<usize> {
        let sent = self.socket.send_to(data, self.addr).await?;
        self.record_sent(sent);
        Ok(sent)
    }

    /// Get participant statistics.
    pub fn stats(&self) -> ParticipantStats {
        ParticipantStats {
            id: self.id.clone(),
            addr: self.addr,
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }
}

/// Participant statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantStats {
    /// Participant ID.
    pub id: ParticipantId,
    /// Remote address.
    pub addr: SocketAddr,
    /// Packets sent.
    pub packets_sent: u64,
    /// Packets received.
    pub packets_received: u64,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Bytes received.
    pub bytes_received: u64,
}

/// Summary of a participant (for API responses).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantSummary {
    /// Participant ID.
    pub id: ParticipantId,
    /// Remote address.
    pub addr: SocketAddr,
    /// Role.
    pub role: ParticipantRole,
    /// State.
    pub state: ParticipantState,
    /// Session ID.
    pub session_id: u32,
}

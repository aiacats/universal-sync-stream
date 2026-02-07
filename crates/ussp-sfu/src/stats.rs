//! SFU statistics and metrics.

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

/// SFU server statistics.
#[derive(Debug, Default)]
pub struct SfuStats {
    /// Total rooms created.
    pub rooms_created: AtomicU64,
    /// Total rooms closed.
    pub rooms_closed: AtomicU64,
    /// Total participants joined.
    pub participants_joined: AtomicU64,
    /// Total participants left.
    pub participants_left: AtomicU64,
    /// Total packets received.
    pub packets_received: AtomicU64,
    /// Total packets forwarded.
    pub packets_forwarded: AtomicU64,
    /// Total bytes received.
    pub bytes_received: AtomicU64,
    /// Total bytes forwarded.
    pub bytes_forwarded: AtomicU64,
    /// Total failovers triggered.
    pub failovers_triggered: AtomicU64,
    /// Total failovers succeeded.
    pub failovers_succeeded: AtomicU64,
    /// Server start time.
    start_time: Option<Instant>,
}

impl SfuStats {
    /// Create new stats.
    pub fn new() -> Self {
        Self {
            start_time: Some(Instant::now()),
            ..Default::default()
        }
    }

    /// Record a room created.
    pub fn record_room_created(&self) {
        self.rooms_created.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a room closed.
    pub fn record_room_closed(&self) {
        self.rooms_closed.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a participant joined.
    pub fn record_participant_joined(&self) {
        self.participants_joined.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a participant left.
    pub fn record_participant_left(&self) {
        self.participants_left.fetch_add(1, Ordering::Relaxed);
    }

    /// Record packets received.
    pub fn record_received(&self, packets: u64, bytes: u64) {
        self.packets_received.fetch_add(packets, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record packets forwarded.
    pub fn record_forwarded(&self, packets: u64, bytes: u64) {
        self.packets_forwarded.fetch_add(packets, Ordering::Relaxed);
        self.bytes_forwarded.fetch_add(bytes, Ordering::Relaxed);
    }

    /// Record a failover triggered.
    pub fn record_failover_triggered(&self) {
        self.failovers_triggered.fetch_add(1, Ordering::Relaxed);
    }

    /// Record a failover succeeded.
    pub fn record_failover_succeeded(&self) {
        self.failovers_succeeded.fetch_add(1, Ordering::Relaxed);
    }

    /// Get a snapshot of the stats.
    pub fn snapshot(&self) -> SfuStatsSnapshot {
        SfuStatsSnapshot {
            rooms_created: self.rooms_created.load(Ordering::Relaxed),
            rooms_closed: self.rooms_closed.load(Ordering::Relaxed),
            rooms_active: self.rooms_created.load(Ordering::Relaxed)
                .saturating_sub(self.rooms_closed.load(Ordering::Relaxed)),
            participants_joined: self.participants_joined.load(Ordering::Relaxed),
            participants_left: self.participants_left.load(Ordering::Relaxed),
            participants_active: self.participants_joined.load(Ordering::Relaxed)
                .saturating_sub(self.participants_left.load(Ordering::Relaxed)),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            packets_forwarded: self.packets_forwarded.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            bytes_forwarded: self.bytes_forwarded.load(Ordering::Relaxed),
            failovers_triggered: self.failovers_triggered.load(Ordering::Relaxed),
            failovers_succeeded: self.failovers_succeeded.load(Ordering::Relaxed),
            uptime_secs: self.start_time.map(|t| t.elapsed().as_secs()).unwrap_or(0),
        }
    }
}

/// Snapshot of SFU statistics.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SfuStatsSnapshot {
    /// Total rooms created.
    pub rooms_created: u64,
    /// Total rooms closed.
    pub rooms_closed: u64,
    /// Active rooms.
    pub rooms_active: u64,
    /// Total participants joined.
    pub participants_joined: u64,
    /// Total participants left.
    pub participants_left: u64,
    /// Active participants.
    pub participants_active: u64,
    /// Total packets received.
    pub packets_received: u64,
    /// Total packets forwarded.
    pub packets_forwarded: u64,
    /// Total bytes received.
    pub bytes_received: u64,
    /// Total bytes forwarded.
    pub bytes_forwarded: u64,
    /// Total failovers triggered.
    pub failovers_triggered: u64,
    /// Total failovers succeeded.
    pub failovers_succeeded: u64,
    /// Server uptime in seconds.
    pub uptime_secs: u64,
}

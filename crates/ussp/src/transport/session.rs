//! Session management for USSP connections.

use crate::error::{Error, Result};
use crate::fec::FecConfig;
use crate::protocol::{AudioCodecType, VideoCodecType};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Session state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Waiting for initialization.
    Initializing,
    /// Session is active.
    Active,
    /// Session is closing.
    Closing,
    /// Session has ended.
    Closed,
}

/// Session configuration.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Session identifier.
    pub session_id: u32,
    /// Number of audio tracks.
    pub audio_track_count: u16,
    /// Video codec type.
    pub video_codec: VideoCodecType,
    /// Audio codec type.
    pub audio_codec: AudioCodecType,
    /// FEC configuration.
    pub fec: FecConfig,
    /// Video frame rate (fps * 100 for precision).
    pub frame_rate: u32,
    /// Audio sample rate.
    pub sample_rate: u32,
    /// Heartbeat interval.
    pub heartbeat_interval: Duration,
    /// Session timeout.
    pub timeout: Duration,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            session_id: rand_session_id(),
            audio_track_count: 2,
            video_codec: VideoCodecType::H264,
            audio_codec: AudioCodecType::Opus,
            fec: FecConfig::default(),
            frame_rate: 3000, // 30.00 fps
            sample_rate: 48000,
            heartbeat_interval: Duration::from_secs(1),
            timeout: Duration::from_secs(10),
        }
    }
}

/// Generate a random session ID.
fn rand_session_id() -> u32 {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let state = RandomState::new();
    let mut hasher = state.build_hasher();
    hasher.write_u64(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64,
    );
    hasher.finish() as u32
}

/// A USSP session.
pub struct Session {
    /// Session configuration.
    config: SessionConfig,
    /// Current state.
    state: SessionState,
    /// Remote address.
    remote_addr: Option<SocketAddr>,
    /// Current sequence number.
    sequence: AtomicU32,
    /// Last activity timestamp.
    last_activity: Instant,
    /// Clock offset (remote - local) in microseconds.
    clock_offset: AtomicU64,
    /// Packets sent.
    packets_sent: AtomicU64,
    /// Packets received.
    packets_received: AtomicU64,
    /// Bytes sent.
    bytes_sent: AtomicU64,
    /// Bytes received.
    bytes_received: AtomicU64,
}

impl Session {
    /// Create a new session with the given configuration.
    pub fn new(config: SessionConfig) -> Self {
        Self {
            config,
            state: SessionState::Initializing,
            remote_addr: None,
            sequence: AtomicU32::new(0),
            last_activity: Instant::now(),
            clock_offset: AtomicU64::new(0),
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
        }
    }

    /// Create a new session with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(SessionConfig::default())
    }

    /// Get the session ID.
    pub fn session_id(&self) -> u32 {
        self.config.session_id
    }

    /// Get the current state.
    pub fn state(&self) -> SessionState {
        self.state
    }

    /// Set the session state.
    pub fn set_state(&mut self, state: SessionState) {
        self.state = state;
    }

    /// Get the remote address.
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        self.remote_addr
    }

    /// Set the remote address.
    pub fn set_remote_addr(&mut self, addr: SocketAddr) {
        self.remote_addr = Some(addr);
    }

    /// Get the next sequence number.
    pub fn next_sequence(&self) -> u32 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current sequence number without incrementing.
    pub fn current_sequence(&self) -> u32 {
        self.sequence.load(Ordering::SeqCst)
    }

    /// Update last activity time.
    pub fn touch(&mut self) {
        self.last_activity = Instant::now();
    }

    /// Check if the session has timed out.
    pub fn is_timed_out(&self) -> bool {
        self.last_activity.elapsed() > self.config.timeout
    }

    /// Get the time since last activity.
    pub fn idle_time(&self) -> Duration {
        self.last_activity.elapsed()
    }

    /// Set the clock offset (for synchronization).
    pub fn set_clock_offset(&self, offset_us: i64) {
        self.clock_offset
            .store(offset_us as u64, Ordering::SeqCst);
    }

    /// Get the clock offset.
    pub fn clock_offset(&self) -> i64 {
        self.clock_offset.load(Ordering::SeqCst) as i64
    }

    /// Get the session configuration.
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    /// Record a sent packet.
    pub fn record_sent(&self, bytes: usize) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Record a received packet.
    pub fn record_received(&self, bytes: usize) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received
            .fetch_add(bytes as u64, Ordering::Relaxed);
    }

    /// Get statistics.
    pub fn stats(&self) -> SessionStats {
        SessionStats {
            packets_sent: self.packets_sent.load(Ordering::Relaxed),
            packets_received: self.packets_received.load(Ordering::Relaxed),
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
        }
    }

    /// Validate that the session is active.
    pub fn validate_active(&self) -> Result<()> {
        match self.state {
            SessionState::Active => Ok(()),
            SessionState::Initializing => {
                Err(Error::Session("Session not yet initialized".to_string()))
            }
            SessionState::Closing | SessionState::Closed => {
                Err(Error::Session("Session is closed".to_string()))
            }
        }
    }
}

/// Session statistics.
#[derive(Debug, Clone, Copy)]
pub struct SessionStats {
    /// Total packets sent.
    pub packets_sent: u64,
    /// Total packets received.
    pub packets_received: u64,
    /// Total bytes sent.
    pub bytes_sent: u64,
    /// Total bytes received.
    pub bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let session = Session::with_defaults();
        assert_eq!(session.state(), SessionState::Initializing);
        assert!(session.remote_addr().is_none());
    }

    #[test]
    fn test_sequence_numbers() {
        let session = Session::with_defaults();
        assert_eq!(session.next_sequence(), 0);
        assert_eq!(session.next_sequence(), 1);
        assert_eq!(session.next_sequence(), 2);
        assert_eq!(session.current_sequence(), 3);
    }

    #[test]
    fn test_session_stats() {
        let session = Session::with_defaults();
        session.record_sent(100);
        session.record_sent(200);
        session.record_received(150);

        let stats = session.stats();
        assert_eq!(stats.packets_sent, 2);
        assert_eq!(stats.bytes_sent, 300);
        assert_eq!(stats.packets_received, 1);
        assert_eq!(stats.bytes_received, 150);
    }
}

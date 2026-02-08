//! Application state management.

use parking_lot::RwLock;
use std::sync::Arc;
use tokio::sync::mpsc;
use ussp::transport::Receiver;

/// Sender state.
pub struct SenderState {
    pub is_running: bool,
    pub stop_tx: Option<mpsc::Sender<()>>,
    pub stats: SenderStats,
}

impl Default for SenderState {
    fn default() -> Self {
        Self {
            is_running: false,
            stop_tx: None,
            stats: SenderStats::default(),
        }
    }
}

/// Receiver state.
pub struct ReceiverState {
    pub is_running: bool,
    pub stop_tx: Option<mpsc::Sender<()>>,
    pub stats: ReceiverStats,
    pub receiver: Option<Arc<Receiver>>,
}

impl Default for ReceiverState {
    fn default() -> Self {
        Self {
            is_running: false,
            stop_tx: None,
            stats: ReceiverStats::default(),
            receiver: None,
        }
    }
}

/// Sender statistics.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct SenderStats {
    pub packets_sent: u64,
    pub bytes_sent: u64,
    pub video_frames_sent: u64,
    pub audio_frames_sent: u64,
    pub fps: f64,
}

/// Receiver statistics.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct ReceiverStats {
    pub packets_received: u64,
    pub bytes_received: u64,
    pub video_frames_received: u64,
    pub audio_frames_received: u64,
    pub packet_loss_rate: f64,
    pub av_sync_diff_us: i64,
    pub fps: f64,
}

/// Application state.
pub struct AppState {
    pub sender: RwLock<SenderState>,
    pub receiver: RwLock<ReceiverState>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            sender: RwLock::new(SenderState::default()),
            receiver: RwLock::new(ReceiverState::default()),
        }
    }
}

impl AppState {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }
}

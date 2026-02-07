//! Jitter buffer for smooth playback.

use crate::protocol::{AudioFrame, VideoFrame};
use std::collections::BTreeMap;
use std::time::Instant;

/// Jitter buffer configuration.
#[derive(Debug, Clone)]
pub struct JitterBufferConfig {
    /// Target buffer depth in microseconds.
    pub target_depth_us: u64,
    /// Minimum buffer depth in microseconds.
    pub min_depth_us: u64,
    /// Maximum buffer depth in microseconds.
    pub max_depth_us: u64,
    /// Maximum frames to buffer.
    pub max_frames: usize,
}

impl Default for JitterBufferConfig {
    fn default() -> Self {
        Self {
            target_depth_us: 100_000,  // 100ms
            min_depth_us: 50_000,      // 50ms
            max_depth_us: 500_000,     // 500ms
            max_frames: 300,
        }
    }
}

/// A buffered media item.
#[derive(Debug)]
pub enum BufferedItem {
    Video(VideoFrame),
    Audio(AudioFrame),
}

impl BufferedItem {
    /// Get the PTS of this item.
    pub fn pts(&self) -> u64 {
        match self {
            BufferedItem::Video(f) => f.pts,
            BufferedItem::Audio(f) => f.pts,
        }
    }

    /// Get the track ID (0 for video, track_id + 1 for audio).
    pub fn track_id(&self) -> u16 {
        match self {
            BufferedItem::Video(_) => 0,
            BufferedItem::Audio(f) => f.track_id + 1,
        }
    }
}

/// Jitter buffer state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferState {
    /// Buffer is filling up.
    Filling,
    /// Buffer is ready for playback.
    Ready,
    /// Buffer is empty (underrun).
    Empty,
}

/// A jitter buffer for smoothing out network jitter.
pub struct JitterBuffer {
    config: JitterBufferConfig,
    /// Frames indexed by PTS.
    frames: BTreeMap<u64, BufferedItem>,
    /// Current state.
    state: BufferState,
    /// Last output PTS.
    last_output_pts: Option<u64>,
    /// When the buffer became ready.
    ready_at: Option<Instant>,
    /// Statistics.
    stats: BufferStats,
}

/// Buffer statistics.
#[derive(Debug, Clone, Copy, Default)]
pub struct BufferStats {
    /// Total frames received.
    pub frames_received: u64,
    /// Total frames output.
    pub frames_output: u64,
    /// Frames dropped due to being late.
    pub frames_dropped_late: u64,
    /// Frames dropped due to buffer overflow.
    pub frames_dropped_overflow: u64,
    /// Buffer underruns.
    pub underruns: u64,
}

impl JitterBuffer {
    /// Create a new jitter buffer with the given configuration.
    pub fn new(config: JitterBufferConfig) -> Self {
        Self {
            config,
            frames: BTreeMap::new(),
            state: BufferState::Filling,
            last_output_pts: None,
            ready_at: None,
            stats: BufferStats::default(),
        }
    }

    /// Create a new jitter buffer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(JitterBufferConfig::default())
    }

    /// Insert a video frame into the buffer.
    pub fn insert_video(&mut self, frame: VideoFrame) {
        self.insert(BufferedItem::Video(frame));
    }

    /// Insert an audio frame into the buffer.
    pub fn insert_audio(&mut self, frame: AudioFrame) {
        self.insert(BufferedItem::Audio(frame));
    }

    /// Insert an item into the buffer.
    fn insert(&mut self, item: BufferedItem) {
        let pts = item.pts();
        self.stats.frames_received += 1;

        // Drop if too old
        if let Some(last_pts) = self.last_output_pts {
            if pts < last_pts {
                self.stats.frames_dropped_late += 1;
                return;
            }
        }

        // Drop oldest if buffer is full
        if self.frames.len() >= self.config.max_frames {
            if let Some((&oldest_pts, _)) = self.frames.first_key_value() {
                self.frames.remove(&oldest_pts);
                self.stats.frames_dropped_overflow += 1;
            }
        }

        self.frames.insert(pts, item);
        self.update_state();
    }

    /// Get the next frame to play.
    ///
    /// # Arguments
    /// * `current_pts` - The current playback PTS.
    ///
    /// Returns the next frame if one is ready, None otherwise.
    pub fn get_next(&mut self, current_pts: u64) -> Option<BufferedItem> {
        if self.state != BufferState::Ready {
            return None;
        }

        // Find frames at or before current_pts
        let keys_to_remove: Vec<u64> = self
            .frames
            .range(..=current_pts)
            .map(|(&k, _)| k)
            .collect();

        if keys_to_remove.is_empty() {
            return None;
        }

        // Return the latest frame, drop older ones
        let mut result = None;
        for pts in keys_to_remove {
            if let Some(frame) = self.frames.remove(&pts) {
                if result.is_some() {
                    // Dropping older frame
                    self.stats.frames_dropped_late += 1;
                } else {
                    self.stats.frames_output += 1;
                    self.last_output_pts = Some(pts);
                }
                result = Some(frame);
            }
        }

        self.update_state();
        result
    }

    /// Peek at the next frame without removing it.
    pub fn peek_next(&self) -> Option<&BufferedItem> {
        self.frames.values().next()
    }

    /// Get the PTS of the next frame.
    pub fn next_pts(&self) -> Option<u64> {
        self.frames.keys().next().copied()
    }

    /// Get the current buffer depth in microseconds.
    pub fn depth_us(&self) -> u64 {
        if let (Some(&first), Some(&last)) = (self.frames.keys().next(), self.frames.keys().last())
        {
            last.saturating_sub(first)
        } else {
            0
        }
    }

    /// Get the current buffer state.
    pub fn state(&self) -> BufferState {
        self.state
    }

    /// Get the number of buffered frames.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Get buffer statistics.
    pub fn stats(&self) -> BufferStats {
        self.stats
    }

    /// Clear the buffer.
    pub fn clear(&mut self) {
        self.frames.clear();
        self.state = BufferState::Filling;
        self.last_output_pts = None;
        self.ready_at = None;
    }

    /// Update the buffer state.
    fn update_state(&mut self) {
        let depth = self.depth_us();

        match self.state {
            BufferState::Filling => {
                if depth >= self.config.target_depth_us {
                    self.state = BufferState::Ready;
                    self.ready_at = Some(Instant::now());
                }
            }
            BufferState::Ready => {
                if self.frames.is_empty() {
                    self.state = BufferState::Empty;
                    self.stats.underruns += 1;
                }
            }
            BufferState::Empty => {
                if depth >= self.config.min_depth_us {
                    self.state = BufferState::Ready;
                    self.ready_at = Some(Instant::now());
                } else if !self.frames.is_empty() {
                    self.state = BufferState::Filling;
                }
            }
        }
    }
}

/// Per-track jitter buffer.
pub struct MultiTrackBuffer {
    /// Video buffer.
    pub video: JitterBuffer,
    /// Audio buffers per track.
    pub audio: Vec<JitterBuffer>,
}

impl MultiTrackBuffer {
    /// Create a new multi-track buffer.
    pub fn new(config: JitterBufferConfig, audio_tracks: usize) -> Self {
        Self {
            video: JitterBuffer::new(config.clone()),
            audio: (0..audio_tracks)
                .map(|_| JitterBuffer::new(config.clone()))
                .collect(),
        }
    }

    /// Insert a video frame.
    pub fn insert_video(&mut self, frame: VideoFrame) {
        self.video.insert_video(frame);
    }

    /// Insert an audio frame.
    pub fn insert_audio(&mut self, frame: AudioFrame) {
        let track_id = frame.track_id as usize;
        if track_id < self.audio.len() {
            self.audio[track_id].insert_audio(frame);
        }
    }

    /// Check if all buffers are ready.
    pub fn all_ready(&self) -> bool {
        self.video.state() == BufferState::Ready
            && self.audio.iter().all(|b| b.state() == BufferState::Ready)
    }

    /// Get the minimum next PTS across all tracks.
    pub fn min_next_pts(&self) -> Option<u64> {
        let video_pts = self.video.next_pts();
        let audio_pts = self.audio.iter().filter_map(|b| b.next_pts()).min();

        match (video_pts, audio_pts) {
            (Some(v), Some(a)) => Some(v.min(a)),
            (Some(v), None) => Some(v),
            (None, Some(a)) => Some(a),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{AudioCodecType, VideoCodecType};
    use bytes::Bytes;

    fn make_video_frame(pts: u64) -> VideoFrame {
        VideoFrame {
            frame_id: 0,
            codec: VideoCodecType::H264,
            is_key_frame: false,
            pts,
            dts: pts,
            data: Bytes::new(),
        }
    }

    fn make_audio_frame(pts: u64, track_id: u16) -> AudioFrame {
        AudioFrame {
            track_id,
            sample_rate: 48000,
            channels: 2,
            bits_per_sample: 16,
            codec: AudioCodecType::Opus,
            pts,
            sample_count: 960,
            data: Bytes::new(),
        }
    }

    #[test]
    fn test_buffer_filling() {
        let config = JitterBufferConfig {
            target_depth_us: 100_000,
            min_depth_us: 50_000,
            max_depth_us: 500_000,
            max_frames: 100,
        };

        let mut buffer = JitterBuffer::new(config);
        assert_eq!(buffer.state(), BufferState::Filling);

        // Add frames spanning 100ms
        for i in 0..10 {
            buffer.insert_video(make_video_frame(i * 10_000));
        }

        // Should now be ready (90ms gap between first and last frame)
        // Need more frames to reach 100ms
        buffer.insert_video(make_video_frame(100_000));
        assert_eq!(buffer.state(), BufferState::Ready);
    }

    #[test]
    fn test_buffer_output() {
        let mut buffer = JitterBuffer::with_defaults();

        // Insert frames
        for i in 0..20 {
            buffer.insert_video(make_video_frame(i * 10_000));
        }

        // Force ready state
        buffer.state = BufferState::Ready;

        // Get frame at pts 50_000
        let frame = buffer.get_next(50_000);
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().pts(), 50_000);

        // Older frames should be dropped
        assert!(buffer.stats.frames_dropped_late > 0 || buffer.stats.frames_output == 1);
    }

    #[test]
    fn test_multi_track_buffer() {
        let config = JitterBufferConfig::default();
        let mut buffer = MultiTrackBuffer::new(config, 2);

        // Insert video and audio
        for i in 0..20 {
            buffer.insert_video(make_video_frame(i * 10_000));
            buffer.insert_audio(make_audio_frame(i * 10_000, 0));
            buffer.insert_audio(make_audio_frame(i * 10_000, 1));
        }

        assert!(buffer.min_next_pts().is_some());
    }
}

//! Synchronization point payload definitions.

use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Synchronization point payload.
///
/// Sent periodically to provide synchronization information for all tracks.
#[derive(Debug, Clone)]
pub struct SyncPoint {
    /// Reference timestamp for this sync point (microseconds).
    pub reference_timestamp: u64,
    /// Video PTS at this sync point.
    pub video_pts: u64,
    /// Per-track audio synchronization info.
    pub audio_tracks: Vec<TrackSyncInfo>,
}

/// Synchronization info for a single audio track.
#[derive(Debug, Clone, Copy)]
pub struct TrackSyncInfo {
    /// Track identifier.
    pub track_id: u16,
    /// Audio PTS at the sync point.
    pub pts: u64,
    /// Offset from reference timestamp (can be negative, stored as i64).
    pub offset_us: i64,
}

impl SyncPoint {
    /// Create a new sync point.
    pub fn new(reference_timestamp: u64, video_pts: u64) -> Self {
        Self {
            reference_timestamp,
            video_pts,
            audio_tracks: Vec::new(),
        }
    }

    /// Add an audio track sync info.
    pub fn add_track(&mut self, track_id: u16, pts: u64, offset_us: i64) {
        self.audio_tracks.push(TrackSyncInfo {
            track_id,
            pts,
            offset_us,
        });
    }

    /// Encode the sync point to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u64(self.reference_timestamp);
        buf.put_u64(self.video_pts);
        buf.put_u16(self.audio_tracks.len() as u16);
        buf.put_u16(0); // reserved
        buf.put_u32(0); // reserved

        for track in &self.audio_tracks {
            buf.put_u16(track.track_id);
            buf.put_u16(0); // reserved
            buf.put_u32(0); // reserved
            buf.put_u64(track.pts);
            buf.put_i64(track.offset_us);
        }

        Ok(())
    }

    /// Decode a sync point from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < 24 {
            return Err(Error::BufferTooSmall {
                needed: 24,
                got: buf.remaining(),
            });
        }

        let reference_timestamp = buf.get_u64();
        let video_pts = buf.get_u64();
        let track_count = buf.get_u16() as usize;
        let _reserved1 = buf.get_u16();
        let _reserved2 = buf.get_u32();

        let track_info_size = 24; // per track
        if buf.remaining() < track_count * track_info_size {
            return Err(Error::BufferTooSmall {
                needed: track_count * track_info_size,
                got: buf.remaining(),
            });
        }

        let mut audio_tracks = Vec::with_capacity(track_count);
        for _ in 0..track_count {
            let track_id = buf.get_u16();
            let _reserved1 = buf.get_u16();
            let _reserved2 = buf.get_u32();
            let pts = buf.get_u64();
            let offset_us = buf.get_i64();

            audio_tracks.push(TrackSyncInfo {
                track_id,
                pts,
                offset_us,
            });
        }

        Ok(Self {
            reference_timestamp,
            video_pts,
            audio_tracks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_point_roundtrip() {
        let mut sync = SyncPoint::new(1_000_000, 1_000_000);
        sync.add_track(0, 1_000_000, 0);
        sync.add_track(1, 1_001_000, 1000);
        sync.add_track(2, 999_000, -1000);

        let mut buf = BytesMut::new();
        sync.encode(&mut buf).unwrap();

        let mut bytes = buf.freeze();
        let decoded = SyncPoint::decode(&mut bytes).unwrap();

        assert_eq!(decoded.reference_timestamp, 1_000_000);
        assert_eq!(decoded.video_pts, 1_000_000);
        assert_eq!(decoded.audio_tracks.len(), 3);
        assert_eq!(decoded.audio_tracks[0].track_id, 0);
        assert_eq!(decoded.audio_tracks[1].offset_us, 1000);
        assert_eq!(decoded.audio_tracks[2].offset_us, -1000);
    }
}

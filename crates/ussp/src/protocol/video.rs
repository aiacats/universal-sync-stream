//! Video frame payload definitions.

use super::types::VideoCodecType;
use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Video payload header size.
pub const VIDEO_PAYLOAD_HEADER_SIZE: usize = 32;

/// Video frame payload.
#[derive(Debug, Clone)]
pub struct VideoPayload {
    /// Frame identifier.
    pub frame_id: u32,
    /// Fragment index (0-based).
    pub fragment_index: u16,
    /// Total number of fragments for this frame.
    pub total_fragments: u16,
    /// Video codec type.
    pub codec: VideoCodecType,
    /// Whether this is a key frame.
    pub is_key_frame: bool,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// Decoding timestamp in microseconds.
    pub dts: u64,
    /// Frame data.
    pub data: Bytes,
}

impl VideoPayload {
    /// Create a new video payload for a complete frame (single fragment).
    pub fn new(
        frame_id: u32,
        codec: VideoCodecType,
        is_key_frame: bool,
        pts: u64,
        dts: u64,
        data: Bytes,
    ) -> Self {
        Self {
            frame_id,
            fragment_index: 0,
            total_fragments: 1,
            codec,
            is_key_frame,
            pts,
            dts,
            data,
        }
    }

    /// Create a new video payload fragment.
    pub fn new_fragment(
        frame_id: u32,
        fragment_index: u16,
        total_fragments: u16,
        codec: VideoCodecType,
        is_key_frame: bool,
        pts: u64,
        dts: u64,
        data: Bytes,
    ) -> Self {
        Self {
            frame_id,
            fragment_index,
            total_fragments,
            codec,
            is_key_frame,
            pts,
            dts,
            data,
        }
    }

    /// Encode the video payload to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u32(self.frame_id);
        buf.put_u16(self.fragment_index);
        buf.put_u16(self.total_fragments);
        buf.put_u8(self.codec as u8);
        buf.put_u8(if self.is_key_frame { 1 } else { 0 });
        buf.put_u16(0); // reserved
        buf.put_u64(self.pts);
        buf.put_u64(self.dts);
        buf.put_u32(self.data.len() as u32);
        buf.put_slice(&self.data);
        Ok(())
    }

    /// Decode a video payload from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < VIDEO_PAYLOAD_HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: VIDEO_PAYLOAD_HEADER_SIZE,
                got: buf.remaining(),
            });
        }

        let frame_id = buf.get_u32();
        let fragment_index = buf.get_u16();
        let total_fragments = buf.get_u16();
        let codec = VideoCodecType::try_from(buf.get_u8())?;
        let is_key_frame = buf.get_u8() != 0;
        let _reserved = buf.get_u16();
        let pts = buf.get_u64();
        let dts = buf.get_u64();
        let data_len = buf.get_u32() as usize;

        if buf.remaining() < data_len {
            return Err(Error::BufferTooSmall {
                needed: data_len,
                got: buf.remaining(),
            });
        }

        let data = buf.copy_to_bytes(data_len);

        Ok(Self {
            frame_id,
            fragment_index,
            total_fragments,
            codec,
            is_key_frame,
            pts,
            dts,
            data,
        })
    }

    /// Check if this is the last fragment of a frame.
    pub fn is_last_fragment(&self) -> bool {
        self.fragment_index == self.total_fragments - 1
    }

    /// Check if this is a complete frame (single fragment).
    pub fn is_complete(&self) -> bool {
        self.total_fragments == 1
    }
}

/// A complete video frame assembled from fragments.
#[derive(Debug, Clone)]
pub struct VideoFrame {
    /// Frame identifier.
    pub frame_id: u32,
    /// Video codec type.
    pub codec: VideoCodecType,
    /// Whether this is a key frame.
    pub is_key_frame: bool,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// Decoding timestamp in microseconds.
    pub dts: u64,
    /// Complete frame data.
    pub data: Bytes,
}

impl From<VideoPayload> for VideoFrame {
    fn from(payload: VideoPayload) -> Self {
        Self {
            frame_id: payload.frame_id,
            codec: payload.codec,
            is_key_frame: payload.is_key_frame,
            pts: payload.pts,
            dts: payload.dts,
            data: payload.data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_video_payload_roundtrip() {
        let payload = VideoPayload::new(
            1,
            VideoCodecType::H264,
            true,
            1_000_000,
            900_000,
            Bytes::from_static(b"video data here"),
        );

        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();

        let mut bytes = buf.freeze();
        let decoded = VideoPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.frame_id, 1);
        assert_eq!(decoded.codec, VideoCodecType::H264);
        assert!(decoded.is_key_frame);
        assert_eq!(decoded.pts, 1_000_000);
        assert_eq!(decoded.dts, 900_000);
        assert_eq!(decoded.data.as_ref(), b"video data here");
    }
}

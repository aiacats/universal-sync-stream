//! Audio frame payload definitions.

use super::types::AudioCodecType;
use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Audio payload header size.
pub const AUDIO_PAYLOAD_HEADER_SIZE: usize = 24;

/// Audio frame payload.
#[derive(Debug, Clone)]
pub struct AudioPayload {
    /// Track identifier (0-based).
    pub track_id: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u8,
    /// Bits per sample.
    pub bits_per_sample: u8,
    /// Audio codec type.
    pub codec: AudioCodecType,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// Number of samples in this frame.
    pub sample_count: u32,
    /// Audio data.
    pub data: Bytes,
}

impl AudioPayload {
    /// Create a new audio payload.
    pub fn new(
        track_id: u16,
        sample_rate: u32,
        channels: u8,
        bits_per_sample: u8,
        codec: AudioCodecType,
        pts: u64,
        sample_count: u32,
        data: Bytes,
    ) -> Self {
        Self {
            track_id,
            sample_rate,
            channels,
            bits_per_sample,
            codec,
            pts,
            sample_count,
            data,
        }
    }

    /// Encode the audio payload to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_u16(self.track_id);
        buf.put_u32(self.sample_rate);
        buf.put_u8(self.channels);
        buf.put_u8(self.bits_per_sample);
        buf.put_u8(self.codec as u8);
        buf.put_u8(0); // reserved
        buf.put_u16(0); // reserved
        buf.put_u64(self.pts);
        buf.put_u32(self.sample_count);
        buf.put_u32(self.data.len() as u32);
        buf.put_slice(&self.data);
        Ok(())
    }

    /// Decode an audio payload from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < AUDIO_PAYLOAD_HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: AUDIO_PAYLOAD_HEADER_SIZE,
                got: buf.remaining(),
            });
        }

        let track_id = buf.get_u16();
        let sample_rate = buf.get_u32();
        let channels = buf.get_u8();
        let bits_per_sample = buf.get_u8();
        let codec = AudioCodecType::try_from(buf.get_u8())?;
        let _reserved1 = buf.get_u8();
        let _reserved2 = buf.get_u16();
        let pts = buf.get_u64();
        let sample_count = buf.get_u32();
        let data_len = buf.get_u32() as usize;

        if buf.remaining() < data_len {
            return Err(Error::BufferTooSmall {
                needed: data_len,
                got: buf.remaining(),
            });
        }

        let data = buf.copy_to_bytes(data_len);

        Ok(Self {
            track_id,
            sample_rate,
            channels,
            bits_per_sample,
            codec,
            pts,
            sample_count,
            data,
        })
    }

    /// Calculate the duration of this audio frame in microseconds.
    pub fn duration_us(&self) -> u64 {
        if self.sample_rate == 0 {
            return 0;
        }
        (self.sample_count as u64 * 1_000_000) / self.sample_rate as u64
    }
}

/// A complete audio frame.
#[derive(Debug, Clone)]
pub struct AudioFrame {
    /// Track identifier (0-based).
    pub track_id: u16,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Number of channels.
    pub channels: u8,
    /// Bits per sample.
    pub bits_per_sample: u8,
    /// Audio codec type.
    pub codec: AudioCodecType,
    /// Presentation timestamp in microseconds.
    pub pts: u64,
    /// Number of samples in this frame.
    pub sample_count: u32,
    /// Audio data.
    pub data: Bytes,
}

impl From<AudioPayload> for AudioFrame {
    fn from(payload: AudioPayload) -> Self {
        Self {
            track_id: payload.track_id,
            sample_rate: payload.sample_rate,
            channels: payload.channels,
            bits_per_sample: payload.bits_per_sample,
            codec: payload.codec,
            pts: payload.pts,
            sample_count: payload.sample_count,
            data: payload.data,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_payload_roundtrip() {
        let payload = AudioPayload::new(
            0,
            48000,
            2,
            16,
            AudioCodecType::Opus,
            1_000_000,
            960,
            Bytes::from_static(b"audio data here"),
        );

        let mut buf = BytesMut::new();
        payload.encode(&mut buf).unwrap();

        let mut bytes = buf.freeze();
        let decoded = AudioPayload::decode(&mut bytes).unwrap();

        assert_eq!(decoded.track_id, 0);
        assert_eq!(decoded.sample_rate, 48000);
        assert_eq!(decoded.channels, 2);
        assert_eq!(decoded.bits_per_sample, 16);
        assert_eq!(decoded.codec, AudioCodecType::Opus);
        assert_eq!(decoded.pts, 1_000_000);
        assert_eq!(decoded.sample_count, 960);
    }

    #[test]
    fn test_duration_calculation() {
        let payload = AudioPayload::new(
            0,
            48000,
            2,
            16,
            AudioCodecType::Opus,
            0,
            48000, // 1 second worth of samples
            Bytes::new(),
        );

        assert_eq!(payload.duration_us(), 1_000_000);
    }
}

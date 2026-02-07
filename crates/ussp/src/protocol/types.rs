//! Common types for USSP protocol.

use crate::error::Error;
use bitflags::bitflags;

bitflags! {
    /// Packet flags.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Flags: u16 {
        /// This is a key frame (for video).
        const KEY_FRAME = 0x0001;
        /// End of stream.
        const END_OF_STREAM = 0x0002;
        /// Packet requires acknowledgment.
        const REQUIRES_ACK = 0x0004;
        /// Packet is a retransmission.
        const RETRANSMISSION = 0x0008;
        /// FEC protected.
        const FEC_PROTECTED = 0x0010;
    }
}

/// Video codec types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VideoCodecType {
    /// Raw uncompressed video.
    Raw = 0,
    /// H.264/AVC.
    H264 = 1,
    /// H.265/HEVC.
    H265 = 2,
    /// VP8.
    Vp8 = 3,
    /// VP9.
    Vp9 = 4,
    /// AV1.
    Av1 = 5,
}

impl TryFrom<u8> for VideoCodecType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(VideoCodecType::Raw),
            1 => Ok(VideoCodecType::H264),
            2 => Ok(VideoCodecType::H265),
            3 => Ok(VideoCodecType::Vp8),
            4 => Ok(VideoCodecType::Vp9),
            5 => Ok(VideoCodecType::Av1),
            _ => Err(Error::Codec(format!("Unknown video codec: {}", value))),
        }
    }
}

/// Audio codec types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum AudioCodecType {
    /// Raw PCM.
    Pcm = 0,
    /// AAC.
    Aac = 1,
    /// Opus.
    Opus = 2,
    /// MP3.
    Mp3 = 3,
    /// FLAC.
    Flac = 4,
}

impl TryFrom<u8> for AudioCodecType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(AudioCodecType::Pcm),
            1 => Ok(AudioCodecType::Aac),
            2 => Ok(AudioCodecType::Opus),
            3 => Ok(AudioCodecType::Mp3),
            4 => Ok(AudioCodecType::Flac),
            _ => Err(Error::Codec(format!("Unknown audio codec: {}", value))),
        }
    }
}

/// Generic codec type enum for convenience.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodecType {
    /// Video codec.
    Video(VideoCodecType),
    /// Audio codec.
    Audio(AudioCodecType),
}

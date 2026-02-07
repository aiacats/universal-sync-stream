//! Packet header definitions.

use super::types::Flags;
use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Magic bytes identifying USSP packets.
pub const MAGIC: [u8; 4] = *b"USSP";

/// Current protocol version.
pub const VERSION: u8 = 1;

/// Header size in bytes.
pub const HEADER_SIZE: usize = 24;

/// Packet types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PacketType {
    /// Video frame data.
    VideoFrame = 0x01,
    /// Audio frame data.
    AudioFrame = 0x02,
    /// Synchronization point.
    SyncPoint = 0x03,
    /// FEC repair packet.
    FecRepair = 0x04,
    /// Session initialization.
    SessionInit = 0x05,
    /// Session acknowledgment.
    SessionAck = 0x06,
    /// Heartbeat / keep-alive.
    Heartbeat = 0x07,
    /// DMX frame data (Art-Net compatible).
    DmxFrame = 0x08,
}

impl TryFrom<u8> for PacketType {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self> {
        match value {
            0x01 => Ok(PacketType::VideoFrame),
            0x02 => Ok(PacketType::AudioFrame),
            0x03 => Ok(PacketType::SyncPoint),
            0x04 => Ok(PacketType::FecRepair),
            0x05 => Ok(PacketType::SessionInit),
            0x06 => Ok(PacketType::SessionAck),
            0x07 => Ok(PacketType::Heartbeat),
            0x08 => Ok(PacketType::DmxFrame),
            _ => Err(Error::UnknownPacketType(value)),
        }
    }
}

/// USSP packet header (24 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PacketHeader {
    /// Protocol version.
    pub version: u8,
    /// Packet type.
    pub packet_type: PacketType,
    /// Packet flags.
    pub flags: Flags,
    /// Session identifier.
    pub session_id: u32,
    /// Sequence number.
    pub sequence: u32,
    /// Timestamp in microseconds.
    pub timestamp: u64,
}

impl PacketHeader {
    /// Create a new packet header.
    pub fn new(packet_type: PacketType, session_id: u32, sequence: u32, timestamp: u64) -> Self {
        Self {
            version: VERSION,
            packet_type,
            flags: Flags::empty(),
            session_id,
            sequence,
            timestamp,
        }
    }

    /// Set flags on the header.
    pub fn with_flags(mut self, flags: Flags) -> Self {
        self.flags = flags;
        self
    }

    /// Encode the header to bytes.
    pub fn encode(&self, buf: &mut BytesMut) -> Result<()> {
        buf.put_slice(&MAGIC);
        buf.put_u8(self.version);
        buf.put_u8(self.packet_type as u8);
        buf.put_u16(self.flags.bits());
        buf.put_u32(self.session_id);
        buf.put_u32(self.sequence);
        buf.put_u64(self.timestamp);
        Ok(())
    }

    /// Decode a header from bytes.
    pub fn decode(buf: &mut Bytes) -> Result<Self> {
        if buf.remaining() < HEADER_SIZE {
            return Err(Error::BufferTooSmall {
                needed: HEADER_SIZE,
                got: buf.remaining(),
            });
        }

        let mut magic = [0u8; 4];
        buf.copy_to_slice(&mut magic);
        if magic != MAGIC {
            return Err(Error::InvalidMagic);
        }

        let version = buf.get_u8();
        if version != VERSION {
            return Err(Error::UnsupportedVersion(version));
        }

        let packet_type = PacketType::try_from(buf.get_u8())?;
        let flags = Flags::from_bits_truncate(buf.get_u16());
        let session_id = buf.get_u32();
        let sequence = buf.get_u32();
        let timestamp = buf.get_u64();

        Ok(Self {
            version,
            packet_type,
            flags,
            session_id,
            sequence,
            timestamp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_roundtrip() {
        let header = PacketHeader::new(PacketType::VideoFrame, 12345, 100, 1_000_000)
            .with_flags(Flags::KEY_FRAME);

        let mut buf = BytesMut::new();
        header.encode(&mut buf).unwrap();

        let mut bytes = buf.freeze();
        let decoded = PacketHeader::decode(&mut bytes).unwrap();

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_invalid_magic() {
        let mut bytes = Bytes::from_static(b"XXXX\x01\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00");
        let result = PacketHeader::decode(&mut bytes);
        assert!(matches!(result, Err(Error::InvalidMagic)));
    }
}

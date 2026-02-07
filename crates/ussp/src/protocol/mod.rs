//! Protocol definitions for USSP packets.

mod audio;
mod dmx;
mod header;
mod sync;
mod types;
mod video;

pub use audio::{AudioFrame, AudioPayload};
pub use dmx::{DmxFrame, DmxPayload, DMX_CHANNELS_PER_UNIVERSE};
pub use header::{PacketHeader, PacketType, HEADER_SIZE, MAGIC};
pub use sync::{SyncPoint, TrackSyncInfo};
pub use types::{AudioCodecType, CodecType, Flags, VideoCodecType};
pub use video::{VideoFrame, VideoPayload};

use crate::error::{Error, Result};
use bytes::{Buf, BufMut, Bytes, BytesMut};

/// Maximum UDP payload size (considering typical MTU of 1500 - IP/UDP headers).
pub const MAX_PAYLOAD_SIZE: usize = 1400;

/// A complete USSP packet.
#[derive(Debug, Clone)]
pub struct Packet {
    /// Packet header.
    pub header: PacketHeader,
    /// Packet payload.
    pub payload: Payload,
}

/// Packet payload variants.
#[derive(Debug, Clone)]
pub enum Payload {
    /// Video frame data.
    Video(VideoPayload),
    /// Audio frame data.
    Audio(AudioPayload),
    /// Synchronization point.
    Sync(SyncPoint),
    /// FEC repair data.
    FecRepair(FecRepairPayload),
    /// Session initialization.
    SessionInit(SessionInitPayload),
    /// Session acknowledgment.
    SessionAck(SessionAckPayload),
    /// Heartbeat.
    Heartbeat,
    /// DMX frame data (Art-Net compatible).
    Dmx(DmxPayload),
}

/// FEC repair packet payload.
#[derive(Debug, Clone)]
pub struct FecRepairPayload {
    /// FEC group ID.
    pub group_id: u32,
    /// Index within the FEC group.
    pub index: u16,
    /// Total shards in the group (data + parity).
    pub total_shards: u16,
    /// Number of data shards.
    pub data_shards: u16,
    /// Repair data.
    pub data: Bytes,
}

/// Session initialization payload.
#[derive(Debug, Clone)]
pub struct SessionInitPayload {
    /// Requested session ID.
    pub session_id: u32,
    /// Number of audio tracks.
    pub audio_track_count: u16,
    /// Video codec type.
    pub video_codec: VideoCodecType,
    /// Audio codec type.
    pub audio_codec: AudioCodecType,
    /// FEC data shards (k).
    pub fec_data_shards: u16,
    /// FEC parity shards (m).
    pub fec_parity_shards: u16,
    /// Target video frame rate (fps * 100).
    pub frame_rate: u32,
    /// Audio sample rate.
    pub sample_rate: u32,
}

/// Session acknowledgment payload.
#[derive(Debug, Clone)]
pub struct SessionAckPayload {
    /// Acknowledged session ID.
    pub session_id: u32,
    /// Whether session was accepted.
    pub accepted: bool,
    /// Server timestamp for clock sync.
    pub server_timestamp: u64,
}

impl Packet {
    /// Encode the packet to bytes.
    pub fn encode(&self) -> Result<Bytes> {
        let mut buf = BytesMut::with_capacity(MAX_PAYLOAD_SIZE);
        self.header.encode(&mut buf)?;

        match &self.payload {
            Payload::Video(v) => v.encode(&mut buf)?,
            Payload::Audio(a) => a.encode(&mut buf)?,
            Payload::Sync(s) => s.encode(&mut buf)?,
            Payload::FecRepair(f) => encode_fec_repair(f, &mut buf)?,
            Payload::SessionInit(s) => encode_session_init(s, &mut buf)?,
            Payload::SessionAck(s) => encode_session_ack(s, &mut buf)?,
            Payload::Heartbeat => {}
            Payload::Dmx(d) => d.encode(&mut buf)?,
        }

        Ok(buf.freeze())
    }

    /// Decode a packet from bytes.
    pub fn decode(mut data: Bytes) -> Result<Self> {
        let header = PacketHeader::decode(&mut data)?;

        let payload = match header.packet_type {
            PacketType::VideoFrame => Payload::Video(VideoPayload::decode(&mut data)?),
            PacketType::AudioFrame => Payload::Audio(AudioPayload::decode(&mut data)?),
            PacketType::SyncPoint => Payload::Sync(SyncPoint::decode(&mut data)?),
            PacketType::FecRepair => Payload::FecRepair(decode_fec_repair(&mut data)?),
            PacketType::SessionInit => Payload::SessionInit(decode_session_init(&mut data)?),
            PacketType::SessionAck => Payload::SessionAck(decode_session_ack(&mut data)?),
            PacketType::Heartbeat => Payload::Heartbeat,
            PacketType::DmxFrame => Payload::Dmx(DmxPayload::decode(&mut data)?),
        };

        Ok(Packet { header, payload })
    }
}

fn encode_fec_repair(payload: &FecRepairPayload, buf: &mut BytesMut) -> Result<()> {
    buf.put_u32(payload.group_id);
    buf.put_u16(payload.index);
    buf.put_u16(payload.total_shards);
    buf.put_u16(payload.data_shards);
    buf.put_u16(0); // reserved
    buf.put_u32(payload.data.len() as u32);
    buf.put_slice(&payload.data);
    Ok(())
}

fn decode_fec_repair(buf: &mut Bytes) -> Result<FecRepairPayload> {
    if buf.remaining() < 16 {
        return Err(Error::BufferTooSmall {
            needed: 16,
            got: buf.remaining(),
        });
    }

    let group_id = buf.get_u32();
    let index = buf.get_u16();
    let total_shards = buf.get_u16();
    let data_shards = buf.get_u16();
    let _reserved = buf.get_u16();
    let data_len = buf.get_u32() as usize;

    if buf.remaining() < data_len {
        return Err(Error::BufferTooSmall {
            needed: data_len,
            got: buf.remaining(),
        });
    }

    let data = buf.copy_to_bytes(data_len);

    Ok(FecRepairPayload {
        group_id,
        index,
        total_shards,
        data_shards,
        data,
    })
}

fn encode_session_init(payload: &SessionInitPayload, buf: &mut BytesMut) -> Result<()> {
    buf.put_u32(payload.session_id);
    buf.put_u16(payload.audio_track_count);
    buf.put_u8(payload.video_codec as u8);
    buf.put_u8(payload.audio_codec as u8);
    buf.put_u16(payload.fec_data_shards);
    buf.put_u16(payload.fec_parity_shards);
    buf.put_u32(payload.frame_rate);
    buf.put_u32(payload.sample_rate);
    Ok(())
}

fn decode_session_init(buf: &mut Bytes) -> Result<SessionInitPayload> {
    if buf.remaining() < 20 {
        return Err(Error::BufferTooSmall {
            needed: 20,
            got: buf.remaining(),
        });
    }

    let session_id = buf.get_u32();
    let audio_track_count = buf.get_u16();
    let video_codec = VideoCodecType::try_from(buf.get_u8())?;
    let audio_codec = AudioCodecType::try_from(buf.get_u8())?;
    let fec_data_shards = buf.get_u16();
    let fec_parity_shards = buf.get_u16();
    let frame_rate = buf.get_u32();
    let sample_rate = buf.get_u32();

    Ok(SessionInitPayload {
        session_id,
        audio_track_count,
        video_codec,
        audio_codec,
        fec_data_shards,
        fec_parity_shards,
        frame_rate,
        sample_rate,
    })
}

fn encode_session_ack(payload: &SessionAckPayload, buf: &mut BytesMut) -> Result<()> {
    buf.put_u32(payload.session_id);
    buf.put_u8(if payload.accepted { 1 } else { 0 });
    buf.put_u8(0); // reserved
    buf.put_u16(0); // reserved
    buf.put_u64(payload.server_timestamp);
    Ok(())
}

fn decode_session_ack(buf: &mut Bytes) -> Result<SessionAckPayload> {
    if buf.remaining() < 16 {
        return Err(Error::BufferTooSmall {
            needed: 16,
            got: buf.remaining(),
        });
    }

    let session_id = buf.get_u32();
    let accepted = buf.get_u8() != 0;
    let _reserved1 = buf.get_u8();
    let _reserved2 = buf.get_u16();
    let server_timestamp = buf.get_u64();

    Ok(SessionAckPayload {
        session_id,
        accepted,
        server_timestamp,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packet_roundtrip() {
        let packet = Packet {
            header: PacketHeader {
                version: 1,
                packet_type: PacketType::Heartbeat,
                flags: Flags::empty(),
                session_id: 12345,
                sequence: 1,
                timestamp: 1000000,
            },
            payload: Payload::Heartbeat,
        };

        let encoded = packet.encode().unwrap();
        let decoded = Packet::decode(encoded).unwrap();

        assert_eq!(decoded.header.session_id, 12345);
        assert_eq!(decoded.header.sequence, 1);
    }
}

//! Packet forwarding logic.

use crate::error::Result;
use crate::room::Room;
use bytes::Bytes;
use tracing::debug;
use ussp::protocol::{Packet, PacketType};

/// Packet forwarder.
pub struct Forwarder {
    /// Forward video frames.
    pub forward_video: bool,
    /// Forward audio frames.
    pub forward_audio: bool,
    /// Forward DMX frames.
    pub forward_dmx: bool,
    /// Forward motion capture frames.
    pub forward_mocap: bool,
    /// Forward sync points.
    pub forward_sync: bool,
}

impl Default for Forwarder {
    fn default() -> Self {
        Self {
            forward_video: true,
            forward_audio: true,
            forward_dmx: true,
            forward_mocap: true,
            forward_sync: true,
        }
    }
}

impl Forwarder {
    /// Create a new forwarder with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Check if a packet type should be forwarded.
    pub fn should_forward(&self, packet_type: PacketType) -> bool {
        match packet_type {
            PacketType::VideoFrame => self.forward_video,
            PacketType::AudioFrame => self.forward_audio,
            PacketType::DmxFrame => self.forward_dmx,
            PacketType::MocapFrame => self.forward_mocap,
            PacketType::SyncPoint => self.forward_sync,
            PacketType::FecRepair => true, // Always forward FEC
            PacketType::SessionInit => false,
            PacketType::SessionAck => false,
            PacketType::Heartbeat => false,
        }
    }

    /// Forward a packet to all receivers in a room.
    pub async fn forward(
        &self,
        room: &Room,
        data: &Bytes,
        sender_id: Option<&str>,
    ) -> Result<ForwardResult> {
        // Parse packet to check type
        let packet = Packet::decode(data.clone())?;

        if !self.should_forward(packet.header.packet_type) {
            return Ok(ForwardResult {
                forwarded: false,
                recipients: 0,
                packet_type: packet.header.packet_type,
            });
        }

        // Forward to all receivers
        room.forward_to_receivers(data, sender_id.map(String::from).as_ref())
            .await;

        let recipients = room.receiver_count().await;

        debug!(
            room_id = %room.id,
            packet_type = ?packet.header.packet_type,
            recipients = recipients,
            "Packet forwarded"
        );

        Ok(ForwardResult {
            forwarded: true,
            recipients,
            packet_type: packet.header.packet_type,
        })
    }

    /// Forward raw data without parsing (for performance).
    pub async fn forward_raw(
        &self,
        room: &Room,
        data: &Bytes,
        sender_id: Option<&str>,
    ) -> Result<usize> {
        room.forward_to_receivers(data, sender_id.map(String::from).as_ref())
            .await;
        Ok(room.receiver_count().await)
    }
}

/// Result of forwarding a packet.
#[derive(Debug)]
pub struct ForwardResult {
    /// Whether the packet was forwarded.
    pub forwarded: bool,
    /// Number of recipients.
    pub recipients: usize,
    /// Packet type.
    pub packet_type: PacketType,
}

/// Selective forwarding configuration.
#[derive(Debug, Clone)]
pub struct SelectiveForwardConfig {
    /// Forward only key frames for video.
    pub video_keyframes_only: bool,
    /// Maximum video bitrate (0 = unlimited).
    pub max_video_bitrate: u64,
    /// Skip audio tracks.
    pub skip_audio_tracks: Vec<u16>,
    /// Skip DMX universes.
    pub skip_dmx_universes: Vec<u16>,
}

impl Default for SelectiveForwardConfig {
    fn default() -> Self {
        Self {
            video_keyframes_only: false,
            max_video_bitrate: 0,
            skip_audio_tracks: Vec::new(),
            skip_dmx_universes: Vec::new(),
        }
    }
}

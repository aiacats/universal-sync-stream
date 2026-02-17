//! USSP sender implementation.

use super::session::{Session, SessionConfig, SessionState};
use crate::error::{Error, Result};
use crate::fec::{FecEncoder, FecGroup};
use crate::protocol::{
    AudioPayload, DmxPayload, FecRepairPayload, Flags, MocapPayload, Packet, PacketHeader,
    PacketType, Payload, SessionInitPayload, SyncPoint, VideoPayload, MAX_PAYLOAD_SIZE,
};
use bytes::Bytes;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::Mutex;
use tracing::{debug, info};

/// Sender configuration.
#[derive(Debug, Clone)]
pub struct SenderConfig {
    /// Session configuration.
    pub session: SessionConfig,
    /// Enable FEC.
    pub fec_enabled: bool,
    /// Maximum fragment size for video frames.
    pub max_fragment_size: usize,
    /// Sync point interval in microseconds.
    pub sync_interval_us: u64,
}

impl Default for SenderConfig {
    fn default() -> Self {
        Self {
            session: SessionConfig::default(),
            fec_enabled: true,
            max_fragment_size: MAX_PAYLOAD_SIZE - 64, // Leave room for headers
            sync_interval_us: 1_000_000, // 1 second
        }
    }
}

/// USSP sender for streaming video and audio.
pub struct Sender {
    socket: Arc<UdpSocket>,
    dest_addr: SocketAddr,
    session: Arc<Mutex<Session>>,
    config: SenderConfig,
    fec_encoder: Option<Mutex<FecEncoder>>,
    pending_fec_packets: Mutex<Vec<Bytes>>,
    last_sync_time: Mutex<u64>,
    frame_id: Mutex<u32>,
}

impl Sender {
    /// Bind to a local address and connect to a destination.
    pub async fn bind(
        local_addr: &str,
        dest_addr: &str,
        config: SenderConfig,
    ) -> Result<Self> {
        let socket = UdpSocket::bind(local_addr).await?;
        let dest_addr: SocketAddr = dest_addr
            .parse()
            .map_err(|e| Error::Config(format!("Invalid destination address: {}", e)))?;

        let fec_encoder = if config.fec_enabled {
            Some(Mutex::new(FecEncoder::new(config.session.fec)?))
        } else {
            None
        };

        let session = Session::new(config.session.clone());

        info!(
            "USSP sender bound to {}, destination {}",
            socket.local_addr()?,
            dest_addr
        );

        Ok(Self {
            socket: Arc::new(socket),
            dest_addr,
            session: Arc::new(Mutex::new(session)),
            config,
            fec_encoder,
            pending_fec_packets: Mutex::new(Vec::new()),
            last_sync_time: Mutex::new(0),
            frame_id: Mutex::new(0),
        })
    }

    /// Get the current timestamp in microseconds.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    /// Initialize the session with the receiver.
    pub async fn init_session(&self) -> Result<()> {
        let mut session = self.session.lock().await;
        let config = &self.config.session;

        let init_payload = SessionInitPayload {
            session_id: session.session_id(),
            audio_track_count: config.audio_track_count,
            video_codec: config.video_codec,
            audio_codec: config.audio_codec,
            fec_data_shards: config.fec.data_shards as u16,
            fec_parity_shards: config.fec.parity_shards as u16,
            frame_rate: config.frame_rate,
            sample_rate: config.sample_rate,
        };

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::SessionInit,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            ),
            payload: Payload::SessionInit(init_payload),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());
        session.set_state(SessionState::Active);
        session.touch();

        info!("Session initialized: {}", session.session_id());
        Ok(())
    }

    /// Send a video frame, fragmenting if necessary.
    pub async fn send_video(&self, payload: VideoPayload) -> Result<()> {
        let session = self.session.lock().await;
        session.validate_active()?;
        let session_id = session.session_id();
        let timestamp = Self::current_timestamp();
        drop(session);

        // Check if we need to fragment
        if payload.data.len() <= self.config.max_fragment_size {
            // Single packet
            self.send_video_packet(payload, session_id, timestamp).await
        } else {
            // Fragment the frame
            self.send_fragmented_video(payload, session_id, timestamp)
                .await
        }
    }

    /// Send a video frame that fits in a single packet.
    async fn send_video_packet(
        &self,
        payload: VideoPayload,
        session_id: u32,
        timestamp: u64,
    ) -> Result<()> {
        let session = self.session.lock().await;
        let seq = session.next_sequence();

        let mut flags = Flags::empty();
        if payload.is_key_frame {
            flags |= Flags::KEY_FRAME;
        }
        if self.config.fec_enabled {
            flags |= Flags::FEC_PROTECTED;
        }

        let packet = Packet {
            header: PacketHeader::new(PacketType::VideoFrame, session_id, seq, timestamp)
                .with_flags(flags),
            payload: Payload::Video(payload),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());

        // Add to FEC group if enabled
        if self.config.fec_enabled {
            self.add_to_fec_group(data).await?;
        }

        Ok(())
    }

    /// Send a fragmented video frame.
    async fn send_fragmented_video(
        &self,
        payload: VideoPayload,
        session_id: u32,
        timestamp: u64,
    ) -> Result<()> {
        let chunks: Vec<Bytes> = payload
            .data
            .chunks(self.config.max_fragment_size)
            .map(Bytes::copy_from_slice)
            .collect();

        let total_fragments = chunks.len() as u16;

        for (i, chunk) in chunks.into_iter().enumerate() {
            let fragment = VideoPayload::new_fragment(
                payload.frame_id,
                i as u16,
                total_fragments,
                payload.codec,
                payload.is_key_frame,
                payload.pts,
                payload.dts,
                chunk,
            );

            self.send_video_packet(fragment, session_id, timestamp)
                .await?;
        }

        Ok(())
    }

    /// Send an audio frame.
    pub async fn send_audio(&self, payload: AudioPayload) -> Result<()> {
        let session = self.session.lock().await;
        session.validate_active()?;

        let mut flags = Flags::empty();
        if self.config.fec_enabled {
            flags |= Flags::FEC_PROTECTED;
        }

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::AudioFrame,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            )
            .with_flags(flags),
            payload: Payload::Audio(payload),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());
        drop(session);

        if self.config.fec_enabled {
            self.add_to_fec_group(data).await?;
        }

        Ok(())
    }

    /// Send a DMX frame (Art-Net compatible).
    pub async fn send_dmx(&self, payload: DmxPayload) -> Result<()> {
        let session = self.session.lock().await;
        session.validate_active()?;

        let mut flags = Flags::empty();
        if self.config.fec_enabled {
            flags |= Flags::FEC_PROTECTED;
        }

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::DmxFrame,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            )
            .with_flags(flags),
            payload: Payload::Dmx(payload),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());
        drop(session);

        if self.config.fec_enabled {
            self.add_to_fec_group(data).await?;
        }

        debug!("DMX frame sent");
        Ok(())
    }

    /// Send a motion capture frame (NatNet compatible).
    pub async fn send_mocap(&self, payload: MocapPayload) -> Result<()> {
        let session = self.session.lock().await;
        session.validate_active()?;

        let mut flags = Flags::empty();
        if self.config.fec_enabled {
            flags |= Flags::FEC_PROTECTED;
        }

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::MocapFrame,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            )
            .with_flags(flags),
            payload: Payload::Mocap(payload),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());
        drop(session);

        if self.config.fec_enabled {
            self.add_to_fec_group(data).await?;
        }

        debug!("Mocap frame sent");
        Ok(())
    }

    /// Send a sync point.
    pub async fn send_sync_point(&self, sync: SyncPoint) -> Result<()> {
        let session = self.session.lock().await;
        session.validate_active()?;

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::SyncPoint,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            ),
            payload: Payload::Sync(sync),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());

        *self.last_sync_time.lock().await = Self::current_timestamp();

        debug!("Sync point sent");
        Ok(())
    }

    /// Send a heartbeat.
    pub async fn send_heartbeat(&self) -> Result<()> {
        let session = self.session.lock().await;

        let packet = Packet {
            header: PacketHeader::new(
                PacketType::Heartbeat,
                session.session_id(),
                session.next_sequence(),
                Self::current_timestamp(),
            ),
            payload: Payload::Heartbeat,
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, self.dest_addr).await?;
        session.record_sent(data.len());

        Ok(())
    }

    /// Add a packet to the FEC group.
    async fn add_to_fec_group(&self, data: Bytes) -> Result<()> {
        let encoder = match &self.fec_encoder {
            Some(e) => e,
            None => return Ok(()),
        };

        let mut pending = self.pending_fec_packets.lock().await;
        pending.push(data);

        let data_shards = self.config.session.fec.data_shards;
        if pending.len() >= data_shards {
            // Encode FEC group
            let packets: Vec<Bytes> = pending.drain(..).collect();
            drop(pending);

            let mut enc = encoder.lock().await;
            let group = enc.encode(packets)?;
            drop(enc);

            // Send parity shards
            self.send_fec_repair(&group).await?;
        }

        Ok(())
    }

    /// Send FEC repair packets.
    async fn send_fec_repair(&self, group: &FecGroup) -> Result<()> {
        let session = self.session.lock().await;

        // Send only parity shards (skip data shards)
        for (i, shard) in group.shards.iter().enumerate().skip(group.data_shards) {
            let repair = FecRepairPayload {
                group_id: group.group_id,
                index: i as u16,
                total_shards: group.shards.len() as u16,
                data_shards: group.data_shards as u16,
                data: shard.clone(),
            };

            let packet = Packet {
                header: PacketHeader::new(
                    PacketType::FecRepair,
                    session.session_id(),
                    session.next_sequence(),
                    Self::current_timestamp(),
                ),
                payload: Payload::FecRepair(repair),
            };

            let data = packet.encode()?;
            self.socket.send_to(&data, self.dest_addr).await?;
            session.record_sent(data.len());
        }

        debug!("FEC repair packets sent for group {}", group.group_id);
        Ok(())
    }

    /// Flush any remaining FEC packets.
    pub async fn flush_fec(&self) -> Result<()> {
        let encoder = match &self.fec_encoder {
            Some(e) => e,
            None => return Ok(()),
        };

        let mut pending = self.pending_fec_packets.lock().await;
        if pending.is_empty() {
            return Ok(());
        }

        let packets: Vec<Bytes> = pending.drain(..).collect();
        drop(pending);

        let mut enc = encoder.lock().await;
        let group = enc.encode_partial(packets)?;
        drop(enc);

        self.send_fec_repair(&group).await
    }

    /// Get the next frame ID.
    pub async fn next_frame_id(&self) -> u32 {
        let mut id = self.frame_id.lock().await;
        let current = *id;
        *id = id.wrapping_add(1);
        current
    }

    /// Check if a sync point should be sent.
    pub async fn should_send_sync(&self) -> bool {
        let last = *self.last_sync_time.lock().await;
        Self::current_timestamp() - last >= self.config.sync_interval_us
    }

    /// Close the session.
    pub async fn close(&self) -> Result<()> {
        // Flush remaining FEC packets
        self.flush_fec().await?;

        let mut session = self.session.lock().await;
        session.set_state(SessionState::Closed);

        info!("Session closed: {}", session.session_id());
        Ok(())
    }

    /// Get session statistics.
    pub async fn stats(&self) -> super::session::SessionStats {
        self.session.lock().await.stats()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::VideoCodecType;

    #[tokio::test]
    async fn test_sender_creation() {
        let config = SenderConfig::default();
        let sender = Sender::bind("127.0.0.1:0", "127.0.0.1:5001", config).await;
        assert!(sender.is_ok());
    }
}

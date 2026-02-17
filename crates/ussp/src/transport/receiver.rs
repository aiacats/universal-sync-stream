//! USSP receiver implementation.

use super::session::{Session, SessionConfig, SessionState};
use super::MAX_PACKET_SIZE;
use crate::error::Result;
use crate::fec::{DecodeResult, FecConfig, FecDecoder};
use crate::protocol::{
    AudioFrame, AudioPayload, DmxFrame, DmxPayload, MocapFrame, MocapPayload, Packet, PacketType,
    Payload, SessionAckPayload, SyncPoint, VideoFrame, VideoPayload,
};
use bytes::Bytes;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info, warn};

/// Receiver configuration.
#[derive(Debug, Clone)]
pub struct ReceiverConfig {
    /// Maximum pending fragments per frame.
    pub max_pending_fragments: usize,
    /// Fragment timeout.
    pub fragment_timeout: Duration,
    /// Enable FEC decoding.
    pub fec_enabled: bool,
    /// FEC configuration (must match sender).
    pub fec: FecConfig,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            max_pending_fragments: 100,
            fragment_timeout: Duration::from_secs(1),
            fec_enabled: true,
            fec: FecConfig::default(),
        }
    }
}

/// Events emitted by the receiver.
#[derive(Debug)]
pub enum ReceiverEvent {
    /// A complete video frame was received.
    VideoFrame(VideoFrame),
    /// An audio frame was received.
    AudioFrame(AudioFrame),
    /// A DMX frame was received (Art-Net compatible).
    DmxFrame(DmxFrame),
    /// A motion capture frame was received (NatNet compatible).
    MocapFrame(MocapFrame),
    /// A sync point was received.
    SyncPoint(SyncPoint),
    /// Session was initialized.
    SessionInit {
        session_id: u32,
        audio_tracks: u16,
    },
    /// Connection was lost.
    Disconnected,
}

/// Pending video frame fragments.
struct PendingFrame {
    fragments: HashMap<u16, VideoPayload>,
    total_fragments: u16,
    received_at: std::time::Instant,
}

/// USSP receiver for receiving video and audio streams.
pub struct Receiver {
    /// The UDP socket.
    pub socket: Arc<UdpSocket>,
    session: Arc<RwLock<Option<Session>>>,
    config: ReceiverConfig,
    fec_decoder: Option<RwLock<FecDecoder>>,
    pending_frames: RwLock<HashMap<u32, PendingFrame>>,
    event_tx: mpsc::Sender<ReceiverEvent>,
    event_rx: tokio::sync::Mutex<mpsc::Receiver<ReceiverEvent>>,
    /// Running flag for graceful shutdown.
    running: Arc<AtomicBool>,
}

impl Receiver {
    /// Bind to a local address.
    pub async fn bind(local_addr: &str, config: ReceiverConfig) -> Result<Self> {
        let socket = UdpSocket::bind(local_addr).await?;

        let fec_decoder = if config.fec_enabled {
            Some(RwLock::new(FecDecoder::new(config.fec)?))
        } else {
            None
        };

        let (event_tx, event_rx) = mpsc::channel(1000);

        info!("USSP receiver bound to {}", socket.local_addr()?);

        Ok(Self {
            socket: Arc::new(socket),
            session: Arc::new(RwLock::new(None)),
            config,
            fec_decoder,
            pending_frames: RwLock::new(HashMap::new()),
            event_tx,
            event_rx: tokio::sync::Mutex::new(event_rx),
            running: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Get the current timestamp in microseconds.
    fn current_timestamp() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    /// Receive the next event.
    pub async fn recv(&self) -> Option<ReceiverEvent> {
        self.event_rx.lock().await.recv().await
    }

    /// Start receiving packets.
    pub async fn run(&self) -> Result<()> {
        self.running.store(true, Ordering::SeqCst);
        let mut buf = vec![0u8; MAX_PACKET_SIZE];

        while self.running.load(Ordering::SeqCst) {
            // Use timeout to periodically check the running flag
            match tokio::time::timeout(
                Duration::from_millis(100),
                self.socket.recv_from(&mut buf),
            )
            .await
            {
                Ok(Ok((len, addr))) => {
                    let data = Bytes::copy_from_slice(&buf[..len]);
                    if let Err(e) = self.handle_packet(data, addr).await {
                        warn!("Error handling packet: {}", e);
                    }
                }
                Ok(Err(e)) => {
                    // Socket error - might be closed
                    if self.running.load(Ordering::SeqCst) {
                        warn!("Socket error: {}", e);
                    }
                    break;
                }
                Err(_) => {
                    // Timeout - continue to check running flag
                    continue;
                }
            }
        }

        info!("Receiver stopped");
        Ok(())
    }

    /// Stop the receiver.
    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
        // Send disconnect event
        let tx = self.event_tx.clone();
        tokio::spawn(async move {
            let _ = tx.send(ReceiverEvent::Disconnected).await;
        });
    }

    /// Check if the receiver is running.
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    /// Handle a received packet.
    async fn handle_packet(&self, data: Bytes, addr: SocketAddr) -> Result<()> {
        let packet = Packet::decode(data.clone())?;

        // Update session activity
        if let Some(session) = self.session.write().await.as_mut() {
            session.touch();
            session.record_received(data.len());
        }

        match packet.payload {
            Payload::Video(payload) => {
                self.handle_video(payload).await?;
            }
            Payload::Audio(payload) => {
                self.handle_audio(payload).await?;
            }
            Payload::Dmx(payload) => {
                self.handle_dmx(payload).await?;
            }
            Payload::Mocap(payload) => {
                self.handle_mocap(payload).await?;
            }
            Payload::Sync(sync) => {
                self.event_tx
                    .send(ReceiverEvent::SyncPoint(sync))
                    .await
                    .ok();
            }
            Payload::FecRepair(repair) => {
                self.handle_fec_repair(repair).await?;
            }
            Payload::SessionInit(init) => {
                self.handle_session_init(init, addr).await?;
            }
            Payload::SessionAck(_) => {
                // Sender shouldn't receive ACKs normally
            }
            Payload::Heartbeat => {
                // Just update last activity, already done above
                debug!("Heartbeat received from {}", addr);
            }
        }

        // Add to FEC decoder if enabled
        if self.config.fec_enabled && packet.header.packet_type != PacketType::FecRepair {
            self.add_to_fec_decoder(data).await;
        }

        Ok(())
    }

    /// Handle session initialization.
    async fn handle_session_init(
        &self,
        init: crate::protocol::SessionInitPayload,
        addr: SocketAddr,
    ) -> Result<()> {
        let config = SessionConfig {
            session_id: init.session_id,
            audio_track_count: init.audio_track_count,
            video_codec: init.video_codec,
            audio_codec: init.audio_codec,
            fec: FecConfig::new(init.fec_data_shards as usize, init.fec_parity_shards as usize),
            frame_rate: init.frame_rate,
            sample_rate: init.sample_rate,
            ..Default::default()
        };

        let mut session = Session::new(config);
        session.set_remote_addr(addr);
        session.set_state(SessionState::Active);

        *self.session.write().await = Some(session);

        // Send ACK
        self.send_session_ack(init.session_id, addr).await?;

        self.event_tx
            .send(ReceiverEvent::SessionInit {
                session_id: init.session_id,
                audio_tracks: init.audio_track_count,
            })
            .await
            .ok();

        info!("Session initialized: {}", init.session_id);
        Ok(())
    }

    /// Send session acknowledgment.
    async fn send_session_ack(&self, session_id: u32, addr: SocketAddr) -> Result<()> {
        let ack = SessionAckPayload {
            session_id,
            accepted: true,
            server_timestamp: Self::current_timestamp(),
        };

        let packet = Packet {
            header: crate::protocol::PacketHeader::new(
                PacketType::SessionAck,
                session_id,
                0,
                Self::current_timestamp(),
            ),
            payload: Payload::SessionAck(ack),
        };

        let data = packet.encode()?;
        self.socket.send_to(&data, addr).await?;
        Ok(())
    }

    /// Handle a video payload.
    async fn handle_video(&self, payload: VideoPayload) -> Result<()> {
        if payload.is_complete() {
            // Single fragment, emit immediately
            self.event_tx
                .send(ReceiverEvent::VideoFrame(payload.into()))
                .await
                .ok();
        } else {
            // Fragmented frame
            self.handle_video_fragment(payload).await?;
        }
        Ok(())
    }

    /// Handle a video fragment.
    async fn handle_video_fragment(&self, payload: VideoPayload) -> Result<()> {
        let frame_id = payload.frame_id;
        let fragment_index = payload.fragment_index;
        let total_fragments = payload.total_fragments;

        let mut pending = self.pending_frames.write().await;

        let frame = pending.entry(frame_id).or_insert_with(|| PendingFrame {
            fragments: HashMap::new(),
            total_fragments,
            received_at: std::time::Instant::now(),
        });

        frame.fragments.insert(fragment_index, payload);

        // Check if frame is complete
        if frame.fragments.len() == total_fragments as usize {
            let fragments = pending.remove(&frame_id).unwrap();
            drop(pending);

            // Reassemble frame
            if let Some(video_frame) = self.reassemble_frame(fragments) {
                self.event_tx
                    .send(ReceiverEvent::VideoFrame(video_frame))
                    .await
                    .ok();
            }
        } else {
            // Clean up old pending frames
            pending.retain(|_, f| f.received_at.elapsed() < self.config.fragment_timeout);
        }

        Ok(())
    }

    /// Reassemble a fragmented frame.
    fn reassemble_frame(&self, pending: PendingFrame) -> Option<VideoFrame> {
        let mut fragments: Vec<_> = pending.fragments.into_iter().collect();
        fragments.sort_by_key(|(idx, _)| *idx);

        if fragments.len() != pending.total_fragments as usize {
            warn!("Incomplete frame: missing fragments");
            return None;
        }

        // Extract metadata from first fragment before consuming
        let first = &fragments[0].1;
        let frame_id = first.frame_id;
        let codec = first.codec;
        let is_key_frame = first.is_key_frame;
        let pts = first.pts;
        let dts = first.dts;

        let mut data = Vec::new();
        for (_, frag) in fragments {
            data.extend_from_slice(&frag.data);
        }

        Some(VideoFrame {
            frame_id,
            codec,
            is_key_frame,
            pts,
            dts,
            data: Bytes::from(data),
        })
    }

    /// Handle an audio payload.
    async fn handle_audio(&self, payload: AudioPayload) -> Result<()> {
        self.event_tx
            .send(ReceiverEvent::AudioFrame(payload.into()))
            .await
            .ok();
        Ok(())
    }

    /// Handle a DMX payload.
    async fn handle_dmx(&self, payload: DmxPayload) -> Result<()> {
        self.event_tx
            .send(ReceiverEvent::DmxFrame(payload.into()))
            .await
            .ok();
        Ok(())
    }

    /// Handle a motion capture payload.
    async fn handle_mocap(&self, payload: MocapPayload) -> Result<()> {
        self.event_tx
            .send(ReceiverEvent::MocapFrame(payload.into()))
            .await
            .ok();
        Ok(())
    }

    /// Handle FEC repair packet.
    async fn handle_fec_repair(
        &self,
        repair: crate::protocol::FecRepairPayload,
    ) -> Result<()> {
        if let Some(decoder) = &self.fec_decoder {
            let mut dec = decoder.write().await;
            let data_len = repair.data.len();
            let result = dec.add_shard(
                repair.group_id,
                repair.index as usize,
                repair.data,
                data_len,
            );

            if let DecodeResult::Recovered(shards) = result {
                debug!("FEC recovered {} shards for group {}", shards.len(), repair.group_id);
                // Process recovered packets
                for shard in shards {
                    if let Ok(packet) = Packet::decode(shard) {
                        match packet.payload {
                            Payload::Video(v) => {
                                self.event_tx
                                    .send(ReceiverEvent::VideoFrame(v.into()))
                                    .await
                                    .ok();
                            }
                            Payload::Audio(a) => {
                                self.event_tx
                                    .send(ReceiverEvent::AudioFrame(a.into()))
                                    .await
                                    .ok();
                            }
                            Payload::Dmx(d) => {
                                self.event_tx
                                    .send(ReceiverEvent::DmxFrame(d.into()))
                                    .await
                                    .ok();
                            }
                            Payload::Mocap(m) => {
                                self.event_tx
                                    .send(ReceiverEvent::MocapFrame(m.into()))
                                    .await
                                    .ok();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Add packet to FEC decoder.
    async fn add_to_fec_decoder(&self, _data: Bytes) {
        // In a real implementation, we'd track which FEC group this packet
        // belongs to based on sequence numbers. For simplicity, we skip this
        // as the repair packets contain the necessary information.
    }

    /// Get session statistics.
    pub fn stats(&self) -> Option<super::session::SessionStats> {
        self.session.try_read().ok().and_then(|s| s.as_ref().map(|s| s.stats()))
    }

    /// Check if a session is active.
    pub fn is_active(&self) -> bool {
        self.session
            .try_read()
            .ok()
            .map(|s| s.as_ref().is_some_and(|s| s.state() == SessionState::Active))
            .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_receiver_creation() {
        let config = ReceiverConfig::default();
        let receiver = Receiver::bind("127.0.0.1:0", config).await;
        assert!(receiver.is_ok());
    }
}

//! Receiver logic for the USSP tester.

use crate::state::{AppState, ReceiverStats};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::interval;
use ussp::transport::{Receiver, ReceiverConfig, ReceiverEvent};

/// Video frame data for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct VideoFrameData {
    pub width: u32,
    pub height: u32,
    pub pts: u64,
    pub is_key_frame: bool,
    pub data_base64: String,
}

/// Audio level data for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct AudioLevelData {
    pub track_id: u16,
    pub level_db: f32,
    pub pts: u64,
}

/// Sync point data for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SyncPointData {
    pub video_pts: u64,
    pub audio_pts: Vec<u64>,
    pub av_diff_us: i64,
}

/// DMX frame data for frontend.
#[derive(Debug, Clone, serde::Serialize)]
pub struct DmxFrameData {
    pub universe: u16,
    pub channel_count: u16,
    pub pts: u64,
    pub channels: Vec<u8>,
}

/// Calculate audio level in dB.
fn calculate_audio_level(data: &[u8]) -> f32 {
    if data.is_empty() {
        return -60.0;
    }

    // Assume 16-bit stereo PCM
    let samples: Vec<i16> = data
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();

    if samples.is_empty() {
        return -60.0;
    }

    // RMS calculation
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64).powi(2)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt();

    // Convert to dB
    if rms > 0.0 {
        20.0 * (rms / 32768.0).log10() as f32
    } else {
        -60.0
    }
}

/// Start the receiver.
pub async fn start_receiver(
    app: AppHandle,
    state: Arc<AppState>,
    port: u16,
) -> Result<(), String> {
    // Check if already running
    {
        let receiver_state = state.receiver.read();
        if receiver_state.is_running {
            return Err("Receiver is already running".to_string());
        }
    }

    let local_addr = format!("0.0.0.0:{}", port);

    let config = ReceiverConfig::default();
    let receiver = Arc::new(
        Receiver::bind(&local_addr, config)
            .await
            .map_err(|e| format!("Failed to bind receiver: {}", e))?,
    );

    // Create stop channel
    let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

    // Update state
    {
        let mut receiver_state = state.receiver.write();
        receiver_state.is_running = true;
        receiver_state.stop_tx = Some(stop_tx);
        receiver_state.stats = ReceiverStats::default();
        receiver_state.receiver = Some(Arc::clone(&receiver));
    }

    // Clone for tasks
    let state_clone = Arc::clone(&state);
    let app_clone = app.clone();
    let receiver_clone = Arc::clone(&receiver);

    // Start receiver loop
    tokio::spawn(async move {
        if let Err(e) = receiver_clone.run().await {
            tracing::warn!("Receiver error: {}", e);
        }
    });

    // Event processing task
    tokio::spawn(async move {
        let mut stats_ticker = interval(Duration::from_millis(500));

        let video_frames_received = Arc::new(AtomicU64::new(0));
        let audio_frames_received = Arc::new(AtomicU64::new(0));
        let last_video_pts = Arc::new(AtomicU64::new(0));
        let last_audio_pts = Arc::new(AtomicU64::new(0));

        let start_time = Instant::now();

        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    break;
                }

                event = receiver.recv() => {
                    match event {
                        Some(ReceiverEvent::VideoFrame(frame)) => {
                            video_frames_received.fetch_add(1, Ordering::Relaxed);
                            last_video_pts.store(frame.pts, Ordering::Relaxed);

                            // Downsample for preview (every 3rd frame)
                            if video_frames_received.load(Ordering::Relaxed) % 3 == 0 {
                                let frame_data = VideoFrameData {
                                    width: 640, // Assumed
                                    height: 480,
                                    pts: frame.pts,
                                    is_key_frame: frame.is_key_frame,
                                    data_base64: BASE64.encode(&frame.data),
                                };

                                let _ = app_clone.emit("ussp://video-frame", frame_data);
                            }
                        }

                        Some(ReceiverEvent::AudioFrame(frame)) => {
                            audio_frames_received.fetch_add(1, Ordering::Relaxed);
                            last_audio_pts.store(frame.pts, Ordering::Relaxed);

                            // Calculate and emit audio level
                            let level = calculate_audio_level(&frame.data);
                            let level_data = AudioLevelData {
                                track_id: frame.track_id,
                                level_db: level,
                                pts: frame.pts,
                            };

                            let _ = app_clone.emit("ussp://audio-level", level_data);
                        }

                        Some(ReceiverEvent::DmxFrame(frame)) => {
                            let dmx_data = DmxFrameData {
                                universe: frame.universe,
                                channel_count: frame.data.len() as u16,
                                pts: frame.pts,
                                channels: frame.data.to_vec(),
                            };

                            let _ = app_clone.emit("ussp://dmx-frame", dmx_data);
                        }

                        Some(ReceiverEvent::SyncPoint(sync)) => {
                            let audio_pts: Vec<u64> = sync.audio_tracks.iter().map(|t| t.pts).collect();
                            let av_diff = if !audio_pts.is_empty() {
                                sync.video_pts as i64 - audio_pts[0] as i64
                            } else {
                                0
                            };

                            let sync_data = SyncPointData {
                                video_pts: sync.video_pts,
                                audio_pts,
                                av_diff_us: av_diff,
                            };

                            let _ = app_clone.emit("ussp://sync-point", sync_data);
                        }

                        Some(ReceiverEvent::SessionInit { session_id, audio_tracks }) => {
                            let _ = app_clone.emit("ussp://session-init", serde_json::json!({
                                "session_id": session_id,
                                "audio_tracks": audio_tracks,
                            }));
                        }

                        Some(ReceiverEvent::Disconnected) => {
                            let _ = app_clone.emit("ussp://disconnected", ());
                            break;
                        }

                        None => {
                            break;
                        }
                    }
                }

                _ = stats_ticker.tick() => {
                    let session_stats = receiver.stats();
                    let elapsed = start_time.elapsed().as_secs_f64();

                    let vf = video_frames_received.load(Ordering::Relaxed);
                    let af = audio_frames_received.load(Ordering::Relaxed);
                    let vpts = last_video_pts.load(Ordering::Relaxed);
                    let apts = last_audio_pts.load(Ordering::Relaxed);

                    let fps = if elapsed > 0.0 {
                        vf as f64 / elapsed
                    } else {
                        0.0
                    };

                    let stats = ReceiverStats {
                        packets_received: session_stats.map(|s| s.packets_received).unwrap_or(0),
                        bytes_received: session_stats.map(|s| s.bytes_received).unwrap_or(0),
                        video_frames_received: vf,
                        audio_frames_received: af,
                        packet_loss_rate: 0.0, // TODO: Calculate from FEC
                        av_sync_diff_us: vpts as i64 - apts as i64,
                        fps,
                    };

                    // Update state
                    {
                        let mut receiver_state = state_clone.receiver.write();
                        receiver_state.stats = stats.clone();
                    }

                    // Emit event
                    let _ = app_clone.emit("ussp://receiver-stats", stats);
                }
            }
        }

        // Cleanup
        {
            let mut receiver_state = state_clone.receiver.write();
            receiver_state.is_running = false;
            receiver_state.stop_tx = None;
        }
    });

    Ok(())
}

/// Stop the receiver.
pub async fn stop_receiver(state: Arc<AppState>) -> Result<(), String> {
    // Get the receiver and stop channel
    let (stop_tx, receiver) = {
        let receiver_state = state.receiver.read();
        (receiver_state.stop_tx.clone(), receiver_state.receiver.clone())
    };

    // Stop the receiver
    if let Some(recv) = receiver {
        recv.stop();
    }

    // Send stop signal to event processing task
    if let Some(tx) = stop_tx {
        let _ = tx.send(()).await;
    }

    // Wait a bit for cleanup
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Clear state
    {
        let mut receiver_state = state.receiver.write();
        receiver_state.is_running = false;
        receiver_state.stop_tx = None;
        receiver_state.receiver = None;
    }

    Ok(())
}

/// Get receiver statistics.
pub fn get_receiver_stats(state: Arc<AppState>) -> ReceiverStats {
    state.receiver.read().stats.clone()
}

//! Sender logic for the USSP tester.

use crate::state::{AppState, SenderStats};
use bytes::Bytes;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use tokio::sync::mpsc;
use tokio::time::interval;
use ussp::protocol::{AudioCodecType, AudioPayload, SyncPoint, VideoCodecType, VideoPayload};
use ussp::transport::{Sender, SenderConfig};

/// Sender options from frontend.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SenderOptions {
    pub audio_tracks: u8,
    pub fec_enabled: bool,
    pub fps: u32,
    pub width: u32,
    pub height: u32,
}

impl Default for SenderOptions {
    fn default() -> Self {
        Self {
            audio_tracks: 2,
            fec_enabled: true,
            fps: 30,
            width: 640,
            height: 480,
        }
    }
}

/// Generate a test pattern frame (color bars).
fn generate_test_pattern(width: u32, height: u32, frame_num: u32) -> Vec<u8> {
    let mut data = Vec::with_capacity((width * height * 3) as usize);

    // Simple color bars pattern
    let bar_width = width / 8;
    let colors: [(u8, u8, u8); 8] = [
        (255, 255, 255), // White
        (255, 255, 0),   // Yellow
        (0, 255, 255),   // Cyan
        (0, 255, 0),     // Green
        (255, 0, 255),   // Magenta
        (255, 0, 0),     // Red
        (0, 0, 255),     // Blue
        (0, 0, 0),       // Black
    ];

    for y in 0..height {
        for x in 0..width {
            let bar_index = ((x / bar_width) as usize).min(7);

            // Add some animation based on frame number
            let offset = ((frame_num as f32 * 0.1).sin() * 20.0) as i32;
            let y_mod = ((y as i32 + offset) as u32) % height;

            // Add moving line
            if y_mod < 4 {
                data.push(255);
                data.push(255);
                data.push(255);
            } else {
                let (r, g, b) = colors[bar_index];
                data.push(r);
                data.push(g);
                data.push(b);
            }
        }
    }

    data
}

/// Generate test audio samples (sine wave).
fn generate_test_audio(sample_rate: u32, samples: u32, frequency: f32, track_id: u16) -> Vec<u8> {
    let mut data = Vec::with_capacity((samples * 2 * 2) as usize); // 16-bit stereo

    for i in 0..samples {
        let t = i as f32 / sample_rate as f32;
        // Different frequency for each track
        let freq = frequency * (1.0 + track_id as f32 * 0.5);
        let sample = (t * freq * 2.0 * std::f32::consts::PI).sin();
        let sample_i16 = (sample * 16000.0) as i16;

        // Left and right channel
        data.extend_from_slice(&sample_i16.to_le_bytes());
        data.extend_from_slice(&sample_i16.to_le_bytes());
    }

    data
}

/// Start the sender.
pub async fn start_sender(
    app: AppHandle,
    state: Arc<AppState>,
    dest: String,
    port: u16,
    options: SenderOptions,
) -> Result<(), String> {
    // Check if already running
    {
        let sender_state = state.sender.read();
        if sender_state.is_running {
            return Err("Sender is already running".to_string());
        }
    }

    let local_addr = "0.0.0.0:0";
    let dest_addr = format!("{}:{}", dest, port);

    let config = SenderConfig {
        fec_enabled: options.fec_enabled,
        ..Default::default()
    };

    let sender = Sender::bind(local_addr, &dest_addr, config)
        .await
        .map_err(|e| format!("Failed to bind sender: {}", e))?;

    sender
        .init_session()
        .await
        .map_err(|e| format!("Failed to init session: {}", e))?;

    // Create stop channel
    let (stop_tx, mut stop_rx) = mpsc::channel::<()>(1);

    // Update state
    {
        let mut sender_state = state.sender.write();
        sender_state.is_running = true;
        sender_state.stop_tx = Some(stop_tx);
        sender_state.stats = SenderStats::default();
    }

    // Clone for the task
    let state_clone = Arc::clone(&state);
    let app_clone = app.clone();

    // Spawn sender task
    tokio::spawn(async move {
        let frame_interval = Duration::from_millis(1000 / options.fps as u64);
        let audio_interval = Duration::from_millis(20); // 20ms audio frames
        let sync_interval = Duration::from_secs(1);
        let stats_interval = Duration::from_millis(500);

        let mut video_ticker = interval(frame_interval);
        let mut audio_ticker = interval(audio_interval);
        let mut sync_ticker = interval(sync_interval);
        let mut stats_ticker = interval(stats_interval);

        let mut video_pts: u64 = 0;
        let mut audio_pts: u64 = 0;
        let mut frame_count: u32 = 0;
        let mut video_frames_sent: u64 = 0;
        let mut audio_frames_sent: u64 = 0;

        let start_time = Instant::now();

        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    break;
                }

                _ = video_ticker.tick() => {
                    // Generate test pattern
                    let pattern = generate_test_pattern(options.width, options.height, frame_count);

                    // In real usage, this would be encoded. For testing, we send raw.
                    let frame_id = sender.next_frame_id().await;
                    let is_key_frame = frame_count % 30 == 0;

                    let payload = VideoPayload::new(
                        frame_id,
                        VideoCodecType::Raw,
                        is_key_frame,
                        video_pts,
                        video_pts,
                        Bytes::from(pattern),
                    );

                    if sender.send_video(payload).await.is_ok() {
                        video_frames_sent += 1;
                    }

                    video_pts += 1_000_000 / options.fps as u64;
                    frame_count += 1;
                }

                _ = audio_ticker.tick() => {
                    // Send audio for each track
                    for track_id in 0..options.audio_tracks as u16 {
                        let audio_data = generate_test_audio(48000, 960, 440.0, track_id);

                        let payload = AudioPayload::new(
                            track_id,
                            48000,
                            2,
                            16,
                            AudioCodecType::Pcm,
                            audio_pts,
                            960,
                            Bytes::from(audio_data),
                        );

                        if sender.send_audio(payload).await.is_ok() {
                            audio_frames_sent += 1;
                        }
                    }

                    audio_pts += 20_000; // 20ms
                }

                _ = sync_ticker.tick() => {
                    let mut sync = SyncPoint::new(
                        ussp::sync::ClockSync::current_time_us(),
                        video_pts,
                    );

                    for track_id in 0..options.audio_tracks as u16 {
                        sync.add_track(track_id, audio_pts, 0);
                    }

                    let _ = sender.send_sync_point(sync).await;
                }

                _ = stats_ticker.tick() => {
                    let session_stats = sender.stats().await;
                    let elapsed = start_time.elapsed().as_secs_f64();
                    let fps = if elapsed > 0.0 {
                        video_frames_sent as f64 / elapsed
                    } else {
                        0.0
                    };

                    let stats = SenderStats {
                        packets_sent: session_stats.packets_sent,
                        bytes_sent: session_stats.bytes_sent,
                        video_frames_sent,
                        audio_frames_sent,
                        fps,
                    };

                    // Update state
                    {
                        let mut sender_state = state_clone.sender.write();
                        sender_state.stats = stats.clone();
                    }

                    // Emit event
                    let _ = app_clone.emit("ussp://sender-stats", stats);
                }
            }
        }

        // Cleanup
        let _ = sender.close().await;

        {
            let mut sender_state = state_clone.sender.write();
            sender_state.is_running = false;
            sender_state.stop_tx = None;
        }
    });

    Ok(())
}

/// Stop the sender.
pub async fn stop_sender(state: Arc<AppState>) -> Result<(), String> {
    let stop_tx = {
        let sender_state = state.sender.read();
        sender_state.stop_tx.clone()
    };

    if let Some(tx) = stop_tx {
        let _ = tx.send(()).await;
    }

    // Wait for cleanup
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;

    // Clear state
    {
        let mut sender_state = state.sender.write();
        sender_state.is_running = false;
        sender_state.stop_tx = None;
    }

    Ok(())
}

/// Get sender statistics.
pub fn get_sender_stats(state: Arc<AppState>) -> SenderStats {
    state.sender.read().stats.clone()
}

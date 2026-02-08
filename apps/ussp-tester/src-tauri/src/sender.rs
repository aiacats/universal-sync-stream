//! Sender logic for the USSP tester.

use crate::capture::{
    AudioCaptureHandle, AudioSource, CameraCaptureHandle, CameraFrame, PixelFormat, ResolutionOption, VideoSource,
};
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
    pub resolution: ResolutionOption,
    pub video_source: VideoSource,
    pub audio_source: AudioSource,
}

impl Default for SenderOptions {
    fn default() -> Self {
        Self {
            audio_tracks: 2,
            fec_enabled: true,
            fps: 30,
            resolution: ResolutionOption::Hd720,
            video_source: VideoSource::TestPattern,
            audio_source: AudioSource::TestTone,
        }
    }
}

/// Convert BGRA to RGB.
fn bgra_to_rgb(bgra: &[u8]) -> Vec<u8> {
    let pixel_count = bgra.len() / 4;
    let mut rgb = Vec::with_capacity(pixel_count * 3);
    for chunk in bgra.chunks_exact(4) {
        rgb.push(chunk[2]); // R
        rgb.push(chunk[1]); // G
        rgb.push(chunk[0]); // B
    }
    rgb
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

/// Convert f32 samples to i16 PCM bytes.
fn convert_f32_to_pcm(samples: &[f32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(samples.len() * 2);
    for &sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let sample_i16 = (clamped * 32767.0) as i16;
        data.extend_from_slice(&sample_i16.to_le_bytes());
    }
    data
}

/// Calculate audio level in dB from samples.
fn calculate_level_db(samples: &[f32]) -> f64 {
    if samples.is_empty() {
        return -60.0;
    }

    // Calculate RMS
    let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
    let rms = (sum_sq / samples.len() as f64).sqrt();

    // Convert to dB
    if rms > 0.0 {
        (20.0 * rms.log10()).max(-60.0)
    } else {
        -60.0
    }
}

/// Downsample audio for waveform display.
fn downsample_for_display(samples: &[f32], target_samples: usize) -> Vec<f64> {
    if samples.is_empty() {
        return vec![0.0; target_samples];
    }

    let step = samples.len() / target_samples;
    if step == 0 {
        return samples.iter().map(|&s| s as f64).collect();
    }

    (0..target_samples)
        .map(|i| {
            let start = i * step;
            let end = (start + step).min(samples.len());
            let chunk = &samples[start..end];
            if chunk.is_empty() {
                0.0
            } else {
                // Use max absolute value for display
                chunk.iter().map(|&s| s.abs()).fold(0.0f32, f32::max) as f64
                    * if chunk.iter().map(|&s| s).sum::<f32>() >= 0.0 { 1.0 } else { -1.0 }
            }
        })
        .collect()
}

/// Simple bilinear resize for preview (RGB format).
fn resize_for_preview(
    src: &[u8],
    src_width: u32,
    src_height: u32,
    dst_width: u32,
    dst_height: u32,
) -> Vec<u8> {
    let mut dst = vec![0u8; (dst_width * dst_height * 3) as usize];

    let x_ratio = src_width as f32 / dst_width as f32;
    let y_ratio = src_height as f32 / dst_height as f32;

    for y in 0..dst_height {
        for x in 0..dst_width {
            let src_x = (x as f32 * x_ratio) as u32;
            let src_y = (y as f32 * y_ratio) as u32;

            let src_idx = ((src_y * src_width + src_x) * 3) as usize;
            let dst_idx = ((y * dst_width + x) * 3) as usize;

            if src_idx + 2 < src.len() && dst_idx + 2 < dst.len() {
                dst[dst_idx] = src[src_idx];
                dst[dst_idx + 1] = src[src_idx + 1];
                dst[dst_idx + 2] = src[src_idx + 2];
            }
        }
    }

    dst
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

    // Get resolution
    let (width, height) = options.resolution.dimensions();

    // Initialize camera capture if needed (in separate thread)
    let (camera_frame_tx, mut camera_frame_rx) = mpsc::unbounded_channel::<CameraFrame>();
    let camera_handle = match &options.video_source {
        VideoSource::Camera { device_name } => {
            match CameraCaptureHandle::start(device_name.clone(), options.resolution, camera_frame_tx, options.fps) {
                Ok(handle) => {
                    tracing::info!("Camera capture started: {:?}", handle.resolution());
                    Some(handle)
                }
                Err(e) => {
                    tracing::error!("Failed to start camera: {}", e);
                    let _ = app.emit("ussp://error", format!("Camera error: {}", e));
                    None
                }
            }
        }
        VideoSource::TestPattern => None,
    };

    // Get actual resolution from camera if available
    let (actual_width, actual_height) = if let Some(ref cam) = camera_handle {
        cam.resolution()
    } else {
        (width, height)
    };

    // Initialize audio capture if needed
    let (audio_tx, mut audio_rx) = mpsc::unbounded_channel::<Vec<f32>>();
    let audio_handle = match &options.audio_source {
        AudioSource::Microphone { device_index } => {
            match AudioCaptureHandle::start(*device_index, audio_tx) {
                Ok(handle) => {
                    tracing::info!("Audio capture started: {}Hz, {} channels",
                        handle.sample_rate(), handle.channels());
                    Some(handle)
                }
                Err(e) => {
                    tracing::error!("Failed to start microphone: {}", e);
                    let _ = app.emit("ussp://error", format!("Microphone error: {}", e));
                    None
                }
            }
        }
        AudioSource::TestTone => None,
    };

    let audio_sample_rate = audio_handle.as_ref().map(|h| h.sample_rate()).unwrap_or(48000);
    let audio_channels = audio_handle.as_ref().map(|h| h.channels()).unwrap_or(2);

    // Spawn sender task
    tokio::spawn(async move {
        let frame_interval = Duration::from_millis(1000 / options.fps as u64);
        let audio_interval = Duration::from_millis(20); // 20ms audio frames
        let sync_interval = Duration::from_secs(1);
        let stats_interval = Duration::from_millis(500);
        let preview_interval = Duration::from_millis(66); // ~15fps for preview (JPEG in background)

        let mut video_ticker = interval(frame_interval);
        let mut audio_ticker = interval(audio_interval);
        let mut sync_ticker = interval(sync_interval);
        let mut stats_ticker = interval(stats_interval);
        let mut preview_ticker = interval(preview_interval);

        let mut video_pts: u64 = 0;
        let mut audio_pts: u64 = 0;
        let mut frame_count: u32 = 0;
        let mut video_frames_sent: u64 = 0;
        let mut audio_frames_sent: u64 = 0;

        // Audio buffer for accumulating samples
        let mut audio_buffer: Vec<f32> = Vec::new();
        let samples_per_frame = (audio_sample_rate as usize * 20) / 1000; // 20ms worth

        // Latest camera frame
        let mut latest_camera_frame: Option<CameraFrame> = None;

        let start_time = Instant::now();

        loop {
            tokio::select! {
                _ = stop_rx.recv() => {
                    break;
                }

                // Receive camera frames
                Some(frame) = camera_frame_rx.recv(), if camera_handle.is_some() => {
                    latest_camera_frame = Some(frame);
                }

                // Receive audio samples
                Some(samples) = audio_rx.recv(), if audio_handle.is_some() => {
                    audio_buffer.extend(samples);
                }

                _ = video_ticker.tick() => {
                    // Get frame data based on source
                    let frame_data = if camera_handle.is_some() {
                        if let Some(ref frame) = latest_camera_frame {
                            // Convert BGRA to RGB if needed
                            if frame.format == PixelFormat::Bgra {
                                bgra_to_rgb(&frame.data)
                            } else {
                                frame.data.clone()
                            }
                        } else {
                            continue; // No frame available yet
                        }
                    } else {
                        generate_test_pattern(actual_width, actual_height, frame_count)
                    };

                    // Send video frame
                    let frame_id = sender.next_frame_id().await;
                    let is_key_frame = frame_count % 30 == 0;

                    let payload = VideoPayload::new(
                        frame_id,
                        VideoCodecType::Raw,
                        is_key_frame,
                        video_pts,
                        video_pts,
                        Bytes::from(frame_data),
                    );

                    if sender.send_video(payload).await.is_ok() {
                        video_frames_sent += 1;
                    }

                    video_pts += 1_000_000 / options.fps as u64;
                    frame_count += 1;
                }

                _ = preview_ticker.tick() => {
                    // Send camera preview to frontend
                    if let Some(ref frame) = latest_camera_frame {
                        let frame_data = frame.data.clone();
                        let frame_width = frame.width;
                        let frame_height = frame.height;
                        let frame_format = frame.format;
                        let app_for_preview = app_clone.clone();

                        // Spawn blocking task for resizing
                        tokio::task::spawn_blocking(move || {
                            // Fast downscale by 4x using simple sampling
                            let preview_width = frame_width / 4;
                            let preview_height = frame_height / 4;
                            let bytes_per_pixel = if frame_format == PixelFormat::Bgra { 4 } else { 3 };
                            let preview_bpp = 4; // Always output RGBA for canvas

                            let mut preview_data = vec![0u8; (preview_width * preview_height * preview_bpp) as usize];

                            for y in 0..preview_height {
                                for x in 0..preview_width {
                                    let src_x = x * 4;
                                    let src_y = y * 4;
                                    let src_idx = ((src_y * frame_width + src_x) * bytes_per_pixel) as usize;
                                    let dst_idx = ((y * preview_width + x) * preview_bpp) as usize;

                                    if src_idx + (bytes_per_pixel as usize) <= frame_data.len() {
                                        if frame_format == PixelFormat::Bgra {
                                            // BGRA -> RGBA
                                            preview_data[dst_idx] = frame_data[src_idx + 2];     // R
                                            preview_data[dst_idx + 1] = frame_data[src_idx + 1]; // G
                                            preview_data[dst_idx + 2] = frame_data[src_idx];     // B
                                            preview_data[dst_idx + 3] = 255;                     // A
                                        } else {
                                            // RGB -> RGBA
                                            preview_data[dst_idx] = frame_data[src_idx];
                                            preview_data[dst_idx + 1] = frame_data[src_idx + 1];
                                            preview_data[dst_idx + 2] = frame_data[src_idx + 2];
                                            preview_data[dst_idx + 3] = 255;
                                        }
                                    }
                                }
                            }

                            let encoded = base64::Engine::encode(
                                &base64::engine::general_purpose::STANDARD,
                                &preview_data,
                            );
                            let _ = app_for_preview.emit("ussp://camera-preview-rgba", serde_json::json!({
                                "width": preview_width,
                                "height": preview_height,
                                "data": encoded,
                            }));
                        });
                    }
                }

                _ = audio_ticker.tick() => {
                    // Send audio based on source
                    if audio_handle.is_some() {
                        // Process accumulated audio buffer
                        let channels = audio_channels as usize;
                        let samples_needed = samples_per_frame * channels;

                        if audio_buffer.len() >= samples_needed {
                            let samples: Vec<f32> = audio_buffer.drain(..samples_needed).collect();

                            // Calculate level and emit event for waveform display
                            let level_db = calculate_level_db(&samples);
                            let display_samples = downsample_for_display(&samples, 64);
                            let _ = app_clone.emit("ussp://sender-audio-level", serde_json::json!({
                                "track_id": 0,
                                "level_db": level_db,
                                "samples": display_samples,
                            }));

                            let audio_data = convert_f32_to_pcm(&samples);

                            let payload = AudioPayload::new(
                                0, // Single track for microphone
                                audio_sample_rate,
                                audio_channels as u8,
                                16,
                                AudioCodecType::Pcm,
                                audio_pts,
                                samples_per_frame as u32,
                                Bytes::from(audio_data),
                            );

                            if sender.send_audio(payload).await.is_ok() {
                                audio_frames_sent += 1;
                            }

                            audio_pts += 20_000; // 20ms
                        }
                    } else {
                        // Send test tone for each track
                        for track_id in 0..options.audio_tracks as u16 {
                            let audio_data = generate_test_audio(48000, 960, 440.0, track_id);

                            // Generate samples for waveform display (convert from PCM)
                            let display_samples: Vec<f64> = (0..64)
                                .map(|i| {
                                    let t = i as f32 / 64.0 * 960.0 / 48000.0;
                                    let freq = 440.0 * (1.0 + track_id as f32 * 0.5);
                                    (t * freq * 2.0 * std::f32::consts::PI).sin() as f64
                                })
                                .collect();
                            let level_db = -6.0; // Test tone at ~-6dB

                            let _ = app_clone.emit("ussp://sender-audio-level", serde_json::json!({
                                "track_id": track_id,
                                "level_db": level_db,
                                "samples": display_samples,
                            }));

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
                }

                _ = sync_ticker.tick() => {
                    let mut sync = SyncPoint::new(
                        ussp::sync::ClockSync::current_time_us(),
                        video_pts,
                    );

                    let num_tracks = if audio_handle.is_some() {
                        1
                    } else {
                        options.audio_tracks as u16
                    };

                    for track_id in 0..num_tracks {
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

        // Cleanup - explicitly drop handles to ensure resources are released
        // Drop camera first to turn off the LED
        drop(camera_handle);
        drop(audio_handle);
        let _ = sender.close().await;
        tracing::info!("Sender stopped, camera released");

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

    // Wait for cleanup (camera thread join may take some time)
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

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

//! Device capture module for video and audio input using ffmpeg.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Read};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Video device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceInfo {
    pub index: usize,
    pub name: String,
    pub description: String,
}

/// Audio device information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDeviceInfo {
    pub index: usize,
    pub name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

/// Resolution option.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ResolutionOption {
    #[serde(rename = "640x480")]
    Vga,
    #[serde(rename = "1280x720")]
    Hd720,
    #[serde(rename = "1920x1080")]
    Hd1080,
}

impl ResolutionOption {
    pub fn dimensions(&self) -> (u32, u32) {
        match self {
            ResolutionOption::Vga => (640, 480),
            ResolutionOption::Hd720 => (1280, 720),
            ResolutionOption::Hd1080 => (1920, 1080),
        }
    }
}

impl Default for ResolutionOption {
    fn default() -> Self {
        Self::Hd720
    }
}

/// Video source type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum VideoSource {
    TestPattern,
    Camera { device_name: String },
}

impl Default for VideoSource {
    fn default() -> Self {
        Self::TestPattern
    }
}

/// Audio source type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum AudioSource {
    TestTone,
    Microphone { device_index: usize },
}

impl Default for AudioSource {
    fn default() -> Self {
        Self::TestTone
    }
}

/// List available video devices using ffmpeg.
pub fn list_video_devices() -> Result<Vec<VideoDeviceInfo>, String> {
    // Run ffmpeg to list devices
    let output = Command::new("ffmpeg")
        .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
        .stderr(Stdio::piped())
        .stdout(Stdio::null())
        .output()
        .map_err(|e| format!("Failed to run ffmpeg: {}", e))?;

    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut devices = Vec::new();
    let mut in_video_section = false;

    for line in stderr.lines() {
        if line.contains("AVFoundation video devices:") {
            in_video_section = true;
            continue;
        }
        if line.contains("AVFoundation audio devices:") {
            break;
        }
        if in_video_section {
            // Parse lines like "[AVFoundation indev @ 0x...] [0] FaceTime HD Camera"
            if let Some(bracket_pos) = line.rfind("] [") {
                let after_bracket = &line[bracket_pos + 3..];
                if let Some(end_bracket) = after_bracket.find(']') {
                    if let Ok(index) = after_bracket[..end_bracket].parse::<usize>() {
                        let name = after_bracket[end_bracket + 2..].trim().to_string();
                        tracing::info!("list_video_devices: index={}, name={}", index, &name);
                        devices.push(VideoDeviceInfo {
                            index,
                            name: name.clone(),
                            description: name,
                        });
                    }
                }
            }
        }
    }

    Ok(devices)
}

/// List available audio input devices.
pub fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    let host = cpal::default_host();
    let devices = host
        .input_devices()
        .map_err(|e| format!("Failed to query audio devices: {}", e))?;

    let mut result = Vec::new();
    for (idx, device) in devices.enumerate() {
        let name = device.name().unwrap_or_else(|_| "Unknown".to_string());

        // Get default config for sample rate and channels
        let (sample_rate, channels) = if let Ok(config) = device.default_input_config() {
            (config.sample_rate().0, config.channels())
        } else {
            (48000, 2)
        };

        result.push(AudioDeviceInfo {
            index: idx,
            name,
            sample_rate,
            channels,
        });
    }

    Ok(result)
}

/// Camera frame pixel format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PixelFormat {
    Rgb,
    Bgra,
}

/// Camera frame data.
#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
    pub format: PixelFormat,
}

/// Camera capture handle using ffmpeg (runs in a separate thread).
pub struct CameraCaptureHandle {
    running: Arc<AtomicBool>,
    width: u32,
    height: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CameraCaptureHandle {
    /// Start camera capture using ffmpeg in a dedicated thread.
    pub fn start(
        device_name: String,
        resolution: ResolutionOption,
        frame_tx: mpsc::UnboundedSender<CameraFrame>,
        fps: u32,
    ) -> Result<Self, String> {
        let (width, height) = resolution.dimensions();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // Find device index by name
        let devices = list_video_devices()?;
        let device_index = devices
            .iter()
            .find(|d| d.name == device_name)
            .map(|d| d.index)
            .ok_or_else(|| format!("Camera not found: {}", device_name))?;

        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        let thread = std::thread::spawn(move || {
            tracing::info!(
                "Starting ffmpeg capture: device={}, {}x{} @ {}fps",
                device_index,
                width,
                height,
                fps
            );

            // Build ffmpeg command
            // -f avfoundation: use AVFoundation input
            // -framerate: target frame rate
            // -video_size: resolution
            // -i "index:none": video device index, no audio
            // -f rawvideo: output raw video
            // -pix_fmt rgb24: RGB format
            // pipe:1: output to stdout
            let mut child = match Command::new("ffmpeg")
                .args([
                    "-f", "avfoundation",
                    "-framerate", &fps.to_string(),
                    "-video_size", &format!("{}x{}", width, height),
                    "-i", &format!("{}:none", device_index),
                    "-f", "rawvideo",
                    "-pix_fmt", "rgb24",
                    "-an",  // no audio
                    "pipe:1",
                ])
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                Ok(child) => child,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to start ffmpeg: {}", e)));
                    return;
                }
            };

            // Spawn a thread to log stderr
            if let Some(stderr) = child.stderr.take() {
                std::thread::spawn(move || {
                    let reader = BufReader::new(stderr);
                    for line in reader.lines() {
                        if let Ok(line) = line {
                            tracing::debug!("ffmpeg: {}", line);
                        }
                    }
                });
            }

            let stdout = match child.stdout.take() {
                Some(stdout) => stdout,
                None => {
                    let _ = init_tx.send(Err("Failed to get ffmpeg stdout".to_string()));
                    return;
                }
            };

            // Signal successful initialization
            let _ = init_tx.send(Ok(()));

            let frame_size = (width * height * 3) as usize; // RGB24
            let mut reader = BufReader::with_capacity(frame_size * 2, stdout);
            let mut buffer = vec![0u8; frame_size];

            let mut frame_count: u64 = 0;
            let mut last_log_time = std::time::Instant::now();

            while running_clone.load(Ordering::Relaxed) {
                // Read exactly one frame
                match reader.read_exact(&mut buffer) {
                    Ok(()) => {
                        let frame_data = CameraFrame {
                            width,
                            height,
                            data: buffer.clone(),
                            format: PixelFormat::Rgb,
                        };
                        if frame_tx.send(frame_data).is_err() {
                            break;
                        }
                        frame_count += 1;

                        // Log FPS every second
                        let now = std::time::Instant::now();
                        if now.duration_since(last_log_time).as_secs() >= 1 {
                            tracing::info!("Camera capture FPS: {}", frame_count);
                            frame_count = 0;
                            last_log_time = now;
                        }
                    }
                    Err(e) => {
                        if running_clone.load(Ordering::Relaxed) {
                            tracing::warn!("Failed to read frame: {}", e);
                        }
                        break;
                    }
                }
            }

            // Kill ffmpeg process
            tracing::info!("Stopping ffmpeg...");
            let _ = child.kill();
            let _ = child.wait();
            tracing::info!("ffmpeg stopped");
        });

        // Wait for initialization
        match init_rx.recv() {
            Ok(Ok(())) => {
                tracing::info!("Camera capture started: {}x{}", width, height);
                Ok(Self {
                    running,
                    width,
                    height,
                    thread: Some(thread),
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Camera thread terminated unexpectedly".to_string()),
        }
    }

    /// Get the actual resolution.
    pub fn resolution(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Stop the camera capture.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for CameraCaptureHandle {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Relaxed);
        // Wait for thread to finish
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// Audio capture handle (runs in a separate thread).
pub struct AudioCaptureHandle {
    running: Arc<AtomicBool>,
    sample_rate: u32,
    channels: u16,
    _thread: std::thread::JoinHandle<()>,
}

impl AudioCaptureHandle {
    /// Start audio capture in a dedicated thread.
    pub fn start(
        device_index: usize,
        audio_tx: mpsc::UnboundedSender<Vec<f32>>,
    ) -> Result<Self, String> {
        // First, query device info on the main thread to return sample_rate and channels
        let host = cpal::default_host();
        let devices: Vec<_> = host
            .input_devices()
            .map_err(|e| format!("Failed to query devices: {}", e))?
            .collect();

        let device = devices
            .into_iter()
            .nth(device_index)
            .ok_or_else(|| format!("Audio device {} not found", device_index))?;

        let config = device
            .default_input_config()
            .map_err(|e| format!("Failed to get config: {}", e))?;

        let sample_rate = config.sample_rate().0;
        let channels = config.channels();
        let sample_format = config.sample_format();
        let stream_config: cpal::StreamConfig = config.into();

        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // Use a channel to pass initialization result from thread
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(), String>>();

        // Spawn a thread that creates and runs the stream
        let thread = std::thread::spawn(move || {
            // Re-open device in this thread (necessary because device/stream are not Send)
            let host = cpal::default_host();
            let devices: Vec<_> = match host.input_devices() {
                Ok(devs) => devs.collect(),
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to query devices: {}", e)));
                    return;
                }
            };

            let device = match devices.into_iter().nth(device_index) {
                Some(d) => d,
                None => {
                    let _ = init_tx.send(Err(format!("Audio device {} not found", device_index)));
                    return;
                }
            };

            let stream = match sample_format {
                cpal::SampleFormat::F32 => {
                    let tx = audio_tx.clone();
                    let running_cb = Arc::clone(&running_clone);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[f32], _: &cpal::InputCallbackInfo| {
                            if running_cb.load(Ordering::Relaxed) {
                                let _ = tx.send(data.to_vec());
                            }
                        },
                        |err| {
                            tracing::error!("Audio stream error: {}", err);
                        },
                        None,
                    )
                }
                cpal::SampleFormat::I16 => {
                    let tx = audio_tx.clone();
                    let running_cb = Arc::clone(&running_clone);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[i16], _: &cpal::InputCallbackInfo| {
                            if running_cb.load(Ordering::Relaxed) {
                                let samples: Vec<f32> =
                                    data.iter().map(|&s| s as f32 / 32768.0).collect();
                                let _ = tx.send(samples);
                            }
                        },
                        |err| {
                            tracing::error!("Audio stream error: {}", err);
                        },
                        None,
                    )
                }
                cpal::SampleFormat::U16 => {
                    let tx = audio_tx.clone();
                    let running_cb = Arc::clone(&running_clone);
                    device.build_input_stream(
                        &stream_config,
                        move |data: &[u16], _: &cpal::InputCallbackInfo| {
                            if running_cb.load(Ordering::Relaxed) {
                                let samples: Vec<f32> = data
                                    .iter()
                                    .map(|&s| (s as f32 / 32768.0) - 1.0)
                                    .collect();
                                let _ = tx.send(samples);
                            }
                        },
                        |err| {
                            tracing::error!("Audio stream error: {}", err);
                        },
                        None,
                    )
                }
                _ => {
                    let _ = init_tx.send(Err("Unsupported sample format".to_string()));
                    return;
                }
            };

            let stream = match stream {
                Ok(s) => s,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to build stream: {}", e)));
                    return;
                }
            };

            if let Err(e) = stream.play() {
                let _ = init_tx.send(Err(format!("Failed to start stream: {}", e)));
                return;
            }

            // Signal success
            let _ = init_tx.send(Ok(()));

            // Keep the stream alive until running becomes false
            while running_clone.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            // Stream is dropped here when thread exits
            drop(stream);
        });

        // Wait for initialization result
        match init_rx.recv() {
            Ok(Ok(())) => {
                tracing::info!(
                    "Audio capture started: {}Hz, {} channels",
                    sample_rate,
                    channels
                );
                Ok(Self {
                    running,
                    sample_rate,
                    channels,
                    _thread: thread,
                })
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Err("Audio thread terminated unexpectedly".to_string()),
        }
    }

    /// Get sample rate.
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    /// Get channel count.
    pub fn channels(&self) -> u16 {
        self.channels
    }

    /// Stop the audio capture.
    pub fn stop(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for AudioCaptureHandle {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Initialize camera backend (no-op for ffmpeg).
pub fn initialize_camera_backend() -> Result<(), String> {
    // Check if ffmpeg is available
    match Command::new("ffmpeg").arg("-version").output() {
        Ok(output) => {
            if output.status.success() {
                tracing::info!("ffmpeg is available");
                Ok(())
            } else {
                Err("ffmpeg returned error".to_string())
            }
        }
        Err(e) => Err(format!("ffmpeg not found: {}", e)),
    }
}

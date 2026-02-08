//! Device capture module for video and audio input.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use serde::{Deserialize, Serialize};
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

/// List available video devices.
pub fn list_video_devices() -> Result<Vec<VideoDeviceInfo>, String> {
    let devices =
        nokhwa::query(ApiBackend::Auto).map_err(|e| format!("Failed to query cameras: {}", e))?;

    let result: Vec<VideoDeviceInfo> = devices
        .into_iter()
        .enumerate()
        .map(|(list_idx, info)| {
            // Get the actual camera index from CameraInfo
            let camera_index = info.index().as_index().unwrap_or(0) as usize;
            tracing::info!(
                "list_video_devices: list_idx={}, camera_index={}, name={}",
                list_idx, camera_index, info.human_name()
            );
            VideoDeviceInfo {
                index: camera_index,
                name: info.human_name(),
                description: info.description().to_string(),
            }
        })
        .collect();
    Ok(result)
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

/// Camera frame data.
#[derive(Debug, Clone)]
pub struct CameraFrame {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

/// Camera capture handle (runs in a separate thread).
pub struct CameraCaptureHandle {
    running: Arc<AtomicBool>,
    width: u32,
    height: u32,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl CameraCaptureHandle {
    /// Start camera capture in a dedicated thread.
    pub fn start(
        device_name: String,
        resolution: ResolutionOption,
        frame_tx: mpsc::UnboundedSender<CameraFrame>,
        fps: u32,
    ) -> Result<Self, String> {
        let (width, height) = resolution.dimensions();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = Arc::clone(&running);

        // We need to initialize camera in the thread
        let (init_tx, init_rx) = std::sync::mpsc::channel::<Result<(u32, u32), String>>();

        let thread = std::thread::spawn(move || {
            tracing::info!("Opening camera by name: {}", device_name);

            // Find camera by name to get the correct index
            let camera_list = match nokhwa::query(ApiBackend::Auto) {
                Ok(list) => list,
                Err(e) => {
                    let _ = init_tx.send(Err(format!("Failed to query cameras: {}", e)));
                    return;
                }
            };

            tracing::info!("Available cameras:");
            for (i, info) in camera_list.iter().enumerate() {
                tracing::info!("  [{}] index={:?}, name={}", i, info.index(), info.human_name());
            }

            // Find camera by name
            let camera_info = camera_list.iter().find(|info| info.human_name() == device_name);
            let index = match camera_info {
                Some(info) => info.index().clone(),
                None => {
                    let _ = init_tx.send(Err(format!("Camera not found: {}", device_name)));
                    return;
                }
            };

            tracing::info!("Found camera '{}' at index {:?}", device_name, index);

            // Use None to accept the camera's default format
            // This is more compatible with virtual cameras like OBS Virtual Camera
            let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::None);

            let mut camera = match Camera::new(index.clone(), requested) {
                Ok(cam) => cam,
                Err(e) => {
                    tracing::error!("Failed with None format, error: {}", e);
                    // Try with AbsoluteHighestResolution as fallback
                    let requested2 = RequestedFormat::new::<RgbFormat>(
                        RequestedFormatType::AbsoluteHighestResolution,
                    );
                    match Camera::new(index, requested2) {
                        Ok(cam) => cam,
                        Err(e2) => {
                            let _ = init_tx.send(Err(format!("Failed to open camera: {}", e2)));
                            return;
                        }
                    }
                }
            };

            // Open the camera stream - this is required before capturing frames
            if let Err(e) = camera.open_stream() {
                let _ = init_tx.send(Err(format!("Failed to open camera stream: {}", e)));
                return;
            }

            let actual_res = camera.resolution();
            let actual_width = actual_res.width();
            let actual_height = actual_res.height();

            tracing::info!(
                "Camera opened and streaming: {}x{} (requested {}x{})",
                actual_width,
                actual_height,
                width,
                height
            );

            // Send actual resolution
            if init_tx.send(Ok((actual_width, actual_height))).is_err() {
                return;
            }
            let frame_interval = std::time::Duration::from_millis(1000 / fps as u64);

            while running_clone.load(Ordering::Relaxed) {
                match camera.frame() {
                    Ok(frame) => {
                        match frame.decode_image::<RgbFormat>() {
                            Ok(decoded) => {
                                let frame_data = CameraFrame {
                                    width: actual_width,
                                    height: actual_height,
                                    data: decoded.to_vec(),
                                };
                                if frame_tx.send(frame_data).is_err() {
                                    break;
                                }
                            }
                            Err(e) => {
                                tracing::warn!("Failed to decode frame: {}", e);
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to capture frame: {}", e);
                    }
                }

                std::thread::sleep(frame_interval);
            }

            // Explicitly stop the camera stream before dropping
            let _ = camera.stop_stream();
            tracing::info!("Camera stream stopped");
        });

        // Wait for initialization
        match init_rx.recv() {
            Ok(Ok((actual_width, actual_height))) => {
                Ok(Self {
                    running,
                    width: actual_width,
                    height: actual_height,
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
        // Wait for thread to finish to ensure camera is properly released
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

/// Initialize nokhwa (required on macOS).
pub fn initialize_camera_backend() -> Result<(), String> {
    // On macOS, we need to request permission
    #[cfg(target_os = "macos")]
    {
        nokhwa::nokhwa_initialize(|granted| {
            if granted {
                tracing::info!("Camera permission granted");
            } else {
                tracing::warn!("Camera permission denied");
            }
        });
    }
    Ok(())
}

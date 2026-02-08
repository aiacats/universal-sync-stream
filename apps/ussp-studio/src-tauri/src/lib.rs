//! USSP Studio Tauri commands.

mod capture;
mod receiver;
mod sender;
mod state;

use capture::{AudioDeviceInfo, VideoDeviceInfo};
use sender::SenderOptions;
use state::AppState;
use std::sync::Arc;
use tauri::State;
use tracing_subscriber::prelude::*;

/// Tauri command: List available video devices (cameras).
#[tauri::command]
fn list_video_devices() -> Result<Vec<VideoDeviceInfo>, String> {
    capture::list_video_devices()
}

/// Tauri command: List available audio input devices (microphones).
#[tauri::command]
fn list_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    capture::list_audio_devices()
}

/// Tauri command: Initialize camera backend (required on macOS).
#[tauri::command]
fn initialize_capture() -> Result<(), String> {
    capture::initialize_camera_backend()
}

/// Tauri command: Start the sender.
#[tauri::command]
async fn start_sender(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    dest: String,
    port: u16,
    options: Option<SenderOptions>,
) -> Result<(), String> {
    let options = options.unwrap_or_default();
    sender::start_sender(app, Arc::clone(&state), dest, port, options).await
}

/// Tauri command: Stop the sender.
#[tauri::command]
async fn stop_sender(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    sender::stop_sender(Arc::clone(&state)).await
}

/// Tauri command: Get sender statistics.
#[tauri::command]
fn get_sender_stats(state: State<'_, Arc<AppState>>) -> state::SenderStats {
    sender::get_sender_stats(Arc::clone(&state))
}

/// Tauri command: Start the receiver.
#[tauri::command]
async fn start_receiver(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    port: u16,
) -> Result<(), String> {
    receiver::start_receiver(app, Arc::clone(&state), port).await
}

/// Tauri command: Stop the receiver.
#[tauri::command]
async fn stop_receiver(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    receiver::stop_receiver(Arc::clone(&state)).await
}

/// Tauri command: Get receiver statistics.
#[tauri::command]
fn get_receiver_stats(state: State<'_, Arc<AppState>>) -> state::ReceiverStats {
    receiver::get_receiver_stats(Arc::clone(&state))
}

/// Tauri command: Check if sender is running.
#[tauri::command]
fn is_sender_running(state: State<'_, Arc<AppState>>) -> bool {
    state.sender.read().is_running
}

/// Tauri command: Check if receiver is running.
#[tauri::command]
fn is_receiver_running(state: State<'_, Arc<AppState>>) -> bool {
    state.receiver.read().is_running
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Set up file logging - writes to ussp-studio.log (overwrites on each run)
    let log_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let log_file = std::fs::File::create(log_dir.join("ussp-studio.log"))
        .expect("Failed to create log file");

    // Create file and stdout layers
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(log_file)
        .with_ansi(false);

    let stdout_layer = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stdout);

    tracing_subscriber::registry()
        .with(file_layer)
        .with(stdout_layer)
        .with(tracing_subscriber::EnvFilter::from_default_env()
            .add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("USSP Studio starting, log file: {:?}", log_dir.join("ussp-studio.log"));

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            // Device listing
            list_video_devices,
            list_audio_devices,
            initialize_capture,
            // Sender
            start_sender,
            stop_sender,
            get_sender_stats,
            is_sender_running,
            // Receiver
            start_receiver,
            stop_receiver,
            get_receiver_stats,
            is_receiver_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

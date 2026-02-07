//! USSP Tester Tauri commands.

mod receiver;
mod sender;
mod state;

use sender::SenderOptions;
use state::AppState;
use std::sync::Arc;
use tauri::State;

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
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            start_sender,
            stop_sender,
            get_sender_stats,
            start_receiver,
            stop_receiver,
            get_receiver_stats,
            is_sender_running,
            is_receiver_running,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

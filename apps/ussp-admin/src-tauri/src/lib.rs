//! USSP Admin - SFU Server Management GUI

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tauri::{Manager, State};
use tokio::sync::RwLock;
use tracing::{error, info};

mod api;
mod sidecar;

/// Application state
pub struct AppState {
    pub client: api::ApiClient,
    pub config: RwLock<ServerConfig>,
    pub sidecar: Arc<sidecar::SidecarState>,
}

/// Server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub server_url: String,
    pub api_key: Option<String>,
    pub token: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:8080".to_string(),
            api_key: None,
            token: None,
        }
    }
}

// ============================================================================
// Existing Tauri Commands
// ============================================================================

#[tauri::command]
async fn get_config(state: State<'_, Arc<AppState>>) -> Result<ServerConfig, String> {
    Ok(state.config.read().await.clone())
}

#[tauri::command]
async fn set_config(state: State<'_, Arc<AppState>>, config: ServerConfig) -> Result<(), String> {
    *state.config.write().await = config;
    Ok(())
}

#[tauri::command]
async fn connect(
    state: State<'_, Arc<AppState>>,
    server_url: String,
    api_key: String,
) -> Result<api::TokenResponse, String> {
    // Update config
    {
        let mut config = state.config.write().await;
        config.server_url = server_url.clone();
        config.api_key = Some(api_key.clone());
    }

    // Get token
    let token_response = state
        .client
        .get_token(&server_url, &api_key, "admin")
        .await?;

    // Store token
    {
        let mut config = state.config.write().await;
        config.token = Some(token_response.token.clone());
    }

    Ok(token_response)
}

#[tauri::command]
async fn disconnect(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut config = state.config.write().await;
    config.token = None;
    Ok(())
}

#[tauri::command]
async fn health_check(state: State<'_, Arc<AppState>>) -> Result<api::HealthResponse, String> {
    let config = state.config.read().await;
    state.client.health_check(&config.server_url).await
}

#[tauri::command]
async fn get_rooms(state: State<'_, Arc<AppState>>) -> Result<Vec<api::RoomResponse>, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_rooms(&config.server_url, token)
        .await
}

#[tauri::command]
async fn create_room(
    state: State<'_, Arc<AppState>>,
    room_id: Option<String>,
) -> Result<api::RoomResponse, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .create_room(&config.server_url, token, room_id)
        .await
}

#[tauri::command]
async fn get_room(
    state: State<'_, Arc<AppState>>,
    room_id: String,
) -> Result<api::RoomResponse, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_room(&config.server_url, token, &room_id)
        .await
}

#[tauri::command]
async fn delete_room(state: State<'_, Arc<AppState>>, room_id: String) -> Result<(), String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .delete_room(&config.server_url, token, &room_id)
        .await
}

#[tauri::command]
async fn get_room_stats(
    state: State<'_, Arc<AppState>>,
    room_id: String,
) -> Result<api::RoomStats, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_room_stats(&config.server_url, token, &room_id)
        .await
}

#[tauri::command]
async fn get_participants(
    state: State<'_, Arc<AppState>>,
    room_id: String,
) -> Result<Vec<api::ParticipantResponse>, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_participants(&config.server_url, token, &room_id)
        .await
}

#[tauri::command]
async fn remove_participant(
    state: State<'_, Arc<AppState>>,
    room_id: String,
    participant_id: String,
) -> Result<(), String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .remove_participant(&config.server_url, token, &room_id, &participant_id)
        .await
}

#[tauri::command]
async fn get_servers(state: State<'_, Arc<AppState>>) -> Result<Vec<api::ServerResponse>, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_servers(&config.server_url, token)
        .await
}

#[tauri::command]
async fn get_global_stats(
    state: State<'_, Arc<AppState>>,
) -> Result<api::GlobalStatsResponse, String> {
    let config = state.config.read().await;
    let token = config.token.as_ref().ok_or("Not authenticated")?;
    state
        .client
        .get_global_stats(&config.server_url, token)
        .await
}

// ============================================================================
// Sidecar Tauri Commands
// ============================================================================

#[tauri::command]
async fn start_server(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.sidecar.start(&app).await
}

#[tauri::command]
async fn stop_server(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    state.sidecar.stop().await
}

#[tauri::command]
async fn restart_server(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<(), String> {
    state.sidecar.stop().await?;
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
    state.sidecar.start(&app).await
}

#[tauri::command]
async fn get_server_status(state: State<'_, Arc<AppState>>) -> Result<bool, String> {
    Ok(state.sidecar.is_running().await)
}

#[tauri::command]
async fn get_server_logs(state: State<'_, Arc<AppState>>) -> Result<Vec<String>, String> {
    Ok(state.sidecar.get_logs().await)
}

#[tauri::command]
async fn get_sidecar_config(
    state: State<'_, Arc<AppState>>,
) -> Result<sidecar::SidecarConfig, String> {
    Ok(state.sidecar.get_config().await)
}

#[tauri::command]
async fn set_sidecar_config(
    state: State<'_, Arc<AppState>>,
    config: sidecar::SidecarConfig,
) -> Result<(), String> {
    if state.sidecar.is_running().await {
        return Err("Cannot change config while server is running. Stop the server first.".into());
    }
    state.sidecar.set_config(config).await;
    Ok(())
}

// ============================================================================
// Application Entry Point
// ============================================================================

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let sidecar_state = Arc::new(sidecar::SidecarState::new());
    let app_state = Arc::new(AppState {
        client: api::ApiClient::new(),
        config: RwLock::new(ServerConfig::default()),
        sidecar: Arc::clone(&sidecar_state),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(app_state)
        .setup(move |app| {
            // Auto-start the SFU server sidecar on launch
            let handle = app.handle().clone();
            let sidecar = Arc::clone(&sidecar_state);
            tauri::async_runtime::spawn(async move {
                if let Err(e) = sidecar.start(&handle).await {
                    error!("Failed to auto-start SFU server: {}", e);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Existing commands
            get_config,
            set_config,
            connect,
            disconnect,
            health_check,
            get_rooms,
            create_room,
            get_room,
            delete_room,
            get_room_stats,
            get_participants,
            remove_participant,
            get_servers,
            get_global_stats,
            // Sidecar commands
            start_server,
            stop_server,
            restart_server,
            get_server_status,
            get_server_logs,
            get_sidecar_config,
            set_sidecar_config,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                // Gracefully stop sidecar on application exit
                let state = app.state::<Arc<AppState>>();
                let sidecar: Arc<sidecar::SidecarState> = Arc::clone(&state.sidecar);
                tauri::async_runtime::block_on(async {
                    if let Err(e) = sidecar.stop().await {
                        error!("Failed to stop sidecar on exit: {}", e);
                    } else {
                        info!("SFU server sidecar stopped on exit");
                    }
                });
            }
        });
}

//! Application state management.

use crate::auth::AuthConfig;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use ussp_sfu::{SfuConfig, SfuServer};

/// Server info for cluster management.
#[derive(Debug, Clone)]
pub struct ServerInfo {
    /// Server ID.
    pub id: String,
    /// Server address (for media).
    pub media_addr: String,
    /// Server address (for control).
    pub control_addr: String,
    /// Current load (0.0 - 1.0).
    pub load: f64,
    /// Is server healthy.
    pub healthy: bool,
    /// Active room count.
    pub room_count: usize,
    /// Active participant count.
    pub participant_count: usize,
}

/// Application state shared across handlers.
#[derive(Clone)]
pub struct AppState {
    /// Authentication configuration.
    pub auth_config: AuthConfig,
    /// SFU servers (by server ID).
    servers: Arc<RwLock<HashMap<String, Arc<SfuServer>>>>,
    /// Server info for load balancing.
    server_info: Arc<RwLock<HashMap<String, ServerInfo>>>,
    /// Room to server mapping.
    room_servers: Arc<RwLock<HashMap<String, String>>>,
}

impl AppState {
    /// Create a new application state.
    pub fn new(auth_config: AuthConfig) -> Self {
        Self {
            auth_config,
            servers: Arc::new(RwLock::new(HashMap::new())),
            server_info: Arc::new(RwLock::new(HashMap::new())),
            room_servers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a local SFU server.
    pub async fn register_local_server(&self, server: Arc<SfuServer>, config: &SfuConfig) {
        let server_id = config.server_id.clone();

        self.servers
            .write()
            .await
            .insert(server_id.clone(), server);

        self.server_info.write().await.insert(
            server_id.clone(),
            ServerInfo {
                id: server_id,
                media_addr: config.media_bind_addr.clone(),
                control_addr: config.control_bind_addr.clone(),
                load: 0.0,
                healthy: true,
                room_count: 0,
                participant_count: 0,
            },
        );
    }

    /// Get a server by ID.
    pub async fn get_server(&self, server_id: &str) -> Option<Arc<SfuServer>> {
        self.servers.read().await.get(server_id).cloned()
    }

    /// Get the server for a room.
    pub async fn get_server_for_room(&self, room_id: &str) -> Option<Arc<SfuServer>> {
        let server_id = self.room_servers.read().await.get(room_id).cloned()?;
        self.get_server(&server_id).await
    }

    /// Assign a room to a server.
    pub async fn assign_room_to_server(&self, room_id: &str, server_id: &str) {
        self.room_servers
            .write()
            .await
            .insert(room_id.to_string(), server_id.to_string());
    }

    /// Remove room assignment.
    pub async fn remove_room_assignment(&self, room_id: &str) {
        self.room_servers.write().await.remove(room_id);
    }

    /// Get the best server for a new room (load balancing).
    pub async fn get_best_server(&self) -> Option<String> {
        let info = self.server_info.read().await;

        info.values()
            .filter(|s| s.healthy)
            .min_by(|a, b| a.load.partial_cmp(&b.load).unwrap())
            .map(|s| s.id.clone())
    }

    /// Update server info.
    pub async fn update_server_info(&self, server_id: &str) {
        if let Some(server) = self.get_server(server_id).await {
            let stats = server.stats();
            let _summaries = server.room_summaries().await;

            if let Some(info) = self.server_info.write().await.get_mut(server_id) {
                info.room_count = stats.rooms_active as usize;
                info.participant_count = stats.participants_active as usize;
                // Simple load calculation (can be improved)
                info.load = (info.participant_count as f64) / 1000.0;
            }
        }
    }

    /// Get all server info.
    pub async fn all_server_info(&self) -> Vec<ServerInfo> {
        self.server_info.read().await.values().cloned().collect()
    }

    /// List all rooms across all servers.
    pub async fn list_all_rooms(&self) -> Vec<ussp_sfu::RoomSummary> {
        let mut rooms = Vec::new();
        for server in self.servers.read().await.values() {
            rooms.extend(server.room_summaries().await);
        }
        rooms
    }
}

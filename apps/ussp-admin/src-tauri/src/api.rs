//! HTTP API client for USSP Control Plane

use reqwest::Client;
use serde::{Deserialize, Serialize};

/// API client for the USSP control plane
pub struct ApiClient {
    client: Client,
}

impl ApiClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    fn auth_header(token: &str) -> String {
        format!("Bearer {}", token)
    }

    pub async fn health_check(&self, base_url: &str) -> Result<HealthResponse, String> {
        let url = format!("{}/health", base_url);
        self.client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_token(
        &self,
        base_url: &str,
        api_key: &str,
        role: &str,
    ) -> Result<TokenResponse, String> {
        let url = format!("{}/auth/token", base_url);
        let body = TokenRequest {
            api_key: api_key.to_string(),
            role: Some(role.to_string()),
            rooms: None,
        };

        self.client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_rooms(
        &self,
        base_url: &str,
        token: &str,
    ) -> Result<Vec<RoomResponse>, String> {
        let url = format!("{}/api/v1/rooms", base_url);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn create_room(
        &self,
        base_url: &str,
        token: &str,
        room_id: Option<String>,
    ) -> Result<RoomResponse, String> {
        let url = format!("{}/api/v1/rooms", base_url);
        let body = CreateRoomRequest { room_id };

        let response = self
            .client
            .post(&url)
            .header("Authorization", Self::auth_header(token))
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            let error: ApiError = response
                .json()
                .await
                .unwrap_or(ApiError {
                    code: "UNKNOWN".to_string(),
                    message: "Unknown error".to_string(),
                });
            return Err(error.message);
        }

        response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_room(
        &self,
        base_url: &str,
        token: &str,
        room_id: &str,
    ) -> Result<RoomResponse, String> {
        let url = format!("{}/api/v1/rooms/{}", base_url, room_id);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn delete_room(
        &self,
        base_url: &str,
        token: &str,
        room_id: &str,
    ) -> Result<(), String> {
        let url = format!("{}/api/v1/rooms/{}", base_url, room_id);
        let response = self
            .client
            .delete(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err("Failed to delete room".to_string());
        }
        Ok(())
    }

    pub async fn get_room_stats(
        &self,
        base_url: &str,
        token: &str,
        room_id: &str,
    ) -> Result<RoomStats, String> {
        let url = format!("{}/api/v1/rooms/{}/stats", base_url, room_id);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_participants(
        &self,
        base_url: &str,
        token: &str,
        room_id: &str,
    ) -> Result<Vec<ParticipantResponse>, String> {
        let url = format!("{}/api/v1/rooms/{}/participants", base_url, room_id);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn remove_participant(
        &self,
        base_url: &str,
        token: &str,
        room_id: &str,
        participant_id: &str,
    ) -> Result<(), String> {
        let url = format!(
            "{}/api/v1/rooms/{}/participants/{}",
            base_url, room_id, participant_id
        );
        let response = self
            .client
            .delete(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !response.status().is_success() {
            return Err("Failed to remove participant".to_string());
        }
        Ok(())
    }

    pub async fn get_servers(
        &self,
        base_url: &str,
        token: &str,
    ) -> Result<Vec<ServerResponse>, String> {
        let url = format!("{}/api/v1/servers", base_url);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    pub async fn get_global_stats(
        &self,
        base_url: &str,
        token: &str,
    ) -> Result<GlobalStatsResponse, String> {
        let url = format!("{}/api/v1/stats", base_url);
        self.client
            .get(&url)
            .header("Authorization", Self::auth_header(token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}

// ============================================================================
// API Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct TokenRequest {
    pub api_key: String,
    pub role: Option<String>,
    pub rooms: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenResponse {
    pub token: String,
    pub expires_in: i64,
}

#[derive(Debug, Serialize)]
pub struct CreateRoomRequest {
    pub room_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomResponse {
    pub id: String,
    pub state: String,
    pub participant_count: usize,
    pub server_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomStats {
    pub id: String,
    pub state: String,
    pub participant_count: usize,
    pub receiver_count: usize,
    pub has_primary_sender: bool,
    pub has_backup_sender: bool,
    pub packets_forwarded: u64,
    pub bytes_forwarded: u64,
    pub uptime_secs: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParticipantResponse {
    pub id: String,
    pub addr: String,
    pub role: String,
    pub state: String,
    pub session_id: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerResponse {
    pub id: String,
    pub media_addr: String,
    pub control_addr: String,
    pub load: f64,
    pub healthy: bool,
    pub room_count: usize,
    pub participant_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalStatsResponse {
    pub total_servers: usize,
    pub total_rooms: usize,
    pub total_participants: usize,
    pub total_packets_forwarded: u64,
    pub total_bytes_forwarded: u64,
}

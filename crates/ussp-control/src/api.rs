//! REST API for SFU control plane.

use crate::auth::{AuthConfig, AuthenticatedClaims, Claims, UserRole};
use crate::error::{Error, Result};
use crate::state::AppState;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Extension, Json, Router,
};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use ussp_sfu::ParticipantRole;

/// Create the API router.
pub fn create_router(state: AppState) -> Router {
    let auth_config = state.auth_config.clone();

    // Public routes (no auth required)
    let public_routes = Router::new()
        .route("/health", get(health_check))
        .route("/auth/token", post(generate_token));

    // Protected routes
    let protected_routes = Router::new()
        // Room management
        .route("/rooms", get(list_rooms))
        .route("/rooms", post(create_room))
        .route("/rooms/:room_id", get(get_room))
        .route("/rooms/:room_id", delete(close_room))
        .route("/rooms/:room_id/stats", get(get_room_stats))
        .route("/rooms/:room_id/participants", get(list_participants))
        .route("/rooms/:room_id/participants", post(add_participant))
        .route(
            "/rooms/:room_id/participants/:participant_id",
            delete(remove_participant),
        )
        // Server management
        .route("/servers", get(list_servers))
        .route("/servers/:server_id/stats", get(get_server_stats))
        // Stats
        .route("/stats", get(get_global_stats));

    Router::new()
        .merge(public_routes)
        .nest("/api/v1", protected_routes)
        .layer(Extension(auth_config))
        .layer(CorsLayer::new().allow_origin(Any).allow_methods(Any))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

// ============================================================================
// Request/Response types
// ============================================================================

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

/// Token request.
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    /// API key or credentials.
    pub api_key: String,
    /// Requested role.
    pub role: Option<UserRole>,
    /// Requested room access.
    pub rooms: Option<Vec<String>>,
}

/// Token response.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub token: String,
    pub expires_in: i64,
}

/// Create room request.
#[derive(Debug, Deserialize)]
pub struct CreateRoomRequest {
    /// Room ID (optional, will be generated if not provided).
    pub room_id: Option<String>,
}

/// Room response.
#[derive(Debug, Serialize)]
pub struct RoomResponse {
    pub id: String,
    pub state: String,
    pub participant_count: usize,
    pub server_id: String,
}

/// Add participant request.
#[derive(Debug, Deserialize)]
pub struct AddParticipantRequest {
    /// Participant ID.
    pub participant_id: String,
    /// Participant address.
    pub addr: String,
    /// Participant role.
    pub role: ParticipantRoleDto,
    /// Session ID.
    pub session_id: u32,
}

/// Participant role DTO.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ParticipantRoleDto {
    PrimarySender,
    BackupSender,
    Receiver,
}

impl From<ParticipantRoleDto> for ParticipantRole {
    fn from(dto: ParticipantRoleDto) -> Self {
        match dto {
            ParticipantRoleDto::PrimarySender => ParticipantRole::PrimarySender,
            ParticipantRoleDto::BackupSender => ParticipantRole::BackupSender,
            ParticipantRoleDto::Receiver => ParticipantRole::Receiver,
        }
    }
}

/// Participant response.
#[derive(Debug, Serialize)]
pub struct ParticipantResponse {
    pub id: String,
    pub addr: String,
    pub role: ParticipantRoleDto,
    pub state: String,
    pub session_id: u32,
}

/// Server response.
#[derive(Debug, Serialize)]
pub struct ServerResponse {
    pub id: String,
    pub media_addr: String,
    pub control_addr: String,
    pub load: f64,
    pub healthy: bool,
    pub room_count: usize,
    pub participant_count: usize,
}

/// Global stats response.
#[derive(Debug, Serialize)]
pub struct GlobalStatsResponse {
    pub total_servers: usize,
    pub total_rooms: usize,
    pub total_participants: usize,
    pub total_packets_forwarded: u64,
    pub total_bytes_forwarded: u64,
}

// ============================================================================
// Handlers
// ============================================================================

/// Health check endpoint.
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Generate authentication token.
async fn generate_token(
    Extension(auth_config): Extension<AuthConfig>,
    Json(req): Json<TokenRequest>,
) -> Result<Json<TokenResponse>> {
    // In production, validate the API key against a database
    // For now, we accept any key and create a token based on role
    let role = req.role.unwrap_or(UserRole::Receiver);
    let rooms = req.rooms.unwrap_or_default();

    let claims = match role {
        UserRole::Admin => Claims::admin(&req.api_key, &auth_config),
        UserRole::Operator => Claims::operator(&req.api_key, &auth_config),
        UserRole::Sender => Claims::sender(&req.api_key, rooms, &auth_config),
        UserRole::Receiver => Claims::receiver(&req.api_key, rooms, &auth_config),
    };

    let token = crate::auth::generate_token(&claims, &auth_config)?;

    Ok(Json(TokenResponse {
        token,
        expires_in: auth_config.token_expiration_secs,
    }))
}

/// List all rooms.
async fn list_rooms(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Result<Json<Vec<RoomResponse>>> {
    let rooms = state.list_all_rooms().await;

    let responses: Vec<RoomResponse> = rooms
        .into_iter()
        .filter(|r| claims.can_access_room(&r.id))
        .map(|r| RoomResponse {
            id: r.id.clone(),
            state: format!("{:?}", r.state),
            participant_count: r.participant_count,
            server_id: String::new(), // TODO: include server ID
        })
        .collect();

    Ok(Json(responses))
}

/// Create a new room.
async fn create_room(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Json(req): Json<CreateRoomRequest>,
) -> Result<(StatusCode, Json<RoomResponse>)> {
    if !claims.can_manage_rooms() {
        return Err(Error::NotAuthorized("Cannot create rooms".into()));
    }

    let room_id = req
        .room_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

    // Get best server for load balancing
    let server_id = state
        .get_best_server()
        .await
        .ok_or_else(|| Error::Internal("No servers available".into()))?;

    let server = state
        .get_server(&server_id)
        .await
        .ok_or_else(|| Error::ServerNotFound(server_id.clone()))?;

    let _room = server.create_room(room_id.clone()).map_err(Error::Sfu)?;

    state.assign_room_to_server(&room_id, &server_id).await;

    Ok((
        StatusCode::CREATED,
        Json(RoomResponse {
            id: room_id,
            state: "Paused".to_string(),
            participant_count: 0,
            server_id,
        }),
    ))
}

/// Get room details.
async fn get_room(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(room_id): Path<String>,
) -> Result<Json<RoomResponse>> {
    if !claims.can_access_room(&room_id) {
        return Err(Error::NotAuthorized("Cannot access room".into()));
    }

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    let room = server
        .get_room(&room_id)
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    Ok(Json(RoomResponse {
        id: room.id.clone(),
        state: format!("{:?}", room.state().await),
        participant_count: room.participant_count(),
        server_id: server.server_id().to_string(),
    }))
}

/// Close a room.
async fn close_room(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(room_id): Path<String>,
) -> Result<StatusCode> {
    if !claims.can_manage_rooms() {
        return Err(Error::NotAuthorized("Cannot close rooms".into()));
    }

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    server.close_room(&room_id).await.map_err(Error::Sfu)?;
    state.remove_room_assignment(&room_id).await;

    Ok(StatusCode::NO_CONTENT)
}

/// Get room statistics.
async fn get_room_stats(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(room_id): Path<String>,
) -> Result<impl IntoResponse> {
    if !claims.can_access_room(&room_id) {
        return Err(Error::NotAuthorized("Cannot access room".into()));
    }

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    let stats = server
        .room_stats(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    Ok(Json(stats))
}

/// List participants in a room.
async fn list_participants(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(room_id): Path<String>,
) -> Result<Json<Vec<ParticipantResponse>>> {
    if !claims.can_access_room(&room_id) {
        return Err(Error::NotAuthorized("Cannot access room".into()));
    }

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    let room = server
        .get_room(&room_id)
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    let summaries = room.participant_summaries().await;

    let responses: Vec<ParticipantResponse> = summaries
        .into_iter()
        .map(|p| ParticipantResponse {
            id: p.id,
            addr: p.addr.to_string(),
            role: match p.role {
                ParticipantRole::PrimarySender => ParticipantRoleDto::PrimarySender,
                ParticipantRole::BackupSender => ParticipantRoleDto::BackupSender,
                ParticipantRole::Receiver => ParticipantRoleDto::Receiver,
            },
            state: format!("{:?}", p.state),
            session_id: p.session_id,
        })
        .collect();

    Ok(Json(responses))
}

/// Add a participant to a room.
async fn add_participant(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(room_id): Path<String>,
    Json(req): Json<AddParticipantRequest>,
) -> Result<(StatusCode, Json<ParticipantResponse>)> {
    // Check authorization based on role
    let role: ParticipantRole = req.role.into();
    match role {
        ParticipantRole::PrimarySender | ParticipantRole::BackupSender => {
            if !claims.can_send(&room_id) {
                return Err(Error::NotAuthorized("Cannot send to room".into()));
            }
        }
        ParticipantRole::Receiver => {
            if !claims.can_access_room(&room_id) {
                return Err(Error::NotAuthorized("Cannot access room".into()));
            }
        }
    }

    let addr: SocketAddr = req
        .addr
        .parse()
        .map_err(|_| Error::BadRequest("Invalid address".into()))?;

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    server
        .add_participant(&room_id, req.participant_id.clone(), addr, role, req.session_id)
        .await
        .map_err(Error::Sfu)?;

    Ok((
        StatusCode::CREATED,
        Json(ParticipantResponse {
            id: req.participant_id,
            addr: req.addr,
            role: req.role,
            state: "Active".to_string(),
            session_id: req.session_id,
        }),
    ))
}

/// Remove a participant from a room.
async fn remove_participant(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path((room_id, participant_id)): Path<(String, String)>,
) -> Result<StatusCode> {
    if !claims.can_manage_rooms() {
        return Err(Error::NotAuthorized("Cannot remove participants".into()));
    }

    let server = state
        .get_server_for_room(&room_id)
        .await
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    let room = server
        .get_room(&room_id)
        .ok_or_else(|| Error::RoomNotFound(room_id.clone()))?;

    room.remove_participant(&participant_id)
        .await
        .map_err(Error::Sfu)?;

    Ok(StatusCode::NO_CONTENT)
}

/// List all servers.
async fn list_servers(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Result<Json<Vec<ServerResponse>>> {
    if !claims.is_admin() {
        return Err(Error::NotAuthorized("Admin only".into()));
    }

    let servers = state.all_server_info().await;

    let responses: Vec<ServerResponse> = servers
        .into_iter()
        .map(|s| ServerResponse {
            id: s.id,
            media_addr: s.media_addr,
            control_addr: s.control_addr,
            load: s.load,
            healthy: s.healthy,
            room_count: s.room_count,
            participant_count: s.participant_count,
        })
        .collect();

    Ok(Json(responses))
}

/// Get server statistics.
async fn get_server_stats(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
    Path(server_id): Path<String>,
) -> Result<impl IntoResponse> {
    if !claims.is_admin() {
        return Err(Error::NotAuthorized("Admin only".into()));
    }

    let server = state
        .get_server(&server_id)
        .await
        .ok_or_else(|| Error::ServerNotFound(server_id))?;

    Ok(Json(server.stats()))
}

/// Get global statistics.
async fn get_global_stats(
    State(state): State<AppState>,
    AuthenticatedClaims(claims): AuthenticatedClaims,
) -> Result<Json<GlobalStatsResponse>> {
    if !claims.is_admin() {
        return Err(Error::NotAuthorized("Admin only".into()));
    }

    let servers = state.all_server_info().await;
    let rooms = state.list_all_rooms().await;

    let total_participants: usize = servers.iter().map(|s| s.participant_count).sum();

    // Sum up forwarded stats from all servers
    let mut total_packets = 0u64;
    let mut total_bytes = 0u64;

    for info in &servers {
        if let Some(server) = state.get_server(&info.id).await {
            let stats = server.stats();
            total_packets += stats.packets_forwarded;
            total_bytes += stats.bytes_forwarded;
        }
    }

    Ok(Json(GlobalStatsResponse {
        total_servers: servers.len(),
        total_rooms: rooms.len(),
        total_participants,
        total_packets_forwarded: total_packets,
        total_bytes_forwarded: total_bytes,
    }))
}

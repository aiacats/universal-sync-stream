//! Error types for the control plane.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Result type alias.
pub type Result<T> = std::result::Result<T, Error>;

/// Control plane errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Room not found.
    #[error("Room not found: {0}")]
    RoomNotFound(String),

    /// Room already exists.
    #[error("Room already exists: {0}")]
    RoomAlreadyExists(String),

    /// Participant not found.
    #[error("Participant not found: {0}")]
    ParticipantNotFound(String),

    /// Server not found.
    #[error("Server not found: {0}")]
    ServerNotFound(String),

    /// Authentication error.
    #[error("Authentication failed: {0}")]
    AuthenticationFailed(String),

    /// Authorization error.
    #[error("Not authorized: {0}")]
    NotAuthorized(String),

    /// Invalid token.
    #[error("Invalid token: {0}")]
    InvalidToken(String),

    /// Token expired.
    #[error("Token expired")]
    TokenExpired,

    /// Bad request.
    #[error("Bad request: {0}")]
    BadRequest(String),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// SFU error.
    #[error("SFU error: {0}")]
    Sfu(#[from] ussp_sfu::Error),

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),

    /// JSON error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// JWT error.
    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

/// Error response for API.
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Error code.
    pub code: String,
    /// Error message.
    pub message: String,
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, code) = match &self {
            Error::RoomNotFound(_) => (StatusCode::NOT_FOUND, "ROOM_NOT_FOUND"),
            Error::RoomAlreadyExists(_) => (StatusCode::CONFLICT, "ROOM_ALREADY_EXISTS"),
            Error::ParticipantNotFound(_) => (StatusCode::NOT_FOUND, "PARTICIPANT_NOT_FOUND"),
            Error::ServerNotFound(_) => (StatusCode::NOT_FOUND, "SERVER_NOT_FOUND"),
            Error::AuthenticationFailed(_) => (StatusCode::UNAUTHORIZED, "AUTHENTICATION_FAILED"),
            Error::NotAuthorized(_) => (StatusCode::FORBIDDEN, "NOT_AUTHORIZED"),
            Error::InvalidToken(_) => (StatusCode::UNAUTHORIZED, "INVALID_TOKEN"),
            Error::TokenExpired => (StatusCode::UNAUTHORIZED, "TOKEN_EXPIRED"),
            Error::BadRequest(_) => (StatusCode::BAD_REQUEST, "BAD_REQUEST"),
            Error::Config(_) => (StatusCode::INTERNAL_SERVER_ERROR, "CONFIG_ERROR"),
            Error::Sfu(_) => (StatusCode::INTERNAL_SERVER_ERROR, "SFU_ERROR"),
            Error::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR"),
            Error::Json(_) => (StatusCode::BAD_REQUEST, "JSON_ERROR"),
            Error::Jwt(_) => (StatusCode::UNAUTHORIZED, "JWT_ERROR"),
        };

        let body = ErrorResponse {
            code: code.to_string(),
            message: self.to_string(),
        };

        (status, axum::Json(body)).into_response()
    }
}

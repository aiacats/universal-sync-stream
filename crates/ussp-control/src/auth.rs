//! Authentication and authorization.

use crate::error::{Error, Result};
use axum::{
    extract::FromRequestParts,
    http::{header, request::Parts},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

/// Authentication configuration.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// JWT secret key.
    pub secret: String,
    /// Token expiration in seconds.
    pub token_expiration_secs: i64,
    /// Issuer name.
    pub issuer: String,
    /// Enable authentication (can be disabled for development).
    pub enabled: bool,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            secret: "change-me-in-production".to_string(),
            token_expiration_secs: 3600,
            issuer: "ussp-control".to_string(),
            enabled: true,
        }
    }
}

impl AuthConfig {
    /// Create a new auth config with a secret.
    pub fn new(secret: impl Into<String>) -> Self {
        Self {
            secret: secret.into(),
            ..Default::default()
        }
    }

    /// Disable authentication.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            ..Default::default()
        }
    }
}

/// JWT claims.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID or API key ID).
    pub sub: String,
    /// Issuer.
    pub iss: String,
    /// Expiration time (Unix timestamp).
    pub exp: i64,
    /// Issued at (Unix timestamp).
    pub iat: i64,
    /// User role.
    pub role: UserRole,
    /// Allowed room IDs (empty = all rooms).
    #[serde(default)]
    pub rooms: Vec<String>,
}

/// User roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UserRole {
    /// Admin - full access.
    Admin,
    /// Operator - can manage rooms.
    Operator,
    /// Sender - can send to rooms.
    Sender,
    /// Receiver - can receive from rooms.
    Receiver,
}

impl Claims {
    /// Create admin claims.
    pub fn admin(sub: impl Into<String>, config: &AuthConfig) -> Self {
        let now = Utc::now();
        Self {
            sub: sub.into(),
            iss: config.issuer.clone(),
            exp: (now + Duration::seconds(config.token_expiration_secs)).timestamp(),
            iat: now.timestamp(),
            role: UserRole::Admin,
            rooms: Vec::new(),
        }
    }

    /// Create operator claims.
    pub fn operator(sub: impl Into<String>, config: &AuthConfig) -> Self {
        let now = Utc::now();
        Self {
            sub: sub.into(),
            iss: config.issuer.clone(),
            exp: (now + Duration::seconds(config.token_expiration_secs)).timestamp(),
            iat: now.timestamp(),
            role: UserRole::Operator,
            rooms: Vec::new(),
        }
    }

    /// Create sender claims for specific rooms.
    pub fn sender(sub: impl Into<String>, rooms: Vec<String>, config: &AuthConfig) -> Self {
        let now = Utc::now();
        Self {
            sub: sub.into(),
            iss: config.issuer.clone(),
            exp: (now + Duration::seconds(config.token_expiration_secs)).timestamp(),
            iat: now.timestamp(),
            role: UserRole::Sender,
            rooms,
        }
    }

    /// Create receiver claims for specific rooms.
    pub fn receiver(sub: impl Into<String>, rooms: Vec<String>, config: &AuthConfig) -> Self {
        let now = Utc::now();
        Self {
            sub: sub.into(),
            iss: config.issuer.clone(),
            exp: (now + Duration::seconds(config.token_expiration_secs)).timestamp(),
            iat: now.timestamp(),
            role: UserRole::Receiver,
            rooms,
        }
    }

    /// Check if claims allow access to a room.
    pub fn can_access_room(&self, room_id: &str) -> bool {
        match self.role {
            UserRole::Admin | UserRole::Operator => true,
            UserRole::Sender | UserRole::Receiver => {
                self.rooms.is_empty() || self.rooms.contains(&room_id.to_string())
            }
        }
    }

    /// Check if claims allow sending to a room.
    pub fn can_send(&self, room_id: &str) -> bool {
        match self.role {
            UserRole::Admin | UserRole::Operator | UserRole::Sender => {
                self.can_access_room(room_id)
            }
            UserRole::Receiver => false,
        }
    }

    /// Check if claims allow managing rooms.
    pub fn can_manage_rooms(&self) -> bool {
        matches!(self.role, UserRole::Admin | UserRole::Operator)
    }

    /// Check if claims allow admin operations.
    pub fn is_admin(&self) -> bool {
        matches!(self.role, UserRole::Admin)
    }
}

/// Generate a JWT token.
pub fn generate_token(claims: &Claims, config: &AuthConfig) -> Result<String> {
    let key = EncodingKey::from_secret(config.secret.as_bytes());
    encode(&Header::default(), claims, &key).map_err(Error::Jwt)
}

/// Validate a JWT token.
pub fn validate_token(token: &str, config: &AuthConfig) -> Result<Claims> {
    let key = DecodingKey::from_secret(config.secret.as_bytes());
    let mut validation = Validation::default();
    validation.set_issuer(&[&config.issuer]);

    let token_data = decode::<Claims>(token, &key, &validation)?;

    // Check expiration
    if token_data.claims.exp < Utc::now().timestamp() {
        return Err(Error::TokenExpired);
    }

    Ok(token_data.claims)
}

/// Extractor for authenticated claims.
pub struct AuthenticatedClaims(pub Claims);

impl<S> FromRequestParts<S> for AuthenticatedClaims
where
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self> {
        // Get auth config from extensions
        let config = parts
            .extensions
            .get::<AuthConfig>()
            .cloned()
            .unwrap_or_default();

        // If auth is disabled, return mock admin claims
        if !config.enabled {
            return Ok(AuthenticatedClaims(Claims::admin("anonymous", &config)));
        }

        // Extract Authorization header
        let auth_header = parts
            .headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Error::AuthenticationFailed("Missing Authorization header".into()))?;

        // Parse Bearer token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::AuthenticationFailed("Invalid Authorization format".into()))?;

        // Validate token
        let claims = validate_token(token, &config)?;

        Ok(AuthenticatedClaims(claims))
    }
}

/// Extractor for optional claims (authentication not required).
pub struct OptionalClaims(pub Option<Claims>);

impl<S> FromRequestParts<S> for OptionalClaims
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> std::result::Result<Self, Self::Rejection> {
        match AuthenticatedClaims::from_request_parts(parts, state).await {
            Ok(AuthenticatedClaims(claims)) => Ok(OptionalClaims(Some(claims))),
            Err(_) => Ok(OptionalClaims(None)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_generation_and_validation() {
        let config = AuthConfig::new("test-secret");
        let claims = Claims::admin("user-1", &config);

        let token = generate_token(&claims, &config).unwrap();
        let validated = validate_token(&token, &config).unwrap();

        assert_eq!(validated.sub, "user-1");
        assert_eq!(validated.role, UserRole::Admin);
    }

    #[test]
    fn test_room_access() {
        let config = AuthConfig::default();

        // Admin can access all rooms
        let admin = Claims::admin("admin", &config);
        assert!(admin.can_access_room("any-room"));
        assert!(admin.can_send("any-room"));

        // Sender with specific rooms
        let sender = Claims::sender("sender", vec!["room-1".into()], &config);
        assert!(sender.can_access_room("room-1"));
        assert!(!sender.can_access_room("room-2"));
        assert!(sender.can_send("room-1"));

        // Receiver cannot send
        let receiver = Claims::receiver("receiver", vec!["room-1".into()], &config);
        assert!(receiver.can_access_room("room-1"));
        assert!(!receiver.can_send("room-1"));
    }
}

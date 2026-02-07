//! USSP Control Plane
//!
//! Provides REST API for managing SFU servers, rooms, and participants.

pub mod api;
pub mod auth;
pub mod error;
pub mod state;

pub use api::create_router;
pub use auth::{AuthConfig, Claims};
pub use error::{Error, Result};
pub use state::AppState;

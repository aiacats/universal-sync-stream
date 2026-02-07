//! USSP SFU (Selective Forwarding Unit) Server
//!
//! Provides packet forwarding, session management, and failover capabilities
//! for USSP streams.

pub mod config;
pub mod error;
pub mod forwarder;
pub mod participant;
pub mod room;
pub mod server;
pub mod stats;

pub use config::SfuConfig;
pub use error::{Error, Result};
pub use forwarder::Forwarder;
pub use participant::{Participant, ParticipantId, ParticipantRole, ParticipantState, ParticipantSummary};
pub use room::{Room, RoomId, RoomState, RoomStats, RoomSummary};
pub use server::SfuServer;
pub use stats::{SfuStats, SfuStatsSnapshot};

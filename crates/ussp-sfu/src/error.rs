//! SFU error types.

use thiserror::Error;

/// SFU result type.
pub type Result<T> = std::result::Result<T, Error>;

/// SFU errors.
#[derive(Debug, Error)]
pub enum Error {
    /// Room not found.
    #[error("Room not found: {0}")]
    RoomNotFound(String),

    /// Participant not found.
    #[error("Participant not found: {0}")]
    ParticipantNotFound(String),

    /// Room already exists.
    #[error("Room already exists: {0}")]
    RoomAlreadyExists(String),

    /// Participant already exists.
    #[error("Participant already exists in room")]
    ParticipantAlreadyExists,

    /// Maximum participants reached.
    #[error("Maximum participants reached: {0}")]
    MaxParticipantsReached(usize),

    /// No sender in room.
    #[error("No sender in room: {0}")]
    NoSender(String),

    /// Multiple senders not allowed.
    #[error("Room already has a primary sender")]
    MultipleSendersNotAllowed,

    /// Invalid role transition.
    #[error("Invalid role transition: {from:?} -> {to:?}")]
    InvalidRoleTransition {
        from: crate::ParticipantRole,
        to: crate::ParticipantRole,
    },

    /// Session error.
    #[error("Session error: {0}")]
    Session(String),

    /// Network error.
    #[error("Network error: {0}")]
    Network(#[from] std::io::Error),

    /// Protocol error.
    #[error("Protocol error: {0}")]
    Protocol(#[from] ussp::Error),

    /// Configuration error.
    #[error("Configuration error: {0}")]
    Config(String),

    /// Failover in progress.
    #[error("Failover in progress")]
    FailoverInProgress,

    /// Internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

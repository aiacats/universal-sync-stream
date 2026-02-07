//! Error types for USSP.

use thiserror::Error;

/// Result type alias using USSP Error.
pub type Result<T> = std::result::Result<T, Error>;

/// USSP error types.
#[derive(Error, Debug)]
pub enum Error {
    /// I/O error from network operations.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Packet encoding/decoding error.
    #[error("Codec error: {0}")]
    Codec(String),

    /// Invalid packet header.
    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    /// Invalid magic bytes in packet.
    #[error("Invalid magic bytes")]
    InvalidMagic,

    /// Unsupported protocol version.
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(u8),

    /// Unknown packet type.
    #[error("Unknown packet type: {0}")]
    UnknownPacketType(u8),

    /// Buffer too small for operation.
    #[error("Buffer too small: need {needed} bytes, got {got}")]
    BufferTooSmall { needed: usize, got: usize },

    /// FEC encoding/decoding error.
    #[error("FEC error: {0}")]
    Fec(String),

    /// Session error.
    #[error("Session error: {0}")]
    Session(String),

    /// Timeout error.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Frame reassembly error.
    #[error("Reassembly error: {0}")]
    Reassembly(String),

    /// Synchronization error.
    #[error("Sync error: {0}")]
    Sync(String),

    /// Configuration error.
    #[error("Config error: {0}")]
    Config(String),
}

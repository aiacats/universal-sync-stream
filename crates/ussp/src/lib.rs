//! # USSP - Universal Sync Stream Protocol
//!
//! A Rust library for synchronized streaming of video and multi-track audio over UDP.
//!
//! ## Features
//!
//! - Single video stream with multiple audio tracks
//! - Microsecond-precision synchronization using PTS
//! - Forward Error Correction (FEC) using Reed-Solomon codes
//! - Designed for internet streaming with packet loss tolerance
//!
//! ## Example
//!
//! ```rust,no_run
//! use ussp::transport::{Sender, SenderConfig};
//! use ussp::protocol::{VideoFrame, AudioFrame, CodecType};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = SenderConfig::default();
//!     let sender = Sender::bind("0.0.0.0:5000", "192.168.1.100:5001", config).await?;
//!
//!     // Send video and audio frames...
//!     Ok(())
//! }
//! ```

pub mod error;
pub mod fec;
pub mod protocol;
pub mod sync;
pub mod transport;

pub use error::{Error, Result};

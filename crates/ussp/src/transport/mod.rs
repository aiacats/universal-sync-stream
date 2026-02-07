//! Transport layer for sending and receiving USSP packets.

mod receiver;
mod sender;
mod session;

pub use receiver::{Receiver, ReceiverConfig, ReceiverEvent};
pub use sender::{Sender, SenderConfig};
pub use session::{Session, SessionConfig, SessionState};

/// Default UDP port for USSP.
pub const DEFAULT_PORT: u16 = 5000;

/// Maximum packet size including headers.
pub const MAX_PACKET_SIZE: usize = 1472; // Typical MTU (1500) - IP (20) - UDP (8)

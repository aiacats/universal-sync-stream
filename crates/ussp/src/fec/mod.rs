//! Forward Error Correction (FEC) module using Reed-Solomon codes.

mod decoder;
mod encoder;

pub use decoder::{DecodeResult, FecDecoder};
pub use encoder::{FecEncoder, FecGroup};

/// Default number of data shards.
pub const DEFAULT_DATA_SHARDS: usize = 10;

/// Default number of parity shards.
pub const DEFAULT_PARITY_SHARDS: usize = 3;

/// FEC configuration.
#[derive(Debug, Clone, Copy)]
pub struct FecConfig {
    /// Number of data shards (k).
    pub data_shards: usize,
    /// Number of parity shards (m).
    pub parity_shards: usize,
}

impl Default for FecConfig {
    fn default() -> Self {
        Self {
            data_shards: DEFAULT_DATA_SHARDS,
            parity_shards: DEFAULT_PARITY_SHARDS,
        }
    }
}

impl FecConfig {
    /// Create a new FEC configuration.
    pub fn new(data_shards: usize, parity_shards: usize) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }

    /// Total number of shards (data + parity).
    pub fn total_shards(&self) -> usize {
        self.data_shards + self.parity_shards
    }
}

//! FEC encoder using Reed-Solomon codes.

use super::FecConfig;
use crate::error::{Error, Result};
use bytes::Bytes;
use reed_solomon_erasure::galois_8::ReedSolomon;

/// FEC encoder that generates parity shards for a group of data shards.
pub struct FecEncoder {
    config: FecConfig,
    rs: ReedSolomon,
    group_id: u32,
}

/// Result of encoding a group of packets.
#[derive(Debug, Clone)]
pub struct FecGroup {
    /// Group identifier.
    pub group_id: u32,
    /// All shards (data + parity).
    pub shards: Vec<Bytes>,
    /// Number of data shards.
    pub data_shards: usize,
    /// Shard size (all shards have the same size).
    pub shard_size: usize,
}

impl FecEncoder {
    /// Create a new FEC encoder with the given configuration.
    pub fn new(config: FecConfig) -> Result<Self> {
        let rs = ReedSolomon::new(config.data_shards, config.parity_shards)
            .map_err(|e| Error::Fec(format!("Failed to create Reed-Solomon encoder: {}", e)))?;

        Ok(Self {
            config,
            rs,
            group_id: 0,
        })
    }

    /// Create a new FEC encoder with default configuration.
    pub fn with_defaults() -> Result<Self> {
        Self::new(FecConfig::default())
    }

    /// Get the current configuration.
    pub fn config(&self) -> FecConfig {
        self.config
    }

    /// Encode a group of data packets and generate parity shards.
    ///
    /// # Arguments
    /// * `data_packets` - The data packets to encode. Must have exactly `data_shards` elements.
    ///
    /// # Returns
    /// A `FecGroup` containing all data and parity shards.
    pub fn encode(&mut self, data_packets: Vec<Bytes>) -> Result<FecGroup> {
        if data_packets.len() != self.config.data_shards {
            return Err(Error::Fec(format!(
                "Expected {} data packets, got {}",
                self.config.data_shards,
                data_packets.len()
            )));
        }

        // Find the maximum packet size
        let max_size = data_packets.iter().map(|p| p.len()).max().unwrap_or(0);

        if max_size == 0 {
            return Err(Error::Fec("Cannot encode empty packets".to_string()));
        }

        // Create padded shards (all same size)
        let mut shards: Vec<Vec<u8>> = data_packets
            .iter()
            .map(|p| {
                let mut shard = p.to_vec();
                shard.resize(max_size, 0);
                shard
            })
            .collect();

        // Add empty parity shards
        for _ in 0..self.config.parity_shards {
            shards.push(vec![0u8; max_size]);
        }

        // Encode (generate parity)
        self.rs
            .encode(&mut shards)
            .map_err(|e| Error::Fec(format!("Reed-Solomon encoding failed: {}", e)))?;

        let group_id = self.group_id;
        self.group_id = self.group_id.wrapping_add(1);

        Ok(FecGroup {
            group_id,
            shards: shards.into_iter().map(Bytes::from).collect(),
            data_shards: self.config.data_shards,
            shard_size: max_size,
        })
    }

    /// Encode packets with padding for incomplete groups.
    ///
    /// If there are fewer packets than `data_shards`, the remaining slots
    /// are filled with empty packets.
    pub fn encode_partial(&mut self, mut data_packets: Vec<Bytes>) -> Result<FecGroup> {
        let original_count = data_packets.len();

        if original_count > self.config.data_shards {
            return Err(Error::Fec(format!(
                "Too many packets: {} > {}",
                original_count, self.config.data_shards
            )));
        }

        if original_count == 0 {
            return Err(Error::Fec("Cannot encode empty group".to_string()));
        }

        // Pad with empty packets if needed
        let max_size = data_packets.iter().map(|p| p.len()).max().unwrap_or(1);
        while data_packets.len() < self.config.data_shards {
            data_packets.push(Bytes::from(vec![0u8; max_size]));
        }

        self.encode(data_packets)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encoder_creation() {
        let encoder = FecEncoder::with_defaults().unwrap();
        assert_eq!(encoder.config().data_shards, 10);
        assert_eq!(encoder.config().parity_shards, 3);
    }

    #[test]
    fn test_encode_group() {
        let mut encoder = FecEncoder::new(FecConfig::new(4, 2)).unwrap();

        let data: Vec<Bytes> = (0..4)
            .map(|i| Bytes::from(vec![i as u8; 100]))
            .collect();

        let group = encoder.encode(data).unwrap();

        assert_eq!(group.group_id, 0);
        assert_eq!(group.shards.len(), 6); // 4 data + 2 parity
        assert_eq!(group.data_shards, 4);
        assert_eq!(group.shard_size, 100);
    }

    #[test]
    fn test_encode_partial() {
        let mut encoder = FecEncoder::new(FecConfig::new(4, 2)).unwrap();

        let data: Vec<Bytes> = (0..2)
            .map(|i| Bytes::from(vec![i as u8; 100]))
            .collect();

        let group = encoder.encode_partial(data).unwrap();

        assert_eq!(group.shards.len(), 6);
        assert_eq!(group.data_shards, 4);
    }
}

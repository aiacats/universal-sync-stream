//! FEC decoder for recovering lost packets.

use super::FecConfig;
use crate::error::{Error, Result};
use bytes::Bytes;
use reed_solomon_erasure::galois_8::ReedSolomon;
use std::collections::HashMap;

/// FEC decoder that recovers lost packets from parity shards.
pub struct FecDecoder {
    config: FecConfig,
    rs: ReedSolomon,
    /// Pending groups waiting for enough shards to decode.
    pending_groups: HashMap<u32, PendingGroup>,
}

/// A group of shards pending reconstruction.
struct PendingGroup {
    /// Received shards (Some = received, None = missing).
    shards: Vec<Option<Vec<u8>>>,
    /// Shard size.
    shard_size: usize,
    /// Number of received shards.
    received_count: usize,
}

/// Result of attempting to decode a group.
#[derive(Debug)]
pub enum DecodeResult {
    /// Successfully recovered the data shards.
    Recovered(Vec<Bytes>),
    /// Need more shards to recover.
    NeedMore { received: usize, needed: usize },
    /// Too many shards lost; cannot recover.
    Unrecoverable,
}

impl FecDecoder {
    /// Create a new FEC decoder with the given configuration.
    pub fn new(config: FecConfig) -> Result<Self> {
        let rs = ReedSolomon::new(config.data_shards, config.parity_shards)
            .map_err(|e| Error::Fec(format!("Failed to create Reed-Solomon decoder: {}", e)))?;

        Ok(Self {
            config,
            rs,
            pending_groups: HashMap::new(),
        })
    }

    /// Create a new FEC decoder with default configuration.
    pub fn with_defaults() -> Result<Self> {
        Self::new(FecConfig::default())
    }

    /// Add a received shard to a group.
    ///
    /// # Arguments
    /// * `group_id` - The FEC group identifier.
    /// * `shard_index` - Index of this shard within the group.
    /// * `shard_data` - The shard data.
    /// * `shard_size` - Expected size of all shards in this group.
    ///
    /// # Returns
    /// `DecodeResult` indicating whether recovery is complete, needs more shards, or failed.
    pub fn add_shard(
        &mut self,
        group_id: u32,
        shard_index: usize,
        shard_data: Bytes,
        shard_size: usize,
    ) -> DecodeResult {
        let total = self.config.total_shards();

        if shard_index >= total {
            return DecodeResult::Unrecoverable;
        }

        let group = self.pending_groups.entry(group_id).or_insert_with(|| {
            PendingGroup {
                shards: vec![None; total],
                shard_size,
                received_count: 0,
            }
        });

        // Add the shard if not already present
        if group.shards[shard_index].is_none() {
            let mut data = shard_data.to_vec();
            data.resize(shard_size, 0);
            group.shards[shard_index] = Some(data);
            group.received_count += 1;
        }

        // Check if we can decode
        if group.received_count >= self.config.data_shards {
            self.try_decode(group_id)
        } else {
            DecodeResult::NeedMore {
                received: group.received_count,
                needed: self.config.data_shards,
            }
        }
    }

    /// Try to decode a group.
    fn try_decode(&mut self, group_id: u32) -> DecodeResult {
        let group = match self.pending_groups.remove(&group_id) {
            Some(g) => g,
            None => return DecodeResult::Unrecoverable,
        };

        // Convert to the format expected by reed-solomon-erasure
        let mut shards: Vec<Option<Vec<u8>>> = group.shards;

        // Attempt reconstruction
        match self.rs.reconstruct(&mut shards) {
            Ok(()) => {
                // Extract data shards
                let data_shards: Vec<Bytes> = shards
                    .into_iter()
                    .take(self.config.data_shards)
                    .map(|s| Bytes::from(s.unwrap_or_default()))
                    .collect();

                DecodeResult::Recovered(data_shards)
            }
            Err(_) => DecodeResult::Unrecoverable,
        }
    }

    /// Check if all data shards are present (no recovery needed).
    pub fn has_all_data(&self, group_id: u32) -> bool {
        if let Some(group) = self.pending_groups.get(&group_id) {
            group
                .shards
                .iter()
                .take(self.config.data_shards)
                .all(|s| s.is_some())
        } else {
            false
        }
    }

    /// Clear old pending groups to prevent memory leaks.
    pub fn clear_old_groups(&mut self, max_groups: usize) {
        if self.pending_groups.len() > max_groups {
            // Simple strategy: remove oldest groups
            let to_remove: Vec<u32> = self
                .pending_groups
                .keys()
                .take(self.pending_groups.len() - max_groups)
                .copied()
                .collect();

            for id in to_remove {
                self.pending_groups.remove(&id);
            }
        }
    }

    /// Get the number of pending groups.
    pub fn pending_count(&self) -> usize {
        self.pending_groups.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fec::FecEncoder;

    #[test]
    fn test_decode_no_loss() {
        let config = FecConfig::new(4, 2);
        let mut encoder = FecEncoder::new(config).unwrap();
        let mut decoder = FecDecoder::new(config).unwrap();

        let data: Vec<Bytes> = (0..4)
            .map(|i| Bytes::from(vec![i as u8; 100]))
            .collect();

        let group = encoder.encode(data.clone()).unwrap();

        // Add all data shards
        for (i, shard) in group.shards.iter().enumerate().take(4) {
            let result = decoder.add_shard(group.group_id, i, shard.clone(), group.shard_size);
            if i == 3 {
                if let DecodeResult::Recovered(recovered) = result {
                    for (j, shard) in recovered.iter().enumerate() {
                        assert_eq!(shard.as_ref(), data[j].as_ref());
                    }
                } else {
                    panic!("Expected Recovered");
                }
            }
        }
    }

    #[test]
    fn test_decode_with_loss() {
        let config = FecConfig::new(4, 2);
        let mut encoder = FecEncoder::new(config).unwrap();
        let mut decoder = FecDecoder::new(config).unwrap();

        let data: Vec<Bytes> = (0..4)
            .map(|i| Bytes::from(vec![i as u8; 100]))
            .collect();

        let group = encoder.encode(data.clone()).unwrap();

        // Simulate loss of shards 1 and 2, use parity shards instead
        let indices = [0, 3, 4, 5]; // data 0, data 3, parity 0, parity 1

        for (count, &i) in indices.iter().enumerate() {
            let result =
                decoder.add_shard(group.group_id, i, group.shards[i].clone(), group.shard_size);

            if count == 3 {
                if let DecodeResult::Recovered(recovered) = result {
                    // Verify recovered data matches original
                    assert_eq!(recovered[0].as_ref(), data[0].as_ref());
                    assert_eq!(recovered[3].as_ref(), data[3].as_ref());
                    // Shards 1 and 2 should be recovered
                    assert_eq!(recovered[1].as_ref(), data[1].as_ref());
                    assert_eq!(recovered[2].as_ref(), data[2].as_ref());
                } else {
                    panic!("Expected Recovered, got {:?}", result);
                }
            }
        }
    }

    #[test]
    fn test_unrecoverable_loss() {
        let config = FecConfig::new(4, 2);
        let mut encoder = FecEncoder::new(config).unwrap();
        let mut decoder = FecDecoder::new(config).unwrap();

        let data: Vec<Bytes> = (0..4)
            .map(|i| Bytes::from(vec![i as u8; 100]))
            .collect();

        let group = encoder.encode(data).unwrap();

        // Only add 3 shards (need 4 to recover)
        for i in 0..3 {
            let result =
                decoder.add_shard(group.group_id, i, group.shards[i].clone(), group.shard_size);

            if let DecodeResult::NeedMore { received, needed } = result {
                assert_eq!(received, i + 1);
                assert_eq!(needed, 4);
            }
        }
    }
}

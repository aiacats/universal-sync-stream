//! Clock synchronization between sender and receiver.

use std::collections::VecDeque;
use std::time::{SystemTime, UNIX_EPOCH};

/// Clock synchronization configuration.
#[derive(Debug, Clone)]
pub struct ClockSyncConfig {
    /// Number of samples to keep for averaging.
    pub sample_count: usize,
    /// Minimum samples needed before providing offset.
    pub min_samples: usize,
    /// Maximum allowed clock drift in microseconds.
    pub max_drift_us: i64,
}

impl Default for ClockSyncConfig {
    fn default() -> Self {
        Self {
            sample_count: 10,
            min_samples: 3,
            max_drift_us: 1_000_000, // 1 second
        }
    }
}

/// A sample for clock synchronization.
#[derive(Debug, Clone, Copy)]
struct ClockSample {
    /// Local timestamp when request was sent.
    pub local_send_time: u64,
    /// Remote timestamp from response.
    pub remote_time: u64,
    /// Local timestamp when response was received.
    pub local_recv_time: u64,
    /// Round-trip time.
    pub rtt: u64,
}

impl ClockSample {
    /// Calculate the estimated offset (remote - local).
    fn offset(&self) -> i64 {
        // Using NTP-style offset calculation
        // offset = ((T2 - T1) + (T3 - T4)) / 2
        // Where T1 = local_send, T2 = remote_recv, T3 = remote_send, T4 = local_recv
        // Simplified: assuming remote_recv ≈ remote_send
        let t1 = self.local_send_time as i64;
        let t2 = self.remote_time as i64;
        let t4 = self.local_recv_time as i64;

        // offset = t2 - (t1 + t4) / 2
        t2 - (t1 + t4) / 2
    }
}

/// Clock synchronization state.
pub struct ClockSync {
    config: ClockSyncConfig,
    samples: VecDeque<ClockSample>,
    pending_request: Option<u64>,
}

impl ClockSync {
    /// Create a new clock sync with the given configuration.
    pub fn new(config: ClockSyncConfig) -> Self {
        Self {
            samples: VecDeque::with_capacity(config.sample_count),
            config,
            pending_request: None,
        }
    }

    /// Create a new clock sync with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(ClockSyncConfig::default())
    }

    /// Get the current timestamp in microseconds.
    pub fn current_time_us() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64
    }

    /// Start a sync request. Returns the local timestamp to include in the request.
    pub fn start_request(&mut self) -> u64 {
        let now = Self::current_time_us();
        self.pending_request = Some(now);
        now
    }

    /// Process a sync response.
    ///
    /// # Arguments
    /// * `remote_time` - The timestamp from the remote side.
    pub fn process_response(&mut self, remote_time: u64) {
        let local_recv_time = Self::current_time_us();

        if let Some(local_send_time) = self.pending_request.take() {
            let rtt = local_recv_time.saturating_sub(local_send_time);

            let sample = ClockSample {
                local_send_time,
                remote_time,
                local_recv_time,
                rtt,
            };

            // Only accept samples with reasonable RTT
            if sample.offset().abs() < self.config.max_drift_us {
                if self.samples.len() >= self.config.sample_count {
                    self.samples.pop_front();
                }
                self.samples.push_back(sample);
            }
        }
    }

    /// Get the estimated clock offset in microseconds (remote - local).
    ///
    /// Returns `None` if there are not enough samples yet.
    pub fn offset(&self) -> Option<i64> {
        if self.samples.len() < self.config.min_samples {
            return None;
        }

        // Use median of offsets for robustness
        let mut offsets: Vec<i64> = self.samples.iter().map(|s| s.offset()).collect();
        offsets.sort();

        Some(offsets[offsets.len() / 2])
    }

    /// Get the average round-trip time in microseconds.
    pub fn average_rtt(&self) -> Option<u64> {
        if self.samples.is_empty() {
            return None;
        }

        let sum: u64 = self.samples.iter().map(|s| s.rtt).sum();
        Some(sum / self.samples.len() as u64)
    }

    /// Convert a local timestamp to remote time.
    pub fn local_to_remote(&self, local_time: u64) -> Option<u64> {
        self.offset().map(|off| {
            if off >= 0 {
                local_time.wrapping_add(off as u64)
            } else {
                local_time.wrapping_sub((-off) as u64)
            }
        })
    }

    /// Convert a remote timestamp to local time.
    pub fn remote_to_local(&self, remote_time: u64) -> Option<u64> {
        self.offset().map(|off| {
            if off >= 0 {
                remote_time.wrapping_sub(off as u64)
            } else {
                remote_time.wrapping_add((-off) as u64)
            }
        })
    }

    /// Get the number of collected samples.
    pub fn sample_count(&self) -> usize {
        self.samples.len()
    }

    /// Clear all samples.
    pub fn reset(&mut self) {
        self.samples.clear();
        self.pending_request = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clock_sync_basic() {
        let mut sync = ClockSync::with_defaults();

        // Simulate perfect sync (no offset, no RTT)
        for _ in 0..5 {
            let send_time = ClockSync::current_time_us();
            sync.pending_request = Some(send_time);
            sync.process_response(send_time);
        }

        assert!(sync.offset().is_some());
        assert!(sync.offset().unwrap().abs() < 1000); // Should be close to 0
    }

    #[test]
    fn test_clock_sync_with_offset() {
        let mut sync = ClockSync::with_defaults();

        // Simulate 100ms offset
        let offset: i64 = 100_000;

        for i in 0..5 {
            let send_time = 1000000 + i * 1000;
            let recv_time = send_time + 10; // 10us RTT
            let remote_time = (send_time as i64 + offset + 5) as u64; // remote is ahead

            sync.pending_request = Some(send_time);
            sync.samples.push_back(ClockSample {
                local_send_time: send_time,
                remote_time,
                local_recv_time: recv_time,
                rtt: 10,
            });
        }

        let calculated_offset = sync.offset().unwrap();
        assert!((calculated_offset - offset).abs() < 100);
    }
}

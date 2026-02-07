//! SFU configuration.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// SFU server configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SfuConfig {
    /// Server ID (unique within cluster).
    pub server_id: String,

    /// UDP bind address for media.
    pub media_bind_addr: String,

    /// Control API bind address.
    pub control_bind_addr: String,

    /// Maximum rooms allowed.
    pub max_rooms: usize,

    /// Maximum participants per room.
    pub max_participants_per_room: usize,

    /// Room configuration.
    pub room: RoomConfig,

    /// Failover configuration.
    pub failover: FailoverConfig,

    /// Cluster configuration.
    pub cluster: ClusterConfig,
}

impl Default for SfuConfig {
    fn default() -> Self {
        Self {
            server_id: "sfu-1".to_string(),
            media_bind_addr: "0.0.0.0:5000".to_string(),
            control_bind_addr: "0.0.0.0:8080".to_string(),
            max_rooms: 100,
            max_participants_per_room: 100,
            room: RoomConfig::default(),
            failover: FailoverConfig::default(),
            cluster: ClusterConfig::default(),
        }
    }
}

impl SfuConfig {
    /// Create a test configuration with ephemeral ports.
    #[cfg(test)]
    pub fn test() -> Self {
        Self {
            server_id: "test-sfu".to_string(),
            media_bind_addr: "127.0.0.1:0".to_string(),
            control_bind_addr: "127.0.0.1:0".to_string(),
            ..Default::default()
        }
    }
}

/// Room configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
    /// Allow backup senders.
    pub allow_backup_sender: bool,

    /// Heartbeat interval.
    #[serde(with = "humantime_serde")]
    pub heartbeat_interval: Duration,

    /// Heartbeat timeout (participant considered dead).
    #[serde(with = "humantime_serde")]
    pub heartbeat_timeout: Duration,

    /// Session timeout (room closed if no activity).
    #[serde(with = "humantime_serde")]
    pub session_timeout: Duration,
}

impl Default for RoomConfig {
    fn default() -> Self {
        Self {
            allow_backup_sender: true,
            heartbeat_interval: Duration::from_secs(1),
            heartbeat_timeout: Duration::from_secs(3),
            session_timeout: Duration::from_secs(300),
        }
    }
}

/// Failover configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverConfig {
    /// Enable automatic failover.
    pub enabled: bool,

    /// Time to wait before promoting backup sender.
    #[serde(with = "humantime_serde")]
    pub promotion_delay: Duration,

    /// Maximum failover attempts.
    pub max_attempts: u32,
}

impl Default for FailoverConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            promotion_delay: Duration::from_millis(500),
            max_attempts: 3,
        }
    }
}

/// Cluster configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterConfig {
    /// Enable clustering.
    pub enabled: bool,

    /// Peer SFU addresses.
    pub peers: Vec<String>,

    /// Cluster sync interval.
    #[serde(with = "humantime_serde")]
    pub sync_interval: Duration,
}

impl Default for ClusterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            peers: Vec::new(),
            sync_interval: Duration::from_secs(1),
        }
    }
}

mod humantime_serde {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_millis() as u64)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let millis = u64::deserialize(deserializer)?;
        Ok(Duration::from_millis(millis))
    }
}

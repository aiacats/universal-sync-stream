//! Sidecar process manager for the USSP SFU Server.

use std::collections::VecDeque;
use std::sync::Arc;
use tauri::AppHandle;
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;
use tokio::sync::RwLock;
use tracing::info;

/// Maximum number of log lines to keep in memory.
const MAX_LOG_LINES: usize = 1000;

/// Server launch configuration.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SidecarConfig {
    pub server_id: String,
    pub media_addr: String,
    pub control_addr: String,
    pub no_auth: bool,
    pub max_rooms: usize,
    pub max_participants: usize,
    pub log_level: String,
}

impl Default for SidecarConfig {
    fn default() -> Self {
        Self {
            server_id: "sfu-1".into(),
            media_addr: "0.0.0.0:5000".into(),
            control_addr: "0.0.0.0:8080".into(),
            no_auth: true,
            max_rooms: 100,
            max_participants: 100,
            log_level: "info".into(),
        }
    }
}

/// Manages the sidecar SFU server process lifecycle.
pub struct SidecarState {
    child: RwLock<Option<CommandChild>>,
    logs: RwLock<VecDeque<String>>,
    running: RwLock<bool>,
    config: RwLock<SidecarConfig>,
}

impl SidecarState {
    /// Create a new sidecar state.
    pub fn new() -> Self {
        Self {
            child: RwLock::new(None),
            logs: RwLock::new(VecDeque::new()),
            running: RwLock::new(false),
            config: RwLock::new(SidecarConfig::default()),
        }
    }

    /// Start the SFU server sidecar process.
    pub async fn start(self: &Arc<Self>, app: &AppHandle) -> Result<(), String> {
        if *self.running.read().await {
            return Err("Server is already running".into());
        }

        let config = self.config.read().await.clone();

        // Build CLI arguments
        let mut args: Vec<String> = vec![
            "--server-id".into(),
            config.server_id,
            "--media-addr".into(),
            config.media_addr,
            "--control-addr".into(),
            config.control_addr,
            "--max-rooms".into(),
            config.max_rooms.to_string(),
            "--max-participants".into(),
            config.max_participants.to_string(),
            "--log-level".into(),
            config.log_level,
        ];
        if config.no_auth {
            args.push("--no-auth".into());
        }

        info!("Starting SFU server sidecar with args: {:?}", args);

        // Spawn sidecar
        let (mut rx, child) = app
            .shell()
            .sidecar("ussp-server")
            .map_err(|e| format!("Failed to create sidecar command: {e}"))?
            .args(&args)
            .spawn()
            .map_err(|e| format!("Failed to spawn sidecar: {e}"))?;

        *self.child.write().await = Some(child);
        *self.running.write().await = true;
        self.logs.write().await.clear();

        // Spawn log reader task
        let state = Arc::clone(self);
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    CommandEvent::Stdout(line) | CommandEvent::Stderr(line) => {
                        let text = String::from_utf8_lossy(&line).to_string();
                        let mut logs = state.logs.write().await;
                        logs.push_back(text);
                        while logs.len() > MAX_LOG_LINES {
                            logs.pop_front();
                        }
                    }
                    CommandEvent::Terminated(status) => {
                        let msg = format!(
                            "[Process terminated with code: {:?}]",
                            status.code
                        );
                        info!("{}", msg);
                        state.logs.write().await.push_back(msg);
                        *state.running.write().await = false;
                        *state.child.write().await = None;
                        break;
                    }
                    _ => {}
                }
            }
        });

        info!("SFU server sidecar started");
        Ok(())
    }

    /// Stop the SFU server sidecar process.
    pub async fn stop(&self) -> Result<(), String> {
        let mut child = self.child.write().await;
        if let Some(c) = child.take() {
            info!("Stopping SFU server sidecar");
            c.kill().map_err(|e| format!("Failed to stop server: {e}"))?;
        }
        *self.running.write().await = false;
        Ok(())
    }

    /// Check if the sidecar is currently running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get recent log output from the sidecar.
    pub async fn get_logs(&self) -> Vec<String> {
        self.logs.read().await.iter().cloned().collect()
    }

    /// Update the sidecar configuration (only when not running).
    pub async fn set_config(&self, config: SidecarConfig) {
        *self.config.write().await = config;
    }

    /// Get the current sidecar configuration.
    pub async fn get_config(&self) -> SidecarConfig {
        self.config.read().await.clone()
    }
}

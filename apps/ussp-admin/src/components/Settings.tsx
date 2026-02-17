import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { SidecarConfig } from "../types";

interface SettingsProps {
  serverUrl: string;
  connected: boolean;
  onConnect: (url: string, apiKey: string) => Promise<void>;
  onDisconnect: () => Promise<void>;
  error: string | null;
}

function Settings({ serverUrl, connected, onConnect, onDisconnect, error }: SettingsProps) {
  const [url, setUrl] = useState(serverUrl);
  const [apiKey, setApiKey] = useState("");
  const [connecting, setConnecting] = useState(false);

  // Sidecar config state
  const [sidecarConfig, setSidecarConfig] = useState<SidecarConfig | null>(null);
  const [serverRunning, setServerRunning] = useState(false);
  const [configSaved, setConfigSaved] = useState(false);
  const [configError, setConfigError] = useState<string | null>(null);

  useEffect(() => {
    invoke<SidecarConfig>("get_sidecar_config").then(setSidecarConfig);
    invoke<boolean>("get_server_status").then(setServerRunning);
  }, []);

  const handleConnect = async () => {
    setConnecting(true);
    try {
      await onConnect(url, apiKey);
    } finally {
      setConnecting(false);
    }
  };

  const handleDisconnect = async () => {
    await onDisconnect();
  };

  const handleSaveSidecarConfig = async () => {
    if (!sidecarConfig) return;
    setConfigError(null);
    setConfigSaved(false);
    try {
      await invoke("set_sidecar_config", { config: sidecarConfig });
      setConfigSaved(true);
      setTimeout(() => setConfigSaved(false), 3000);
    } catch (e) {
      setConfigError(String(e));
    }
  };

  const updateConfig = (field: keyof SidecarConfig, value: string | boolean | number) => {
    if (!sidecarConfig) return;
    setSidecarConfig({ ...sidecarConfig, [field]: value });
  };

  return (
    <div className="settings-page">
      {/* Connection Settings */}
      <div className="card settings-card">
        <div className="card-header">
          <h2>Connection Settings</h2>
        </div>

        {error && <div className="error-message">{error}</div>}

        {connected ? (
          <>
            <div className="connection-info">
              <p>
                <strong>Status:</strong>{" "}
                <span className="badge badge-success">Connected</span>
              </p>
              <p style={{ marginTop: "8px" }}>
                <strong>Server:</strong>{" "}
                <span className="server-url">{serverUrl}</span>
              </p>
            </div>
            <button className="btn btn-danger" onClick={handleDisconnect}>
              Disconnect
            </button>
          </>
        ) : (
          <>
            <div className="form-group">
              <label>Control Plane URL</label>
              <input
                type="text"
                value={url}
                onChange={(e) => setUrl(e.target.value)}
                placeholder="http://localhost:8080"
              />
            </div>

            <div className="form-group">
              <label>API Key</label>
              <input
                type="password"
                value={apiKey}
                onChange={(e) => setApiKey(e.target.value)}
                placeholder="Enter your API key"
              />
            </div>

            <button
              className="btn btn-primary"
              onClick={handleConnect}
              disabled={connecting || !url || !apiKey}
            >
              {connecting ? "Connecting..." : "Connect"}
            </button>
          </>
        )}
      </div>

      {/* Local Server Settings */}
      {sidecarConfig && (
        <div className="card settings-card" style={{ marginTop: "16px" }}>
          <div className="card-header">
            <h2>Local Server Settings</h2>
            {serverRunning && (
              <span className="badge badge-warning">Stop server to edit</span>
            )}
          </div>

          {configError && <div className="error-message">{configError}</div>}
          {configSaved && (
            <div style={{ color: "var(--success)", fontSize: "13px", marginBottom: "12px" }}>
              Configuration saved.
            </div>
          )}

          <div className="form-group">
            <label>Server ID</label>
            <input
              type="text"
              value={sidecarConfig.server_id}
              onChange={(e) => updateConfig("server_id", e.target.value)}
              disabled={serverRunning}
            />
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
            <div className="form-group">
              <label>Media Address (UDP)</label>
              <input
                type="text"
                value={sidecarConfig.media_addr}
                onChange={(e) => updateConfig("media_addr", e.target.value)}
                disabled={serverRunning}
              />
            </div>
            <div className="form-group">
              <label>Control Address (HTTP)</label>
              <input
                type="text"
                value={sidecarConfig.control_addr}
                onChange={(e) => updateConfig("control_addr", e.target.value)}
                disabled={serverRunning}
              />
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
            <div className="form-group">
              <label>Max Rooms</label>
              <input
                type="number"
                value={sidecarConfig.max_rooms}
                onChange={(e) => updateConfig("max_rooms", parseInt(e.target.value) || 0)}
                disabled={serverRunning}
              />
            </div>
            <div className="form-group">
              <label>Max Participants</label>
              <input
                type="number"
                value={sidecarConfig.max_participants}
                onChange={(e) => updateConfig("max_participants", parseInt(e.target.value) || 0)}
                disabled={serverRunning}
              />
            </div>
          </div>

          <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: "12px" }}>
            <div className="form-group">
              <label>Log Level</label>
              <select
                value={sidecarConfig.log_level}
                onChange={(e) => updateConfig("log_level", e.target.value)}
                disabled={serverRunning}
              >
                <option value="error">error</option>
                <option value="warn">warn</option>
                <option value="info">info</option>
                <option value="debug">debug</option>
                <option value="trace">trace</option>
              </select>
            </div>
            <div className="form-group">
              <label style={{ display: "flex", alignItems: "center", gap: "8px" }}>
                <input
                  type="checkbox"
                  checked={sidecarConfig.no_auth}
                  onChange={(e) => updateConfig("no_auth", e.target.checked)}
                  disabled={serverRunning}
                />
                Disable Authentication
              </label>
            </div>
          </div>

          <button
            className="btn btn-primary"
            onClick={handleSaveSidecarConfig}
            disabled={serverRunning}
            style={{ marginTop: "8px" }}
          >
            Save Configuration
          </button>
        </div>
      )}

      {/* About */}
      <div style={{ marginTop: "16px", padding: "16px", background: "var(--bg-card)", borderRadius: "6px" }}>
        <h3 style={{ fontSize: "14px", marginBottom: "8px", color: "var(--text-primary)" }}>
          About USSP Admin
        </h3>
        <p style={{ fontSize: "13px", color: "var(--text-secondary)", lineHeight: "1.5" }}>
          USSP Admin is a management interface for USSP SFU (Selective Forwarding Unit) servers.
          The local SFU server starts automatically with this application. Use the Dashboard to
          control the server and view logs. Connect to the control plane to manage rooms and participants.
        </p>
      </div>
    </div>
  );
}

export default Settings;

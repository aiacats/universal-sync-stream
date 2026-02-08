import { useState } from "react";

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

  return (
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

      <div style={{ marginTop: "24px", padding: "16px", background: "var(--bg-card)", borderRadius: "6px" }}>
        <h3 style={{ fontSize: "14px", marginBottom: "8px", color: "var(--text-primary)" }}>
          About USSP Admin
        </h3>
        <p style={{ fontSize: "13px", color: "var(--text-secondary)", lineHeight: "1.5" }}>
          USSP Admin is a management interface for USSP SFU (Selective Forwarding Unit) servers.
          Connect to a control plane to manage rooms, monitor participants, and view server statistics.
        </p>
      </div>
    </div>
  );
}

export default Settings;

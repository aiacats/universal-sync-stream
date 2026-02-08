import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { ServerResponse } from "../types";

function Servers() {
  const [servers, setServers] = useState<ServerResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadServers = async () => {
    try {
      setError(null);
      const data = await invoke<ServerResponse[]>("get_servers");
      setServers(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadServers();
    const interval = setInterval(loadServers, 5000);
    return () => clearInterval(interval);
  }, []);

  const getLoadClass = (load: number): string => {
    if (load < 0.5) return "low";
    if (load < 0.8) return "medium";
    return "high";
  };

  if (loading) {
    return <div className="loading">Loading servers...</div>;
  }

  return (
    <div className="card">
      <div className="card-header">
        <h2>SFU Servers ({servers.length})</h2>
        <button className="btn btn-secondary" onClick={loadServers}>
          Refresh
        </button>
      </div>

      {error && <div className="error-message">{error}</div>}

      {servers.length === 0 ? (
        <div className="empty-state">
          <h3>No Servers</h3>
          <p>No SFU servers are currently registered with the control plane.</p>
        </div>
      ) : (
        <div style={{ display: "flex", flexDirection: "column", gap: "12px" }}>
          {servers.map((server) => (
            <div key={server.id} className="server-card">
              <div className={`server-status ${server.healthy ? "healthy" : "unhealthy"}`}></div>
              <div className="server-info">
                <div className="server-id">{server.id}</div>
                <div className="server-addr">
                  Media: {server.media_addr} | Control: {server.control_addr}
                </div>
              </div>
              <div className="server-stats">
                <div className="server-stat">
                  <div className="value">{server.room_count}</div>
                  <div className="label">Rooms</div>
                </div>
                <div className="server-stat">
                  <div className="value">{server.participant_count}</div>
                  <div className="label">Participants</div>
                </div>
                <div className="server-stat">
                  <div className="value">{(server.load * 100).toFixed(0)}%</div>
                  <div className="label">Load</div>
                  <div className="load-bar" style={{ marginTop: "4px" }}>
                    <div
                      className={`fill ${getLoadClass(server.load)}`}
                      style={{ width: `${server.load * 100}%` }}
                    ></div>
                  </div>
                </div>
              </div>
              <span className={`badge ${server.healthy ? "badge-success" : "badge-error"}`}>
                {server.healthy ? "Healthy" : "Unhealthy"}
              </span>
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

export default Servers;

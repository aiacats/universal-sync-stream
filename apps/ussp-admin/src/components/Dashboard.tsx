import { useState, useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GlobalStatsResponse, HealthResponse } from "../types";

function Dashboard() {
  const [stats, setStats] = useState<GlobalStatsResponse | null>(null);
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  // Sidecar state
  const [serverRunning, setServerRunning] = useState(false);
  const [serverLogs, setServerLogs] = useState<string[]>([]);
  const [showLogs, setShowLogs] = useState(false);
  const [serverAction, setServerAction] = useState(false);
  const logsEndRef = useRef<HTMLDivElement>(null);

  const loadData = async () => {
    try {
      setError(null);
      const [statsData, healthData] = await Promise.all([
        invoke<GlobalStatsResponse>("get_global_stats"),
        invoke<HealthResponse>("health_check"),
      ]);
      setStats(statsData);
      setHealth(healthData);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  const loadServerStatus = async () => {
    try {
      const running = await invoke<boolean>("get_server_status");
      setServerRunning(running);
    } catch {
      // ignore
    }
  };

  useEffect(() => {
    loadData();
    loadServerStatus();
    const interval = setInterval(() => {
      loadData();
      loadServerStatus();
    }, 5000);
    return () => clearInterval(interval);
  }, []);

  // Load logs when visible
  useEffect(() => {
    if (!showLogs) return;
    const loadLogs = async () => {
      try {
        const logs = await invoke<string[]>("get_server_logs");
        setServerLogs(logs);
      } catch {
        // ignore
      }
    };
    loadLogs();
    const interval = setInterval(loadLogs, 2000);
    return () => clearInterval(interval);
  }, [showLogs]);

  // Auto-scroll logs to bottom
  useEffect(() => {
    if (showLogs && logsEndRef.current) {
      logsEndRef.current.scrollIntoView({ behavior: "smooth" });
    }
  }, [serverLogs, showLogs]);

  const handleStartServer = async () => {
    setServerAction(true);
    try {
      await invoke("start_server");
    } catch (e) {
      setError(String(e));
    } finally {
      setServerAction(false);
      loadServerStatus();
    }
  };

  const handleStopServer = async () => {
    setServerAction(true);
    try {
      await invoke("stop_server");
    } catch (e) {
      setError(String(e));
    } finally {
      setServerAction(false);
      loadServerStatus();
    }
  };

  const handleRestartServer = async () => {
    setServerAction(true);
    try {
      await invoke("restart_server");
    } catch (e) {
      setError(String(e));
    } finally {
      setServerAction(false);
      loadServerStatus();
    }
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const formatNumber = (num: number): string => {
    if (num < 1000) return String(num);
    if (num < 1000000) return `${(num / 1000).toFixed(1)}K`;
    return `${(num / 1000000).toFixed(2)}M`;
  };

  return (
    <div className="dashboard">
      {/* Local SFU Server Control */}
      <div className="card">
        <div className="card-header">
          <h2>Local SFU Server</h2>
          <span className={`badge ${serverRunning ? "badge-success" : "badge-error"}`}>
            {serverRunning ? "Running" : "Stopped"}
          </span>
        </div>
        <div style={{ display: "flex", gap: "8px", marginBottom: "12px" }}>
          {serverRunning ? (
            <>
              <button
                className="btn btn-danger btn-small"
                onClick={handleStopServer}
                disabled={serverAction}
              >
                {serverAction ? "..." : "Stop"}
              </button>
              <button
                className="btn btn-secondary btn-small"
                onClick={handleRestartServer}
                disabled={serverAction}
              >
                {serverAction ? "..." : "Restart"}
              </button>
            </>
          ) : (
            <button
              className="btn btn-primary btn-small"
              onClick={handleStartServer}
              disabled={serverAction}
            >
              {serverAction ? "Starting..." : "Start"}
            </button>
          )}
          <button
            className="btn btn-secondary btn-small"
            onClick={() => setShowLogs(!showLogs)}
          >
            {showLogs ? "Hide Logs" : "Show Logs"}
          </button>
        </div>
        {showLogs && (
          <div className="server-logs">
            {serverLogs.length === 0 ? (
              <div className="log-line" style={{ fontStyle: "italic" }}>No logs yet</div>
            ) : (
              serverLogs.map((line, i) => (
                <div key={i} className="log-line">{line}</div>
              ))
            )}
            <div ref={logsEndRef} />
          </div>
        )}
      </div>

      {error && <div className="error-message">{error}</div>}

      {loading ? (
        <div className="loading">Loading dashboard...</div>
      ) : (
        <>
          {health && (
            <div className="card">
              <div className="card-header">
                <h2>Control Plane Status</h2>
                <span className="badge badge-success">{health.status}</span>
              </div>
              <p style={{ color: "var(--text-secondary)", fontSize: "14px" }}>
                Version: {health.version}
              </p>
            </div>
          )}

          {stats && (
            <div className="stats-overview">
              <div className="stat-card">
                <span className="label">Active Servers</span>
                <span className="value highlight">{stats.total_servers}</span>
              </div>
              <div className="stat-card">
                <span className="label">Active Rooms</span>
                <span className="value">{stats.total_rooms}</span>
              </div>
              <div className="stat-card">
                <span className="label">Total Participants</span>
                <span className="value">{stats.total_participants}</span>
              </div>
              <div className="stat-card">
                <span className="label">Packets Forwarded</span>
                <span className="value">{formatNumber(stats.total_packets_forwarded)}</span>
              </div>
              <div className="stat-card">
                <span className="label">Data Forwarded</span>
                <span className="value">{formatBytes(stats.total_bytes_forwarded)}</span>
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

export default Dashboard;

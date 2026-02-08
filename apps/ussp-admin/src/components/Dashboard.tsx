import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { GlobalStatsResponse, HealthResponse } from "../types";

function Dashboard() {
  const [stats, setStats] = useState<GlobalStatsResponse | null>(null);
  const [health, setHealth] = useState<HealthResponse | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

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

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 5000);
    return () => clearInterval(interval);
  }, []);

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

  if (loading) {
    return <div className="loading">Loading dashboard...</div>;
  }

  if (error) {
    return (
      <div className="dashboard">
        <div className="error-message">{error}</div>
        <button className="btn btn-primary" onClick={loadData}>
          Retry
        </button>
      </div>
    );
  }

  return (
    <div className="dashboard">
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
    </div>
  );
}

export default Dashboard;

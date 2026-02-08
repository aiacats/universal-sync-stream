import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RoomStats, ParticipantResponse } from "../types";

interface RoomDetailProps {
  roomId: string;
  onBack: () => void;
}

function RoomDetail({ roomId, onBack }: RoomDetailProps) {
  const [stats, setStats] = useState<RoomStats | null>(null);
  const [participants, setParticipants] = useState<ParticipantResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const loadData = async () => {
    try {
      setError(null);
      const [statsData, participantsData] = await Promise.all([
        invoke<RoomStats>("get_room_stats", { roomId }),
        invoke<ParticipantResponse[]>("get_participants", { roomId }),
      ]);
      setStats(statsData);
      setParticipants(participantsData);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadData();
    const interval = setInterval(loadData, 2000);
    return () => clearInterval(interval);
  }, [roomId]);

  const handleRemoveParticipant = async (participantId: string) => {
    if (!confirm(`Remove participant "${participantId}"?`)) return;

    try {
      await invoke("remove_participant", { roomId, participantId });
      loadData();
    } catch (e) {
      setError(String(e));
    }
  };

  const formatUptime = (secs: number): string => {
    const hours = Math.floor(secs / 3600);
    const minutes = Math.floor((secs % 3600) / 60);
    const seconds = secs % 60;
    return `${hours}h ${minutes}m ${seconds}s`;
  };

  const formatBytes = (bytes: number): string => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
    return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  };

  const getRoleBadgeClass = (role: string): string => {
    switch (role.toLowerCase()) {
      case "primary_sender":
        return "badge-success";
      case "backup_sender":
        return "badge-warning";
      case "receiver":
        return "badge-info";
      default:
        return "badge-gray";
    }
  };

  const getStateBadgeClass = (state: string): string => {
    switch (state.toLowerCase()) {
      case "active":
        return "badge-success";
      case "connected":
        return "badge-success";
      case "waiting":
        return "badge-warning";
      default:
        return "badge-gray";
    }
  };

  if (loading) {
    return <div className="loading">Loading room details...</div>;
  }

  if (error) {
    return (
      <div className="room-detail">
        <div className="error-message">{error}</div>
        <button className="btn btn-secondary" onClick={onBack}>
          Back to Rooms
        </button>
      </div>
    );
  }

  return (
    <div className="room-detail">
      <div className="room-detail-header">
        <button className="btn btn-secondary" onClick={onBack}>
          Back
        </button>
        <h2>Room: {roomId}</h2>
        {stats && (
          <span className={`badge ${getStateBadgeClass(stats.state)}`}>
            {stats.state}
          </span>
        )}
      </div>

      {stats && (
        <div className="card">
          <div className="card-header">
            <h2>Statistics</h2>
          </div>
          <div className="room-stats-grid">
            <div className="room-stat-item">
              <div className="value">{stats.participant_count}</div>
              <div className="label">Participants</div>
            </div>
            <div className="room-stat-item">
              <div className="value">{stats.receiver_count}</div>
              <div className="label">Receivers</div>
            </div>
            <div className="room-stat-item">
              <div className="value" style={{ color: stats.has_primary_sender ? "var(--success)" : "var(--text-secondary)" }}>
                {stats.has_primary_sender ? "Yes" : "No"}
              </div>
              <div className="label">Primary Sender</div>
            </div>
            <div className="room-stat-item">
              <div className="value" style={{ color: stats.has_backup_sender ? "var(--warning)" : "var(--text-secondary)" }}>
                {stats.has_backup_sender ? "Yes" : "No"}
              </div>
              <div className="label">Backup Sender</div>
            </div>
            <div className="room-stat-item">
              <div className="value">{stats.packets_forwarded.toLocaleString()}</div>
              <div className="label">Packets Forwarded</div>
            </div>
            <div className="room-stat-item">
              <div className="value">{formatBytes(stats.bytes_forwarded)}</div>
              <div className="label">Data Forwarded</div>
            </div>
            <div className="room-stat-item">
              <div className="value">{formatUptime(stats.uptime_secs)}</div>
              <div className="label">Uptime</div>
            </div>
          </div>
        </div>
      )}

      <div className="card">
        <div className="card-header">
          <h2>Participants ({participants.length})</h2>
        </div>
        {participants.length === 0 ? (
          <div className="empty-state">
            <h3>No Participants</h3>
            <p>Waiting for participants to join this room.</p>
          </div>
        ) : (
          <div className="table-container">
            <table>
              <thead>
                <tr>
                  <th>Participant ID</th>
                  <th>Role</th>
                  <th>State</th>
                  <th>Address</th>
                  <th>Session ID</th>
                  <th>Actions</th>
                </tr>
              </thead>
              <tbody>
                {participants.map((p) => (
                  <tr key={p.id}>
                    <td style={{ fontFamily: "monospace", fontSize: "13px" }}>{p.id}</td>
                    <td>
                      <span className={`badge ${getRoleBadgeClass(p.role)}`}>
                        {p.role}
                      </span>
                    </td>
                    <td>
                      <span className={`badge ${getStateBadgeClass(p.state)}`}>
                        {p.state}
                      </span>
                    </td>
                    <td style={{ fontFamily: "monospace", fontSize: "13px" }}>{p.addr}</td>
                    <td>{p.session_id}</td>
                    <td>
                      <button
                        className="btn btn-danger btn-small"
                        onClick={() => handleRemoveParticipant(p.id)}
                      >
                        Remove
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
    </div>
  );
}

export default RoomDetail;

import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { RoomResponse } from "../types";

interface RoomsProps {
  onSelectRoom: (roomId: string) => void;
}

function Rooms({ onSelectRoom }: RoomsProps) {
  const [rooms, setRooms] = useState<RoomResponse[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [newRoomId, setNewRoomId] = useState("");

  const loadRooms = async () => {
    try {
      setError(null);
      const data = await invoke<RoomResponse[]>("get_rooms");
      setRooms(data);
    } catch (e) {
      setError(String(e));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    loadRooms();
    const interval = setInterval(loadRooms, 5000);
    return () => clearInterval(interval);
  }, []);

  const handleCreateRoom = async () => {
    try {
      setError(null);
      const roomId = newRoomId.trim() || undefined;
      await invoke("create_room", { roomId });
      setNewRoomId("");
      setCreating(false);
      loadRooms();
    } catch (e) {
      setError(String(e));
    }
  };

  const handleDeleteRoom = async (roomId: string, e: React.MouseEvent) => {
    e.stopPropagation();
    if (!confirm(`Delete room "${roomId}"?`)) return;

    try {
      await invoke("delete_room", { roomId });
      loadRooms();
    } catch (e) {
      setError(String(e));
    }
  };

  const getStateBadgeClass = (state: string): string => {
    switch (state.toLowerCase()) {
      case "active":
        return "badge-success";
      case "waiting":
        return "badge-warning";
      case "empty":
        return "badge-gray";
      default:
        return "badge-info";
    }
  };

  if (loading) {
    return <div className="loading">Loading rooms...</div>;
  }

  return (
    <div className="card">
      <div className="card-header">
        <h2>Rooms ({rooms.length})</h2>
        <button className="btn btn-primary" onClick={() => setCreating(true)}>
          Create Room
        </button>
      </div>

      {error && <div className="error-message">{error}</div>}

      {creating && (
        <div style={{ marginBottom: "16px", padding: "16px", background: "var(--bg-card)", borderRadius: "6px" }}>
          <div className="form-group">
            <label>Room ID (optional - leave empty for auto-generated)</label>
            <input
              type="text"
              value={newRoomId}
              onChange={(e) => setNewRoomId(e.target.value)}
              placeholder="my-room-id"
            />
          </div>
          <div style={{ display: "flex", gap: "8px" }}>
            <button className="btn btn-primary" onClick={handleCreateRoom}>
              Create
            </button>
            <button className="btn btn-secondary" onClick={() => setCreating(false)}>
              Cancel
            </button>
          </div>
        </div>
      )}

      {rooms.length === 0 ? (
        <div className="empty-state">
          <h3>No Rooms</h3>
          <p>Create a room to get started.</p>
        </div>
      ) : (
        <div className="table-container">
          <table>
            <thead>
              <tr>
                <th>Room ID</th>
                <th>State</th>
                <th>Participants</th>
                <th>Server</th>
                <th>Actions</th>
              </tr>
            </thead>
            <tbody>
              {rooms.map((room) => (
                <tr
                  key={room.id}
                  className="table-row-clickable"
                  onClick={() => onSelectRoom(room.id)}
                >
                  <td style={{ fontWeight: 500 }}>{room.id}</td>
                  <td>
                    <span className={`badge ${getStateBadgeClass(room.state)}`}>
                      {room.state}
                    </span>
                  </td>
                  <td>{room.participant_count}</td>
                  <td style={{ fontFamily: "monospace", fontSize: "13px" }}>
                    {room.server_id}
                  </td>
                  <td>
                    <button
                      className="btn btn-danger btn-small"
                      onClick={(e) => handleDeleteRoom(room.id, e)}
                    >
                      Delete
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}

export default Rooms;

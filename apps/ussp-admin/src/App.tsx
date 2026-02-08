import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import Dashboard from "./components/Dashboard";
import Rooms from "./components/Rooms";
import RoomDetail from "./components/RoomDetail";
import Servers from "./components/Servers";
import Settings from "./components/Settings";
import { Tab, ServerConfig } from "./types";

function App() {
  const [tab, setTab] = useState<Tab>("dashboard");
  const [connected, setConnected] = useState(false);
  const [selectedRoom, setSelectedRoom] = useState<string | null>(null);
  const [serverUrl, setServerUrl] = useState("http://localhost:8080");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    // Load saved config on startup
    invoke<ServerConfig>("get_config").then((config) => {
      setServerUrl(config.server_url);
      setConnected(config.token !== null);
    });
  }, []);

  const handleConnect = async (url: string, apiKey: string) => {
    try {
      setError(null);
      await invoke("connect", { serverUrl: url, apiKey });
      setServerUrl(url);
      setConnected(true);
    } catch (e) {
      setError(String(e));
      setConnected(false);
    }
  };

  const handleDisconnect = async () => {
    try {
      await invoke("disconnect");
      setConnected(false);
      setSelectedRoom(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const handleSelectRoom = (roomId: string) => {
    setSelectedRoom(roomId);
  };

  const handleBackToRooms = () => {
    setSelectedRoom(null);
  };

  const renderContent = () => {
    if (!connected && tab !== "settings") {
      return (
        <div className="not-connected">
          <div className="not-connected-content">
            <h2>Not Connected</h2>
            <p>Connect to an SFU Control Plane to manage servers and rooms.</p>
            <button className="btn btn-primary" onClick={() => setTab("settings")}>
              Go to Settings
            </button>
          </div>
        </div>
      );
    }

    switch (tab) {
      case "dashboard":
        return <Dashboard />;
      case "rooms":
        if (selectedRoom) {
          return <RoomDetail roomId={selectedRoom} onBack={handleBackToRooms} />;
        }
        return <Rooms onSelectRoom={handleSelectRoom} />;
      case "servers":
        return <Servers />;
      case "settings":
        return (
          <Settings
            serverUrl={serverUrl}
            connected={connected}
            onConnect={handleConnect}
            onDisconnect={handleDisconnect}
            error={error}
          />
        );
    }
  };

  return (
    <div className="app">
      <header className="header">
        <h1>USSP Admin</h1>
        <nav className="nav-tabs">
          <button
            className={tab === "dashboard" ? "active" : ""}
            onClick={() => setTab("dashboard")}
          >
            Dashboard
          </button>
          <button
            className={tab === "rooms" ? "active" : ""}
            onClick={() => {
              setTab("rooms");
              setSelectedRoom(null);
            }}
          >
            Rooms
          </button>
          <button
            className={tab === "servers" ? "active" : ""}
            onClick={() => setTab("servers")}
          >
            Servers
          </button>
          <button
            className={tab === "settings" ? "active" : ""}
            onClick={() => setTab("settings")}
          >
            Settings
          </button>
        </nav>
        <div className="connection-status">
          <span className={`dot ${connected ? "connected" : "disconnected"}`}></span>
          <span>{connected ? "Connected" : "Disconnected"}</span>
        </div>
      </header>

      <main className="main-content">{renderContent()}</main>
    </div>
  );
}

export default App;

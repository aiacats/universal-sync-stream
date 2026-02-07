import { useState, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { SenderStats, SenderOptions } from "../types";
import Stats from "./Stats";

function Sender() {
  const [isRunning, setIsRunning] = useState(false);
  const [destAddress, setDestAddress] = useState("127.0.0.1");
  const [port, setPort] = useState(5001);
  const [audioTracks, setAudioTracks] = useState(2);
  const [fecEnabled, setFecEnabled] = useState(true);
  const [fps, setFps] = useState(30);
  const [stats, setStats] = useState<SenderStats | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  const addLog = useCallback((message: string, _type: string = "info") => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev.slice(-50), `[${timestamp}] ${message}`]);
  }, []);

  useEffect(() => {
    const unlistenStats = listen<SenderStats>("ussp://sender-stats", (event) => {
      setStats(event.payload);
    });

    return () => {
      unlistenStats.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    invoke<boolean>("is_sender_running").then(setIsRunning);
  }, []);

  const handleStart = async () => {
    try {
      const options: SenderOptions = {
        audio_tracks: audioTracks,
        fec_enabled: fecEnabled,
        fps: fps,
        width: 640,
        height: 480,
      };

      await invoke("start_sender", {
        dest: destAddress,
        port: port,
        options: options,
      });

      setIsRunning(true);
      addLog(`Started sending to ${destAddress}:${port}`, "success");
    } catch (error) {
      addLog(`Failed to start: ${error}`, "error");
    }
  };

  const handleStop = async () => {
    try {
      await invoke("stop_sender");
      setIsRunning(false);
      addLog("Stopped sending", "info");
    } catch (error) {
      addLog(`Failed to stop: ${error}`, "error");
    }
  };

  return (
    <>
      <div className="panel control-panel">
        <h2>Sender Settings</h2>

        <div className="status-indicator">
          <div className={`dot ${isRunning ? "running" : "stopped"}`}></div>
          <span>{isRunning ? "Sending" : "Stopped"}</span>
        </div>

        <div className="form-group">
          <label>Destination Address</label>
          <input
            type="text"
            value={destAddress}
            onChange={(e) => setDestAddress(e.target.value)}
            disabled={isRunning}
          />
        </div>

        <div className="form-group">
          <label>Port</label>
          <input
            type="number"
            value={port}
            onChange={(e) => setPort(parseInt(e.target.value) || 5001)}
            disabled={isRunning}
          />
        </div>

        <div className="form-row">
          <div className="form-group">
            <label>Audio Tracks</label>
            <select
              value={audioTracks}
              onChange={(e) => setAudioTracks(parseInt(e.target.value))}
              disabled={isRunning}
            >
              {[1, 2, 4, 8].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label>FPS</label>
            <select
              value={fps}
              onChange={(e) => setFps(parseInt(e.target.value))}
              disabled={isRunning}
            >
              {[24, 25, 30, 50, 60].map((n) => (
                <option key={n} value={n}>
                  {n}
                </option>
              ))}
            </select>
          </div>
        </div>

        <div className="checkbox-group">
          <input
            type="checkbox"
            id="fec"
            checked={fecEnabled}
            onChange={(e) => setFecEnabled(e.target.checked)}
            disabled={isRunning}
          />
          <label htmlFor="fec">Enable FEC (Forward Error Correction)</label>
        </div>

        {isRunning ? (
          <button className="btn btn-stop" onClick={handleStop}>
            Stop Sending
          </button>
        ) : (
          <button className="btn btn-primary" onClick={handleStart}>
            Start Sending
          </button>
        )}

        <h2>Logs</h2>
        <div className="log-panel">
          {logs.map((log, i) => (
            <div key={i} className="log-entry info">
              {log}
            </div>
          ))}
        </div>
      </div>

      <div className="panel preview-panel">
        <h2>Sender Statistics</h2>

        {stats ? (
          <Stats
            items={[
              { label: "Packets Sent", value: stats.packets_sent.toLocaleString() },
              { label: "Bytes Sent", value: formatBytes(stats.bytes_sent) },
              { label: "Video Frames", value: stats.video_frames_sent.toLocaleString() },
              { label: "Audio Frames", value: stats.audio_frames_sent.toLocaleString() },
              { label: "FPS", value: stats.fps.toFixed(1), fullWidth: true },
            ]}
          />
        ) : (
          <div className="stats-grid">
            <div className="stat-item">
              <div className="label">Status</div>
              <div className="value">Not started</div>
            </div>
          </div>
        )}

        <h2>Test Pattern Preview</h2>
        <div className="video-preview">
          <TestPatternPreview isRunning={isRunning} />
        </div>
      </div>
    </>
  );
}

function TestPatternPreview({ isRunning }: { isRunning: boolean }) {
  const [frame, setFrame] = useState(0);

  useEffect(() => {
    if (!isRunning) return;

    const interval = setInterval(() => {
      setFrame((f) => f + 1);
    }, 33);

    return () => clearInterval(interval);
  }, [isRunning]);

  if (!isRunning) {
    return <span className="placeholder">Start sending to see preview</span>;
  }

  return (
    <canvas
      ref={(canvas) => {
        if (!canvas) return;
        const ctx = canvas.getContext("2d");
        if (!ctx) return;

        canvas.width = 320;
        canvas.height = 240;

        // Draw color bars
        const colors = [
          "#fff",
          "#ff0",
          "#0ff",
          "#0f0",
          "#f0f",
          "#f00",
          "#00f",
          "#000",
        ];
        const barWidth = canvas.width / 8;

        colors.forEach((color, i) => {
          ctx.fillStyle = color;
          ctx.fillRect(i * barWidth, 0, barWidth, canvas.height);
        });

        // Animated line
        const lineY = (frame % canvas.height);
        ctx.fillStyle = "#fff";
        ctx.fillRect(0, lineY, canvas.width, 2);

        // Timecode
        ctx.fillStyle = "#fff";
        ctx.font = "14px monospace";
        ctx.fillText(`Frame: ${frame}`, 10, 20);
      }}
    />
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export default Sender;

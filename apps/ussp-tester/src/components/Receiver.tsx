import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { ReceiverStats, VideoFrameData, AudioLevelData, SyncPointData } from "../types";
import Stats from "./Stats";
import AudioMeter from "./AudioMeter";

function Receiver() {
  const [isRunning, setIsRunning] = useState(false);
  const [port, setPort] = useState(5001);
  const [stats, setStats] = useState<ReceiverStats | null>(null);
  const [audioLevels, setAudioLevels] = useState<Map<number, number>>(new Map());
  const [syncDiff, setSyncDiff] = useState<number>(0);
  const [logs, setLogs] = useState<string[]>([]);
  const canvasRef = useRef<HTMLCanvasElement>(null);

  const addLog = useCallback((message: string) => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev.slice(-50), `[${timestamp}] ${message}`]);
  }, []);

  useEffect(() => {
    const unlistenStats = listen<ReceiverStats>("ussp://receiver-stats", (event) => {
      setStats(event.payload);
    });

    const unlistenVideo = listen<VideoFrameData>("ussp://video-frame", (event) => {
      drawFrame(event.payload);
    });

    const unlistenAudio = listen<AudioLevelData>("ussp://audio-level", (event) => {
      setAudioLevels((prev) => {
        const next = new Map(prev);
        next.set(event.payload.track_id, event.payload.level_db);
        return next;
      });
    });

    const unlistenSync = listen<SyncPointData>("ussp://sync-point", (event) => {
      setSyncDiff(event.payload.av_diff_us);
    });

    const unlistenSession = listen("ussp://session-init", (event) => {
      addLog(`Session initialized: ${JSON.stringify(event.payload)}`);
    });

    const unlistenDisconnect = listen("ussp://disconnected", () => {
      addLog("Disconnected from sender");
      setIsRunning(false);
    });

    return () => {
      unlistenStats.then((fn) => fn());
      unlistenVideo.then((fn) => fn());
      unlistenAudio.then((fn) => fn());
      unlistenSync.then((fn) => fn());
      unlistenSession.then((fn) => fn());
      unlistenDisconnect.then((fn) => fn());
    };
  }, [addLog]);

  useEffect(() => {
    invoke<boolean>("is_receiver_running").then(setIsRunning);
  }, []);

  const drawFrame = (frame: VideoFrameData) => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    // Decode base64 and draw
    try {
      const binaryString = atob(frame.data_base64);
      const bytes = new Uint8Array(binaryString.length);
      for (let i = 0; i < binaryString.length; i++) {
        bytes[i] = binaryString.charCodeAt(i);
      }

      // Assume RGB data
      canvas.width = frame.width;
      canvas.height = frame.height;

      const imageData = ctx.createImageData(frame.width, frame.height);
      for (let i = 0; i < frame.width * frame.height; i++) {
        imageData.data[i * 4] = bytes[i * 3] || 0;     // R
        imageData.data[i * 4 + 1] = bytes[i * 3 + 1] || 0; // G
        imageData.data[i * 4 + 2] = bytes[i * 3 + 2] || 0; // B
        imageData.data[i * 4 + 3] = 255;               // A
      }

      ctx.putImageData(imageData, 0, 0);
    } catch {
      // Drawing placeholder on error
    }
  };

  const handleStart = async () => {
    try {
      await invoke("start_receiver", { port });
      setIsRunning(true);
      addLog(`Started receiving on port ${port}`);
    } catch (error) {
      addLog(`Failed to start: ${error}`);
    }
  };

  const handleStop = async () => {
    try {
      await invoke("stop_receiver");
      setIsRunning(false);
      addLog("Stopped receiving");
    } catch (error) {
      addLog(`Failed to stop: ${error}`);
    }
  };

  const getSyncClass = (diffUs: number): string => {
    const absMs = Math.abs(diffUs) / 1000;
    if (absMs < 20) return "good";
    if (absMs < 50) return "warning";
    return "bad";
  };

  return (
    <>
      <div className="panel control-panel">
        <h2>Receiver Settings</h2>

        <div className="status-indicator">
          <div className={`dot ${isRunning ? "running" : "stopped"}`}></div>
          <span>{isRunning ? "Receiving" : "Stopped"}</span>
        </div>

        <div className="form-group">
          <label>Listen Port</label>
          <input
            type="number"
            value={port}
            onChange={(e) => setPort(parseInt(e.target.value) || 5001)}
            disabled={isRunning}
          />
        </div>

        {isRunning ? (
          <button className="btn btn-stop" onClick={handleStop}>
            Stop Receiving
          </button>
        ) : (
          <button className="btn btn-primary" onClick={handleStart}>
            Start Receiving
          </button>
        )}

        <h2>A/V Sync</h2>
        <div className="sync-status">
          <div className={`sync-value ${getSyncClass(syncDiff)}`}>
            {(syncDiff / 1000).toFixed(1)} ms
          </div>
          <div className="sync-label">Video - Audio difference</div>
        </div>

        <h2>Audio Levels</h2>
        <div className="audio-meters">
          {Array.from(audioLevels.entries())
            .sort((a, b) => a[0] - b[0])
            .map(([trackId, level]) => (
              <AudioMeter key={trackId} trackId={trackId} level={level} />
            ))}
          {audioLevels.size === 0 && (
            <div style={{ color: "var(--text-secondary)", fontSize: 12 }}>
              No audio received yet
            </div>
          )}
        </div>

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
        <h2>Receiver Statistics</h2>

        {stats ? (
          <Stats
            items={[
              { label: "Packets Received", value: stats.packets_received.toLocaleString() },
              { label: "Bytes Received", value: formatBytes(stats.bytes_received) },
              { label: "Video Frames", value: stats.video_frames_received.toLocaleString() },
              { label: "Audio Frames", value: stats.audio_frames_received.toLocaleString() },
              { label: "FPS", value: stats.fps.toFixed(1) },
              { label: "Packet Loss", value: `${(stats.packet_loss_rate * 100).toFixed(1)}%` },
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

        <h2>Video Preview</h2>
        <div className="video-preview">
          {isRunning ? (
            <canvas ref={canvasRef} width={640} height={480} />
          ) : (
            <span className="placeholder">Start receiving to see video</span>
          )}
        </div>
      </div>
    </>
  );
}

function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
  return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
}

export default Receiver;

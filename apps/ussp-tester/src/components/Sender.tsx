import { useState, useEffect, useCallback, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type {
  SenderStats,
  SenderOptions,
  VideoDeviceInfo,
  AudioDeviceInfo,
  VideoSource,
  AudioSource,
  Resolution,
  CameraPreviewData,
  AudioLevelData,
} from "../types";
import Stats from "./Stats";
import AudioWaveform from "./AudioWaveform";

function Sender() {
  const [isRunning, setIsRunning] = useState(false);
  const [destAddress, setDestAddress] = useState("127.0.0.1");
  const [port, setPort] = useState(5001);
  const [audioTracks, setAudioTracks] = useState(2);
  const [fecEnabled, setFecEnabled] = useState(true);
  const [fps, setFps] = useState(30);
  const [resolution, setResolution] = useState<Resolution>("1280x720");
  const [stats, setStats] = useState<SenderStats | null>(null);
  const [logs, setLogs] = useState<string[]>([]);

  // Device lists
  const [videoDevices, setVideoDevices] = useState<VideoDeviceInfo[]>([]);
  const [audioDevices, setAudioDevices] = useState<AudioDeviceInfo[]>([]);

  // Source selection
  const [videoSource, setVideoSource] = useState<VideoSource>({ type: "test-pattern" });
  const [audioSource, setAudioSource] = useState<AudioSource>({ type: "test-tone" });

  // Camera preview
  const previewCanvasRef = useRef<HTMLCanvasElement>(null);

  // Audio waveform data
  const [audioData, setAudioData] = useState<Map<number, { level: number; samples: number[] }>>(new Map());

  const addLog = useCallback((message: string, _type: string = "info") => {
    const timestamp = new Date().toLocaleTimeString();
    setLogs((prev) => [...prev.slice(-50), `[${timestamp}] ${message}`]);
  }, []);

  // Load devices on mount
  useEffect(() => {
    const loadDevices = async () => {
      try {
        // Initialize capture backend
        await invoke("initialize_capture");

        const videos = await invoke<VideoDeviceInfo[]>("list_video_devices");
        setVideoDevices(videos);
        addLog(`Found ${videos.length} video device(s)`, "info");

        const audios = await invoke<AudioDeviceInfo[]>("list_audio_devices");
        setAudioDevices(audios);
        addLog(`Found ${audios.length} audio device(s)`, "info");
      } catch (error) {
        addLog(`Failed to load devices: ${error}`, "error");
      }
    };

    loadDevices();
  }, [addLog]);

  // Listen to events
  useEffect(() => {
    const unlistenStats = listen<SenderStats>("ussp://sender-stats", (event) => {
      setStats(event.payload);
    });

    const unlistenError = listen<string>("ussp://error", (event) => {
      addLog(event.payload, "error");
    });

    const unlistenPreview = listen<CameraPreviewData>("ussp://camera-preview", (event) => {
      const canvas = previewCanvasRef.current;
      if (!canvas) return;

      const ctx = canvas.getContext("2d");
      if (!ctx) return;

      const { width, height, data } = event.payload;
      canvas.width = width;
      canvas.height = height;

      // Decode base64 and draw
      const binaryStr = atob(data);
      const bytes = new Uint8Array(binaryStr.length);
      for (let i = 0; i < binaryStr.length; i++) {
        bytes[i] = binaryStr.charCodeAt(i);
      }

      const imageData = ctx.createImageData(width, height);
      for (let i = 0; i < width * height; i++) {
        imageData.data[i * 4] = bytes[i * 3];     // R
        imageData.data[i * 4 + 1] = bytes[i * 3 + 1]; // G
        imageData.data[i * 4 + 2] = bytes[i * 3 + 2]; // B
        imageData.data[i * 4 + 3] = 255;          // A
      }
      ctx.putImageData(imageData, 0, 0);
    });

    const unlistenAudioLevel = listen<AudioLevelData>("ussp://sender-audio-level", (event) => {
      setAudioData((prev) => {
        const next = new Map(prev);
        next.set(event.payload.track_id, {
          level: event.payload.level_db,
          samples: event.payload.samples,
        });
        return next;
      });
    });

    return () => {
      unlistenStats.then((fn) => fn());
      unlistenError.then((fn) => fn());
      unlistenPreview.then((fn) => fn());
      unlistenAudioLevel.then((fn) => fn());
    };
  }, [addLog]);

  useEffect(() => {
    invoke<boolean>("is_sender_running").then(setIsRunning);
  }, []);

  const handleStart = async () => {
    try {
      // Clear previous audio data
      setAudioData(new Map());

      const options: SenderOptions = {
        audio_tracks: audioTracks,
        fec_enabled: fecEnabled,
        fps: fps,
        resolution: resolution,
        video_source: videoSource,
        audio_source: audioSource,
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
      setAudioData(new Map()); // Clear audio data
      addLog("Stopped sending", "info");
    } catch (error) {
      addLog(`Failed to stop: ${error}`, "error");
    }
  };

  const handleVideoSourceChange = (value: string) => {
    if (value === "test-pattern") {
      setVideoSource({ type: "test-pattern" });
    } else {
      // value format: "camera:DeviceName"
      const deviceName = value.replace("camera:", "");
      setVideoSource({ type: "camera", device_name: deviceName });
    }
  };

  const handleAudioSourceChange = (value: string) => {
    if (value === "test-tone") {
      setAudioSource({ type: "test-tone" });
    } else {
      const index = parseInt(value.replace("mic:", ""));
      setAudioSource({ type: "microphone", device_index: index });
    }
  };

  const getVideoSourceValue = (): string => {
    if (videoSource.type === "test-pattern") return "test-pattern";
    return `camera:${videoSource.device_name}`;
  };

  const getAudioSourceValue = (): string => {
    if (audioSource.type === "test-tone") return "test-tone";
    return `mic:${audioSource.device_index}`;
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

        <h2>Input Sources</h2>

        <div className="form-group">
          <label>Video Source</label>
          <select
            value={getVideoSourceValue()}
            onChange={(e) => handleVideoSourceChange(e.target.value)}
            disabled={isRunning}
          >
            <option value="test-pattern">Test Pattern (Color Bars)</option>
            {videoDevices.map((d) => (
              <option key={d.name} value={`camera:${d.name}`}>
                {d.name}
              </option>
            ))}
          </select>
        </div>

        <div className="form-group">
          <label>Audio Source</label>
          <select
            value={getAudioSourceValue()}
            onChange={(e) => handleAudioSourceChange(e.target.value)}
            disabled={isRunning}
          >
            <option value="test-tone">Test Tone (440Hz Sine)</option>
            {audioDevices.map((d) => (
              <option key={d.index} value={`mic:${d.index}`}>
                {d.name}
              </option>
            ))}
          </select>
        </div>

        <h2>Options</h2>

        <div className="form-group">
          <label>Resolution</label>
          <select
            value={resolution}
            onChange={(e) => setResolution(e.target.value as Resolution)}
            disabled={isRunning}
          >
            <option value="640x480">640x480 (VGA)</option>
            <option value="1280x720">1280x720 (720p)</option>
            <option value="1920x1080">1920x1080 (1080p)</option>
          </select>
        </div>

        <div className="form-row">
          <div className="form-group">
            <label>Audio Tracks</label>
            <select
              value={audioTracks}
              onChange={(e) => setAudioTracks(parseInt(e.target.value))}
              disabled={isRunning || audioSource.type === "microphone"}
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

        <h2>Audio Waveforms</h2>
        <div className="audio-waveforms">
          {Array.from(audioData.entries())
            .sort((a, b) => a[0] - b[0])
            .map(([trackId, data]) => (
              <AudioWaveform
                key={trackId}
                trackId={trackId}
                level={data.level}
                samples={data.samples}
              />
            ))}
          {audioData.size === 0 && (
            <div style={{ color: "var(--text-secondary)", fontSize: 12 }}>
              {isRunning ? "Waiting for audio..." : "Start sending to see waveforms"}
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

        <h2>Preview</h2>
        <div className="video-preview">
          {videoSource.type === "camera" ? (
            <canvas ref={previewCanvasRef} />
          ) : (
            <TestPatternPreview isRunning={isRunning} />
          )}
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
        const lineY = frame % canvas.height;
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

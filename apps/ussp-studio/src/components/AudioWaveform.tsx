import { useRef, useEffect } from "react";

interface AudioWaveformProps {
  trackId: number;
  level: number; // in dB
  samples: number[]; // normalized -1 to 1
}

function AudioWaveform({ trackId, level, samples }: AudioWaveformProps) {
  const canvasRef = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;

    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const width = canvas.width;
    const height = canvas.height;
    const midY = height / 2;

    // Clear canvas
    ctx.fillStyle = "#1a1a2e";
    ctx.fillRect(0, 0, width, height);

    // Draw center line
    ctx.strokeStyle = "#333";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(0, midY);
    ctx.lineTo(width, midY);
    ctx.stroke();

    // Draw waveform
    if (samples.length === 0) return;

    const stepX = width / samples.length;

    // Gradient based on level
    const intensity = Math.min(1, Math.max(0, (level + 60) / 60));
    const hue = 200 - intensity * 80; // Blue to green
    ctx.strokeStyle = `hsl(${hue}, 80%, ${50 + intensity * 20}%)`;
    ctx.lineWidth = 2;

    ctx.beginPath();
    samples.forEach((sample, i) => {
      const x = i * stepX;
      const y = midY - sample * (height / 2) * 0.9;
      if (i === 0) {
        ctx.moveTo(x, y);
      } else {
        ctx.lineTo(x, y);
      }
    });
    ctx.stroke();

    // Fill under the waveform with gradient
    ctx.globalAlpha = 0.3;
    ctx.fillStyle = `hsl(${hue}, 80%, 50%)`;
    ctx.lineTo((samples.length - 1) * stepX, midY);
    ctx.lineTo(0, midY);
    ctx.closePath();
    ctx.fill();
    ctx.globalAlpha = 1;

  }, [samples, level]);

  // Convert dB to percentage for level bar
  const percentage = Math.max(0, Math.min(100, ((level + 60) / 60) * 100));

  return (
    <div className="audio-waveform">
      <div className="waveform-header">
        <span className="track-label">Track {trackId + 1}</span>
        <span className="level-value">{level.toFixed(1)} dB</span>
      </div>
      <canvas ref={canvasRef} width={280} height={60} />
      <div className="level-bar">
        <div
          className="level-fill"
          style={{ width: `${percentage}%` }}
        />
      </div>
    </div>
  );
}

export default AudioWaveform;

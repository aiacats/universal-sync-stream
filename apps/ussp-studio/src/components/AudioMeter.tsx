interface AudioMeterProps {
  trackId: number;
  level: number; // in dB, typically -60 to 0
}

function AudioMeter({ trackId, level }: AudioMeterProps) {
  // Convert dB to percentage (0 dB = 100%, -60 dB = 0%)
  const percentage = Math.max(0, Math.min(100, ((level + 60) / 60) * 100));

  return (
    <div className="audio-meter">
      <span className="track-label">Track {trackId + 1}</span>
      <div className="meter-bar">
        <div
          className="meter-fill"
          style={{ width: `${percentage}%` }}
        />
      </div>
      <span className="level-value">{level.toFixed(1)} dB</span>
    </div>
  );
}

export default AudioMeter;

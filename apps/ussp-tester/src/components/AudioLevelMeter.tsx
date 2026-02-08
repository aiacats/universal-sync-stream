interface AudioLevelMeterProps {
  levels: Map<number, number>; // track_id -> level_db
}

function AudioLevelMeter({ levels }: AudioLevelMeterProps) {
  const sortedLevels = Array.from(levels.entries()).sort((a, b) => a[0] - b[0]);

  // Colors for track labels
  const trackColors = [
    "#4caf50", "#8bc34a", "#cddc39", "#ffeb3b",
    "#ffc107", "#ff9800", "#ff5722", "#f44336",
    "#e91e63", "#9c27b0", "#673ab7", "#3f51b5",
    "#2196f3", "#03a9f4", "#00bcd4", "#009688",
  ];

  return (
    <div className="audio-level-meter">
      <div className="meter-scale">
        <span>0</span>
        <span>-6</span>
        <span>-12</span>
        <span>-24</span>
        <span>-48</span>
      </div>
      <div className="meter-tracks">
        {sortedLevels.map(([trackId, levelDb]) => (
          <div key={trackId} className="meter-track">
            <div className="meter-label" style={{ backgroundColor: trackColors[trackId % trackColors.length] }}>
              {trackId + 1}
            </div>
            <div className="meter-bar-container">
              <div className="meter-bar-bg">
                <div
                  className="meter-bar-fill"
                  style={{ height: `${dbToPercent(levelDb)}%` }}
                />
                <div
                  className="meter-bar-peak"
                  style={{ bottom: `${dbToPercent(levelDb)}%` }}
                />
              </div>
            </div>
          </div>
        ))}
      </div>
      {sortedLevels.length === 0 && (
        <div className="meter-empty">No audio tracks</div>
      )}
    </div>
  );
}

function dbToPercent(db: number): number {
  // Map -60dB to 0dB to 0% to 100%
  const minDb = -60;
  const maxDb = 0;
  const clamped = Math.max(minDb, Math.min(maxDb, db));
  return ((clamped - minDb) / (maxDb - minDb)) * 100;
}

export default AudioLevelMeter;

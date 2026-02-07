interface StatItem {
  label: string;
  value: string;
  fullWidth?: boolean;
}

interface StatsProps {
  items: StatItem[];
}

function Stats({ items }: StatsProps) {
  return (
    <div className="stats-grid">
      {items.map((item, index) => (
        <div
          key={index}
          className={`stat-item ${item.fullWidth ? "full-width" : ""}`}
        >
          <div className="label">{item.label}</div>
          <div className="value">{item.value}</div>
        </div>
      ))}
    </div>
  );
}

export default Stats;

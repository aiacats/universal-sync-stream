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
        <div key={index} className="stat-item">
          <span className="label">{item.label}:</span>
          <span className="value">{item.value}</span>
        </div>
      ))}
    </div>
  );
}

export default Stats;

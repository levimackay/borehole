import "./ProgressBar.css";

export function ProgressBar({
  done,
  total,
  label,
}: {
  done: number;
  total: number;
  label: string;
}) {
  const pct = total > 0 ? Math.min(100, Math.round((done / total) * 100)) : 0;
  return (
    <div className="bh-progress">
      <div
        className="bh-progress__track"
        role="progressbar"
        aria-valuenow={pct}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-label={label}
      >
        <div className="bh-progress__fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="bh-progress__label bh-mono">{label}</span>
    </div>
  );
}

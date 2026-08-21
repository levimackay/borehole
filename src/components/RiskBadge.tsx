import type { RiskTier } from "../lib/ipc-types";

const LABEL: Record<RiskTier, string> = {
  low: "LOW RISK",
  medium: "MEDIUM RISK",
  high: "HIGH RISK",
  critical: "CRITICAL RISK",
};

export function RiskBadge({ risk }: { risk: RiskTier }) {
  return <span className={`bh-risk bh-risk--${risk}`}>{LABEL[risk]}</span>;
}

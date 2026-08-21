import type { Confidence } from "../lib/ipc-types";

const LABEL: Record<Confidence, string> = {
  high: "HIGH",
  medium: "MED",
  low: "LOW",
};

const TITLE: Record<Confidence, string> = {
  high: "High confidence",
  medium: "Medium confidence",
  low: "Low confidence: evidence is thin or indirect",
};

export function ConfidenceBadge({ confidence }: { confidence: Confidence }) {
  return (
    <span
      className={`bh-confidence bh-confidence--${confidence}`}
      title={TITLE[confidence]}
      aria-label={TITLE[confidence]}
    >
      {LABEL[confidence]}
    </span>
  );
}

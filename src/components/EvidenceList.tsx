import type { EvidenceRef } from "../lib/ipc-types";
import "./EvidenceList.css";

function describeEvidence(ev: EvidenceRef): { label: string; detail: string } {
  switch (ev.kind) {
    case "reference":
      return {
        label: "reference",
        detail: `${ev.file}:${ev.span.start_line}`,
      };
    case "commit":
      return {
        label: "commit",
        detail: `${ev.sha.slice(0, 7)}: ${ev.summary}`,
      };
    case "test_file":
      return {
        label: "test",
        detail: ev.file,
      };
    case "config_file":
      return {
        label: "config",
        detail: ev.key ? `${ev.file} (${ev.key})` : ev.file,
      };
  }
}

export function EvidenceList({ evidence }: { evidence: EvidenceRef[] }) {
  if (evidence.length === 0) {
    return <p className="bh-evidence-list__empty">No evidence attached.</p>;
  }
  return (
    <ul className="bh-evidence-list">
      {evidence.map((ev, i) => {
        const { label, detail } = describeEvidence(ev);
        return (
          <li key={i} className="bh-evidence-list__item">
            <span className="bh-evidence-list__tag">{label}</span>
            <span className="bh-evidence-list__detail bh-mono">{detail}</span>
          </li>
        );
      })}
    </ul>
  );
}

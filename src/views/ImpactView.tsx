import { useMemo, useRef, useState } from "react";
import { useAppState } from "../state/AppStateContext";
import { commands } from "../lib/ipc";
import { useAsync } from "../hooks/useAsync";
import { AsyncSection } from "../components/AsyncSection";
import { ConfidenceBadge } from "../components/ConfidenceBadge";
import { RiskBadge } from "../components/RiskBadge";
import { EvidenceList } from "../components/EvidenceList";
import { EmptyState } from "../components/EmptyState";
import { GraphCanvas } from "../components/graph/GraphCanvas";
import { focusViewPanel } from "../components/layout/Sidebar";
import { formatTestReason } from "../lib/format";
import type { BlastRadius, SymbolProfile, BeforeYouChangeThisReport } from "../lib/ipc-types";
import "./ImpactView.css";

type Tab = "profile" | "blast-radius" | "before-you-change";

const TABS: { id: Tab; label: string }[] = [
  { id: "profile", label: "Profile" },
  { id: "blast-radius", label: "Blast Radius" },
  { id: "before-you-change", label: "Before You Change This" },
];

const tabButtonId = (id: Tab) => `bh-impact-tab-${id}`;
const TAB_PANEL_ID = "bh-impact-tabpanel";

export function ImpactView() {
  const { selectedSymbol, setView } = useAppState();
  const [tab, setTab] = useState<Tab>("profile");
  const tabRefs = useRef(new Map<Tab, HTMLButtonElement>());

  // Roving tabindex per the WAI-ARIA APG tabs pattern — same treatment as
  // Sidebar's view switcher, horizontal here since these tabs read left to
  // right instead of top to bottom.
  function handleTabKeyDown(event: React.KeyboardEvent<HTMLButtonElement>, index: number) {
    let nextIndex: number | null = null;
    switch (event.key) {
      case "ArrowRight":
        nextIndex = (index + 1) % TABS.length;
        break;
      case "ArrowLeft":
        nextIndex = (index - 1 + TABS.length) % TABS.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = TABS.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    const nextTab = TABS[nextIndex].id;
    setTab(nextTab);
    tabRefs.current.get(nextTab)?.focus();
  }

  const profileFactory = useMemo(
    () => (selectedSymbol ? () => commands.getSymbolProfile(selectedSymbol.id) : null),
    [selectedSymbol],
  );
  const blastFactory = useMemo(
    () => (selectedSymbol ? () => commands.getBlastRadius(selectedSymbol.id) : null),
    [selectedSymbol],
  );
  const beforeFactory = useMemo(
    () => (selectedSymbol ? () => commands.getBeforeYouChangeThis(selectedSymbol.id) : null),
    [selectedSymbol],
  );

  // Each tab fetches independently — one failing must never block the others.
  const profile = useAsync(profileFactory, [selectedSymbol?.id]);
  const blastRadius = useAsync(blastFactory, [selectedSymbol?.id]);
  const beforeYouChange = useAsync(beforeFactory, [selectedSymbol?.id]);

  if (!selectedSymbol) {
    return (
      <div className="bh-view bh-impact-view">
        <EmptyState
          title="No symbol selected"
          description="Select a symbol to see its profile, blast radius, and change-safety report."
          action={
            <button
              type="button"
              onClick={() => {
                setView("symbols");
                focusViewPanel();
              }}
            >
              Go to Symbols
            </button>
          }
        />
      </div>
    );
  }

  return (
    <div className="bh-view bh-impact-view">
      <div className="bh-impact-view__header">
        <h2 className="bh-mono">{selectedSymbol.name}</h2>
        <span className="bh-impact-view__qualified bh-mono">
          {selectedSymbol.qualified_name}
        </span>
      </div>

      <div className="bh-impact-view__tabs" role="tablist" aria-label="Impact analysis">
        {TABS.map((t, index) => (
          <button
            key={t.id}
            ref={(el) => {
              if (el) tabRefs.current.set(t.id, el);
              else tabRefs.current.delete(t.id);
            }}
            type="button"
            id={tabButtonId(t.id)}
            role="tab"
            aria-selected={tab === t.id}
            aria-controls={TAB_PANEL_ID}
            tabIndex={tab === t.id ? 0 : -1}
            className={tab === t.id ? "bh-impact-view__tab--active" : ""}
            onClick={() => setTab(t.id)}
            onKeyDown={(e) => handleTabKeyDown(e, index)}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div
        id={TAB_PANEL_ID}
        className="bh-impact-view__panel"
        role="tabpanel"
        aria-labelledby={tabButtonId(tab)}
        tabIndex={0}
      >
        {tab === "profile" && (
          <AsyncSection state={profile} onRetry={profile.retry}>
            {(data) => <ProfileTab profile={data} />}
          </AsyncSection>
        )}
        {tab === "blast-radius" && (
          <AsyncSection state={blastRadius} onRetry={blastRadius.retry}>
            {(data) => <BlastRadiusTab blastRadius={data} />}
          </AsyncSection>
        )}
        {tab === "before-you-change" && (
          <AsyncSection state={beforeYouChange} onRetry={beforeYouChange.retry}>
            {(data) => <BeforeYouChangeTab report={data} />}
          </AsyncSection>
        )}
      </div>
    </div>
  );
}

function ProfileTab({ profile }: { profile: SymbolProfile }) {
  return (
    <div className="bh-impact-tab">
      <dl className="bh-impact-tab__facts">
        <div>
          <dt>Direct callers</dt>
          <dd>{profile.direct_callers}</dd>
        </div>
        <div>
          <dt>Indirect callers</dt>
          <dd>{profile.indirect_callers}</dd>
        </div>
        <div>
          <dt>Modification count</dt>
          <dd>{profile.modification_count}</dd>
        </div>
        <div>
          <dt>Introduced</dt>
          <dd>
            {profile.introduced ? (
              <>
                <span className="bh-mono">{profile.introduced.sha.slice(0, 7)}</span>{" "}
                {profile.introduced.summary}
              </>
            ) : (
              "Unknown"
            )}
          </dd>
        </div>
      </dl>

      <section>
        <h3>Tests ({profile.tests.length})</h3>
        {profile.tests.length > 0 ? (
          <ul className="bh-impact-tab__list">
            {profile.tests.map((t, i) => (
              <li key={i}>
                <span className="bh-mono">{t.file}</span>
                <span className="bh-impact-tab__muted">via {formatTestReason(t.reason)}</span>
                <ConfidenceBadge confidence={t.confidence} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">No related tests found.</p>
        )}
      </section>

      <section>
        <h3>Config references ({profile.config.length})</h3>
        {profile.config.length > 0 ? (
          <ul className="bh-impact-tab__list">
            {profile.config.map((c, i) => (
              <li key={i}>
                <span className="bh-mono">{c.file}</span>
                {c.key && <span className="bh-impact-tab__muted">({c.key})</span>}
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">No config references found.</p>
        )}
      </section>
    </div>
  );
}

function BlastRadiusTab({ blastRadius }: { blastRadius: BlastRadius }) {
  return (
    <div className="bh-impact-tab">
      <div className="bh-impact-tab__risk-row">
        <RiskBadge risk={blastRadius.risk} />
        {blastRadius.is_public_api && (
          <span className="bh-impact-tab__public-api">public API</span>
        )}
      </div>

      <dl className="bh-impact-tab__facts">
        <div>
          <dt>Direct callers</dt>
          <dd>{blastRadius.direct_callers}</dd>
        </div>
        <div>
          <dt>Indirect callers</dt>
          <dd>{blastRadius.indirect_callers}</dd>
        </div>
        <div>
          <dt>Test suites touched</dt>
          <dd>{blastRadius.test_suites}</dd>
        </div>
      </dl>

      <section>
        <h3>Risk reasons ({blastRadius.risk_reasons.length})</h3>
        {blastRadius.risk_reasons.length > 0 ? (
          <ul className="bh-impact-tab__reasons">
            {blastRadius.risk_reasons.map((r, i) => (
              <li key={i}>
                <p>{r.text}</p>
                <EvidenceList evidence={r.evidence} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">No specific risk reasons recorded.</p>
        )}
      </section>

      <section>
        <h3>Config files ({blastRadius.config_files.length})</h3>
        {blastRadius.config_files.length > 0 ? (
          <ul className="bh-impact-tab__list">
            {blastRadius.config_files.map((c, i) => (
              <li key={i}>
                <span className="bh-mono">{c.file}</span>
                {c.key && <span className="bh-impact-tab__muted">({c.key})</span>}
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">None.</p>
        )}
      </section>

      <section>
        <h3>Caller graph</h3>
        <GraphCanvas graph={blastRadius.caller_graph} />
      </section>
    </div>
  );
}

function BeforeYouChangeTab({ report }: { report: BeforeYouChangeThisReport }) {
  return (
    <div className="bh-impact-tab">
      <div className="bh-impact-tab__risk-row">
        <ConfidenceBadge confidence={report.confidence} />
        <RiskBadge risk={report.blast_radius.risk} />
      </div>

      <section>
        <h3>Reading list ({report.reading_list.length})</h3>
        {report.reading_list.length > 0 ? (
          <ul className="bh-impact-tab__reasons">
            {report.reading_list.map((item, i) => (
              <li key={i}>
                <p className="bh-mono">{item.symbol_or_file}</p>
                <p className="bh-impact-tab__why">{item.why}</p>
                <EvidenceList evidence={item.evidence} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">Nothing flagged for reading.</p>
        )}
      </section>

      <section>
        <h3>Historical warnings ({report.historical_warnings.length})</h3>
        {report.historical_warnings.length > 0 ? (
          <ul className="bh-impact-tab__reasons">
            {report.historical_warnings.map((w, i) => (
              <li key={i}>
                <p>{w.text}</p>
                <EvidenceList evidence={w.commits} />
              </li>
            ))}
          </ul>
        ) : (
          <p className="bh-impact-tab__muted">No historical warnings found.</p>
        )}
      </section>
    </div>
  );
}

import { useMemo } from "react";
import { useAppState } from "../state/AppStateContext";
import { commands } from "../lib/ipc";
import { useAsync } from "../hooks/useAsync";
import { AsyncSection } from "../components/AsyncSection";
import { ConfidenceBadge } from "../components/ConfidenceBadge";
import { EmptyState } from "../components/EmptyState";
import { Panel } from "../components/Panel";
import { focusViewPanel } from "../components/layout/Sidebar";
import { formatTestReason } from "../lib/format";
import "./TestsView.css";

export function TestsView() {
  const { selectedSymbol, setView } = useAppState();

  const factory = useMemo(() => {
    if (!selectedSymbol) return null;
    return () => commands.getRelatedTests(selectedSymbol.id);
  }, [selectedSymbol]);

  const tests = useAsync(factory, [selectedSymbol?.id]);

  if (!selectedSymbol) {
    return (
      <div className="bh-view bh-tests-view">
        <EmptyState
          title="No symbol selected"
          description="Select a symbol to find the tests believed to cover it."
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
    <div className="bh-view bh-tests-view">
      <Panel title={`Related tests for ${selectedSymbol.name}`}>
        <AsyncSection state={tests} onRetry={tests.retry}>
          {(refs) =>
            refs.length === 0 ? (
              <p className="bh-tests-view__empty">
                No related tests found for this symbol — that's a real result, not a loading
                state.
              </p>
            ) : (
              <ul className="bh-tests-view__list">
                {refs.map((ref, i) => (
                  <li key={i} className="bh-tests-view__row">
                    <span className="bh-tests-view__file bh-mono">{ref.file}</span>
                    <span className="bh-tests-view__reason">
                      matched by {formatTestReason(ref.reason)}
                    </span>
                    <ConfidenceBadge confidence={ref.confidence} />
                  </li>
                ))}
              </ul>
            )
          }
        </AsyncSection>
      </Panel>
    </div>
  );
}

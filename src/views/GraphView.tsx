import { useMemo } from "react";
import { useAppState } from "../state/AppStateContext";
import { commands } from "../lib/ipc";
import { useAsync } from "../hooks/useAsync";
import { AsyncSection } from "../components/AsyncSection";
import { EmptyState } from "../components/EmptyState";
import { Panel } from "../components/Panel";
import { GraphCanvas } from "../components/graph/GraphCanvas";
import "./GraphView.css";

export function GraphView() {
  const { selectedSymbol, graphMode, setGraphMode, setView } = useAppState();

  const factory = useMemo(() => {
    if (!selectedSymbol) return null;
    return graphMode === "callers"
      ? () => commands.getCallers(selectedSymbol.id)
      : () => commands.getCallees(selectedSymbol.id);
  }, [selectedSymbol, graphMode]);

  const graph = useAsync(factory, [selectedSymbol?.id, graphMode]);

  if (!selectedSymbol) {
    return (
      <div className="bh-view bh-graph-view">
        <EmptyState
          title="No symbol selected"
          description="Select a symbol first to see who calls it or what it calls."
          action={
            <button type="button" onClick={() => setView("symbols")}>
              Go to Symbols
            </button>
          }
        />
      </div>
    );
  }

  return (
    <div className="bh-view bh-graph-view">
      <Panel
        title={`${graphMode === "callers" ? "Callers" : "Callees"} of ${selectedSymbol.name}`}
        actions={
          <div className="bh-graph-view__mode-toggle" role="group" aria-label="Graph direction">
            <button
              type="button"
              className={graphMode === "callers" ? "bh-graph-view__mode-btn--active" : ""}
              onClick={() => setGraphMode("callers")}
              aria-pressed={graphMode === "callers"}
            >
              Callers
            </button>
            <button
              type="button"
              className={graphMode === "callees" ? "bh-graph-view__mode-btn--active" : ""}
              onClick={() => setGraphMode("callees")}
              aria-pressed={graphMode === "callees"}
            >
              Callees
            </button>
          </div>
        }
      >
        <AsyncSection state={graph} onRetry={graph.retry}>
          {(data) => <GraphCanvas graph={data} />}
        </AsyncSection>
      </Panel>
      <p className="bh-graph-view__note">
        Layout is a simple tiered arrangement by depth from the root — not a force-directed
        layout. Fine for shallow graphs, a known gap for wide ones.
      </p>
    </div>
  );
}

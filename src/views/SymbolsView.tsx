import { useState } from "react";
import { useAppState, type ViewId } from "../state/AppStateContext";
import { useSymbolSearch } from "../hooks/useSymbolSearch";
import { AsyncSection } from "../components/AsyncSection";
import { SymbolResultsList } from "../components/SymbolResultsList";
import { Panel } from "../components/Panel";
import { focusViewPanel } from "../components/layout/Sidebar";
import { formatKind, formatLanguage } from "../lib/format";
import "./SymbolsView.css";

const JUMP_TARGETS: { id: ViewId; label: string }[] = [
  { id: "impact", label: "Impact" },
  { id: "graph", label: "Graph" },
  { id: "history", label: "History" },
  { id: "tests", label: "Tests" },
];

export function SymbolsView() {
  const [query, setQuery] = useState("");
  const { selectedSymbol, selectSymbol, setView, repo } = useAppState();
  const search = useSymbolSearch(query);

  return (
    <div className="bh-view bh-symbols-view">
      <Panel title="Symbol search" className="bh-symbols-view__search-panel">
        <input
          type="search"
          className="bh-symbols-view__input"
          placeholder={repo ? "Search symbols by name…" : "Open a repository first"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          disabled={!repo}
          aria-label="Search symbols"
        />
        <div className="bh-symbols-view__results">
          <AsyncSection
            state={search}
            idleMessage={
              repo ? "Type at least one character to search." : "No repository open yet."
            }
          >
            {(symbols) => (
              <SymbolResultsList
                symbols={symbols}
                selectedId={selectedSymbol?.id}
                onSelect={selectSymbol}
              />
            )}
          </AsyncSection>
        </div>
      </Panel>

      <Panel title="Selected symbol" className="bh-symbols-view__detail-panel">
        {/*
          Picking a result in the list to the left updates this panel in
          place without moving focus (so keyboard users can keep arrowing
          through results) — aria-live announces the change to screen
          reader users who would otherwise never hear about it.
        */}
        <div aria-live="polite">
          {selectedSymbol ? (
            <div className="bh-symbols-view__detail">
              <h3 className="bh-symbols-view__detail-name bh-mono">{selectedSymbol.name}</h3>
              <p className="bh-symbols-view__detail-qualified bh-mono">
                {selectedSymbol.qualified_name}
              </p>
              {selectedSymbol.signature && (
                <pre className="bh-symbols-view__signature bh-mono">
                  {selectedSymbol.signature}
                </pre>
              )}
              {selectedSymbol.doc_comment && (
                <p className="bh-symbols-view__doc">{selectedSymbol.doc_comment}</p>
              )}
              <dl className="bh-symbols-view__facts">
                <div>
                  <dt>Kind</dt>
                  <dd>{formatKind(selectedSymbol.kind)}</dd>
                </div>
                <div>
                  <dt>Language</dt>
                  <dd>{formatLanguage(selectedSymbol.language)}</dd>
                </div>
                <div>
                  <dt>Location</dt>
                  <dd className="bh-mono">
                    {selectedSymbol.file}:{selectedSymbol.span.start_line}
                  </dd>
                </div>
                <div>
                  <dt>Exported</dt>
                  <dd>{selectedSymbol.is_exported ? "yes" : "no"}</dd>
                </div>
              </dl>
              <div className="bh-symbols-view__jump">
                {JUMP_TARGETS.map((t) => (
                  <button
                    key={t.id}
                    type="button"
                    onClick={() => {
                      setView(t.id);
                      focusViewPanel();
                    }}
                  >
                    {t.label} →
                  </button>
                ))}
              </div>
            </div>
          ) : (
            <p className="bh-symbols-view__no-selection">
              Select a symbol from the search results to inspect it.
            </p>
          )}
        </div>
      </Panel>
    </div>
  );
}

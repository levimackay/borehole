import { useState } from "react";
import { useAppState } from "../state/AppStateContext";
import { useSymbolSearch } from "../hooks/useSymbolSearch";
import { AsyncSection } from "../components/AsyncSection";
import { SymbolResultsList } from "../components/SymbolResultsList";
import { Panel } from "../components/Panel";
import { focusViewPanel } from "../components/layout/Sidebar";
import type { Symbol } from "../lib/ipc-types";
import "./SearchView.css";

/**
 * The fast path: search, pick a result, land directly in Impact analysis.
 * (Symbols view is for browsing a result's raw metadata in place — this
 * view is for jumping straight into "what does changing this touch".)
 */
export function SearchView() {
  const [query, setQuery] = useState("");
  const { selectSymbol, setView, repo } = useAppState();
  const search = useSymbolSearch(query);

  function jumpToImpact(symbol: Symbol) {
    selectSymbol(symbol);
    setView("impact");
    // Picking a result swaps the entire main view out from under the
    // (now-unmounted) result button — without an explicit focus move,
    // focus would silently fall back to <body> and a keyboard/screen
    // reader user would get no indication anything happened.
    focusViewPanel();
  }

  return (
    <div className="bh-view bh-search-view">
      <Panel title="Search">
        <input
          type="search"
          autoFocus
          className="bh-search-view__input"
          placeholder={repo ? "Search for a symbol to jump to its impact analysis…" : "Open a repository first"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          disabled={!repo}
          aria-label="Search and jump to impact analysis"
        />
        <AsyncSection
          state={search}
          idleMessage={repo ? "Type at least one character to search." : "No repository open yet."}
        >
          {(symbols) => (
            <SymbolResultsList symbols={symbols} onSelect={jumpToImpact} />
          )}
        </AsyncSection>
      </Panel>
    </div>
  );
}

import { useAppState, type ViewId } from "../../state/AppStateContext";
import { Sidebar, VIEW_PANEL_ID } from "./Sidebar";
import { IndexingStatusBar } from "./IndexingStatusBar";
import { CommandPalette } from "../CommandPalette";
import { ExplorerView } from "../../views/ExplorerView";
import { SymbolsView } from "../../views/SymbolsView";
import { GraphView } from "../../views/GraphView";
import { HistoryView } from "../../views/HistoryView";
import { ImpactView } from "../../views/ImpactView";
import { TestsView } from "../../views/TestsView";
import { SearchView } from "../../views/SearchView";
import "./AppShell.css";

export function AppShell() {
  const { view, repo } = useAppState();

  return (
    <div className="bh-app-shell">
      <Sidebar />
      <div className="bh-app-shell__main-column">
        {repo && <IndexingStatusBar />}
        <main
          id={VIEW_PANEL_ID}
          className="bh-app-shell__main"
          role="tabpanel"
          aria-labelledby={`bh-tab-${view}`}
          tabIndex={-1}
        >
          {renderView(view)}
        </main>
      </div>
      <CommandPalette />
    </div>
  );
}

function renderView(view: ViewId) {
  switch (view) {
    case "explorer":
      return <ExplorerView />;
    case "symbols":
      return <SymbolsView />;
    case "graph":
      return <GraphView />;
    case "history":
      return <HistoryView />;
    case "impact":
      return <ImpactView />;
    case "tests":
      return <TestsView />;
    case "search":
      return <SearchView />;
  }
}

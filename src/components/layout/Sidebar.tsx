import { useAppState, type ViewId } from "../../state/AppStateContext";
import "./Sidebar.css";

const NAV_ITEMS: { id: ViewId; label: string }[] = [
  { id: "explorer", label: "Explorer" },
  { id: "symbols", label: "Symbols" },
  { id: "graph", label: "Graph" },
  { id: "history", label: "History" },
  { id: "impact", label: "Impact" },
  { id: "tests", label: "Tests" },
  { id: "search", label: "Search" },
];

export function Sidebar() {
  const { view, setView, repo } = useAppState();

  return (
    <nav className="bh-sidebar" aria-label="Views">
      <div className="bh-sidebar__brand">
        <span className="bh-sidebar__brand-mark">bh</span>
        <span className="bh-sidebar__brand-name">borehole</span>
      </div>
      <ul className="bh-sidebar__list">
        {NAV_ITEMS.map((item) => (
          <li key={item.id}>
            <button
              type="button"
              className={`bh-sidebar__item${view === item.id ? " bh-sidebar__item--active" : ""}`}
              onClick={() => setView(item.id)}
              aria-current={view === item.id ? "page" : undefined}
            >
              {item.label}
            </button>
          </li>
        ))}
      </ul>
      <div className="bh-sidebar__footer">
        {repo ? (
          <span className="bh-sidebar__repo bh-mono" title={repo.root}>
            {repo.root.split("/").pop() || repo.root}
          </span>
        ) : (
          <span className="bh-sidebar__repo bh-sidebar__repo--empty">no repository open</span>
        )}
      </div>
    </nav>
  );
}

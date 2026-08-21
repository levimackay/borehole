import { useRef } from "react";
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

export const VIEW_PANEL_ID = "bh-view-panel";
const tabId = (id: ViewId) => `bh-tab-${id}`;

/**
 * Call after any `setView()` that represents a real jump away from the
 * current context (command palette actions, "select a symbol → land in
 * Impact", "no symbol selected → go pick one", etc.) — as opposed to
 * Sidebar's own roving-tabindex arrow-key stepping, which deliberately
 * keeps focus on the tab strip. Without this, the element that triggered
 * the navigation is unmounted along with the old view and focus silently
 * falls back to <body>, leaving keyboard and screen-reader users with no
 * indication anything happened.
 */
export function focusViewPanel() {
  requestAnimationFrame(() => {
    document.getElementById(VIEW_PANEL_ID)?.focus();
  });
}

export function Sidebar() {
  const { view, setView, repo } = useAppState();
  const itemRefs = useRef(new Map<ViewId, HTMLButtonElement>());

  function activate(id: ViewId, { moveFocus }: { moveFocus: boolean }) {
    setView(id);
    if (moveFocus) itemRefs.current.get(id)?.focus();
  }

  // Roving tabindex per the WAI-ARIA APG tabs pattern: arrow keys move
  // *and* select (automatic activation), Home/End jump to the ends, and
  // only the active tab sits in the Tab order.
  function handleKeyDown(event: React.KeyboardEvent<HTMLButtonElement>, index: number) {
    let nextIndex: number | null = null;
    switch (event.key) {
      case "ArrowDown":
        nextIndex = (index + 1) % NAV_ITEMS.length;
        break;
      case "ArrowUp":
        nextIndex = (index - 1 + NAV_ITEMS.length) % NAV_ITEMS.length;
        break;
      case "Home":
        nextIndex = 0;
        break;
      case "End":
        nextIndex = NAV_ITEMS.length - 1;
        break;
      default:
        return;
    }
    event.preventDefault();
    activate(NAV_ITEMS[nextIndex].id, { moveFocus: true });
  }

  return (
    <nav className="bh-sidebar" aria-label="Views">
      <div className="bh-sidebar__brand">
        <span className="bh-sidebar__brand-mark">bh</span>
        <span className="bh-sidebar__brand-name">borehole</span>
      </div>
      <ul
        className="bh-sidebar__list"
        role="tablist"
        aria-orientation="vertical"
        aria-label="Views"
      >
        {NAV_ITEMS.map((item, index) => {
          const active = view === item.id;
          return (
            <li key={item.id} role="presentation">
              <button
                ref={(el) => {
                  if (el) itemRefs.current.set(item.id, el);
                  else itemRefs.current.delete(item.id);
                }}
                type="button"
                id={tabId(item.id)}
                role="tab"
                aria-selected={active}
                aria-controls={VIEW_PANEL_ID}
                tabIndex={active ? 0 : -1}
                className={`bh-sidebar__item${active ? " bh-sidebar__item--active" : ""}`}
                onClick={() => activate(item.id, { moveFocus: false })}
                onKeyDown={(e) => handleKeyDown(e, index)}
              >
                {item.label}
              </button>
            </li>
          );
        })}
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

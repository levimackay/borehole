import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { useAppState, type ViewId } from "../state/AppStateContext";
import { focusViewPanel } from "./layout/Sidebar";
import "./CommandPalette.css";

interface NotAvailableNotice {
  title: string;
  reason: string;
}

export function CommandPalette() {
  const [open, setOpen] = useState(false);
  const [notice, setNotice] = useState<NotAvailableNotice | null>(null);
  const { selectedSymbol, setView, setGraphMode, pickAndOpenRepository } = useAppState();

  useEffect(() => {
    function onKeyDown(e: KeyboardEvent) {
      if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setOpen((prev) => !prev);
      }
      if (e.key === "Escape") setOpen(false);
    }
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (!open) setNotice(null);
  }, [open]);

  function goTo(view: ViewId) {
    setView(view);
    setOpen(false);
    focusViewPanel();
  }

  function goToOrPickSymbolFirst(view: ViewId) {
    // These views need a selected symbol; if there isn't one, send the
    // user to Symbols to pick one rather than rendering a view with
    // nothing to show.
    setView(selectedSymbol ? view : "symbols");
    setOpen(false);
    focusViewPanel();
  }

  function showNotAvailable(title: string) {
    setNotice({
      title,
      reason: "This command isn't wired up yet. No view exists for it in this build.",
    });
  }

  return (
    <Command.Dialog
      open={open}
      onOpenChange={setOpen}
      label="Command palette"
      className="bh-cmdk"
      shouldFilter
    >
      <div className="bh-cmdk__input-row">
        <Command.Input autoFocus placeholder="Type a command…" />
      </div>
      {notice ? (
        <div className="bh-cmdk__notice" role="status">
          <p className="bh-cmdk__notice-title">{notice.title}</p>
          <p className="bh-cmdk__notice-reason">{notice.reason}</p>
          <button type="button" onClick={() => setNotice(null)}>
            Back
          </button>
        </div>
      ) : (
        <Command.List>
          <Command.Empty>No matching command.</Command.Empty>
          <Command.Group heading="Repository">
            <Command.Item
              onSelect={() => {
                setOpen(false);
                void pickAndOpenRepository();
              }}
            >
              Open Repository
            </Command.Item>
          </Command.Group>
          <Command.Group heading="Symbols">
            <Command.Item onSelect={() => goTo("search")}>Search Symbol</Command.Item>
            <Command.Item
              onSelect={() => {
                setGraphMode("callers");
                goToOrPickSymbolFirst("graph");
              }}
            >
              Show Callers{selectedSymbol ? ` of ${selectedSymbol.name}` : ""}
            </Command.Item>
            <Command.Item
              onSelect={() => {
                setGraphMode("callees");
                goToOrPickSymbolFirst("graph");
              }}
            >
              Show Callees{selectedSymbol ? ` of ${selectedSymbol.name}` : ""}
            </Command.Item>
            <Command.Item onSelect={() => goToOrPickSymbolFirst("impact")}>
              Show Impact
            </Command.Item>
            <Command.Item onSelect={() => goToOrPickSymbolFirst("history")}>
              Show History
            </Command.Item>
            <Command.Item onSelect={() => goToOrPickSymbolFirst("tests")}>Find Tests</Command.Item>
          </Command.Group>
          <Command.Group heading="Other">
            <Command.Item onSelect={() => showNotAvailable("Export Context")}>
              Export Context
            </Command.Item>
            <Command.Item onSelect={() => showNotAvailable("Settings")}>Settings</Command.Item>
          </Command.Group>
        </Command.List>
      )}
    </Command.Dialog>
  );
}

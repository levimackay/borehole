import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { open as openDirectoryDialog } from "@tauri-apps/plugin-dialog";
import { commands, onIndexComplete, onIndexError, onIndexProgress } from "../lib/ipc";
import type { IndexProgress, IndexSummary, Symbol } from "../lib/ipc-types";
import { describeError } from "../lib/errors";

export type ViewId = "explorer" | "symbols" | "graph" | "history" | "impact" | "tests" | "search";

export interface RepoInfo {
  root: string;
  hasGit: boolean;
}

export type IndexStatus =
  | { kind: "idle" }
  | { kind: "indexing"; progress: IndexProgress | null }
  | { kind: "complete"; summary: IndexSummary }
  | { kind: "error"; message: string };

interface AppStateValue {
  repo: RepoInfo | null;
  isOpeningRepo: boolean;
  repoError: string | null;
  indexStatus: IndexStatus;
  selectedSymbol: Symbol | null;
  view: ViewId;
  graphMode: "callers" | "callees";
  setView: (view: ViewId) => void;
  selectSymbol: (symbol: Symbol) => void;
  setGraphMode: (mode: "callers" | "callees") => void;
  /** Opens the native directory picker, then opens + indexes the chosen repo. */
  pickAndOpenRepository: () => Promise<void>;
  /** Re-runs indexing on the currently open repo. */
  reindex: () => Promise<void>;
}

const AppStateContext = createContext<AppStateValue | null>(null);

export function AppStateProvider({ children }: { children: ReactNode }) {
  const [repo, setRepo] = useState<RepoInfo | null>(null);
  const [isOpeningRepo, setIsOpeningRepo] = useState(false);
  const [repoError, setRepoError] = useState<string | null>(null);
  const [indexStatus, setIndexStatus] = useState<IndexStatus>({ kind: "idle" });
  const [selectedSymbol, setSelectedSymbol] = useState<Symbol | null>(null);
  const [view, setView] = useState<ViewId>("explorer");
  const [graphMode, setGraphMode] = useState<"callers" | "callees">("callers");

  // Event listeners are global (not scoped to one invoke call) — subscribe
  // once for the app's lifetime.
  useEffect(() => {
    const unlistenPromises = [
      onIndexProgress((progress) => {
        setIndexStatus({ kind: "indexing", progress });
      }),
      onIndexComplete((summary) => {
        setIndexStatus({ kind: "complete", summary });
      }),
      onIndexError((message) => {
        setIndexStatus({ kind: "error", message });
      }),
    ];
    return () => {
      unlistenPromises.forEach((p) => {
        p.then((unlisten) => unlisten()).catch(() => {
          /* listener was never established; nothing to clean up */
        });
      });
    };
  }, []);

  const reindex = useCallback(async () => {
    setIndexStatus({ kind: "indexing", progress: null });
    try {
      const summary = await commands.reindex();
      setIndexStatus({ kind: "complete", summary });
    } catch (err) {
      setIndexStatus({ kind: "error", message: describeError(err) });
    }
  }, []);

  const pickAndOpenRepository = useCallback(async () => {
    setRepoError(null);
    let selection: string | string[] | null;
    try {
      selection = await openDirectoryDialog({ directory: true, multiple: false });
    } catch (err) {
      setRepoError(describeError(err));
      return;
    }
    if (!selection || Array.isArray(selection)) return; // user cancelled

    setIsOpeningRepo(true);
    try {
      const handle = await commands.openRepository(selection);
      setRepo({ root: handle.root, hasGit: handle.hasGit });
      setIsOpeningRepo(false);
      await reindex();
    } catch (err) {
      setRepoError(describeError(err));
      setIsOpeningRepo(false);
    }
  }, [reindex]);

  const selectSymbol = useCallback((symbol: Symbol) => setSelectedSymbol(symbol), []);

  const value: AppStateValue = {
    repo,
    isOpeningRepo,
    repoError,
    indexStatus,
    selectedSymbol,
    view,
    graphMode,
    setView,
    selectSymbol,
    setGraphMode,
    pickAndOpenRepository,
    reindex,
  };

  return <AppStateContext.Provider value={value}>{children}</AppStateContext.Provider>;
}

export function useAppState(): AppStateValue {
  const ctx = useContext(AppStateContext);
  if (!ctx) throw new Error("useAppState must be used within AppStateProvider");
  return ctx;
}

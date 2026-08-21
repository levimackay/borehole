import { useAppState } from "../../state/AppStateContext";
import { ProgressBar } from "../ProgressBar";
import "./IndexingStatusBar.css";

export function IndexingStatusBar() {
  const { indexStatus, reindex } = useAppState();

  if (indexStatus.kind === "idle") return null;

  if (indexStatus.kind === "indexing") {
    const progress = indexStatus.progress;
    return (
      <div className="bh-index-bar bh-index-bar--busy" role="status">
        {progress ? (
          <ProgressBar
            done={progress.files_done}
            total={progress.files_total}
            label={`Indexing repository… ${progress.files_done.toLocaleString()} / ${progress.files_total.toLocaleString()} files${
              progress.current_file != null ? ` (symbol #${progress.current_file})` : ""
            }`}
          />
        ) : (
          <span className="bh-index-bar__label">Indexing repository…</span>
        )}
      </div>
    );
  }

  if (indexStatus.kind === "error") {
    return (
      <div className="bh-index-bar bh-index-bar--error" role="alert">
        <span className="bh-index-bar__label bh-mono">{indexStatus.message}</span>
        <button type="button" className="bh-index-bar__retry" onClick={() => void reindex()}>
          Retry
        </button>
      </div>
    );
  }

  // complete
  const { summary } = indexStatus;
  return (
    <div className="bh-index-bar bh-index-bar--complete">
      <span className="bh-index-bar__label">
        Indexed {summary.files_indexed.toLocaleString()} files —{" "}
        {summary.symbols_indexed.toLocaleString()} symbols,{" "}
        {summary.references_resolved.toLocaleString()}/
        {summary.references_indexed.toLocaleString()} references resolved (
        {summary.duration_ms.toLocaleString()}ms)
      </span>
      <button type="button" className="bh-index-bar__retry" onClick={() => void reindex()}>
        Re-index
      </button>
    </div>
  );
}

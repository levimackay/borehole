import { useAppState } from "../state/AppStateContext";
import { EmptyState } from "../components/EmptyState";
import { Panel } from "../components/Panel";
import "./ExplorerView.css";

export function ExplorerView() {
  const { repo, isOpeningRepo, repoError, indexStatus, pickAndOpenRepository, reindex } =
    useAppState();

  if (!repo) {
    return (
      <div className="bh-view bh-explorer-view bh-explorer-view--empty">
        <EmptyState
          title="No repository open"
          description="Open a local repository to index it and start exploring symbols, dependencies, and history."
          action={
            <button
              type="button"
              className="bh-explorer-view__open-btn"
              onClick={() => void pickAndOpenRepository()}
              disabled={isOpeningRepo}
            >
              {isOpeningRepo ? "Opening…" : "Open Repository"}
            </button>
          }
        />
        {repoError && (
          <div className="bh-explorer-view__error" role="alert">
            <p className="bh-explorer-view__error-title">Couldn't open repository</p>
            <p className="bh-explorer-view__error-message bh-mono">{repoError}</p>
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="bh-view bh-explorer-view">
      <Panel
        title="Repository"
        actions={
          <button type="button" onClick={() => void reindex()}>
            Re-index
          </button>
        }
      >
        <dl className="bh-explorer-view__facts">
          <div>
            <dt>Root</dt>
            <dd className="bh-mono">{repo.root}</dd>
          </div>
          <div>
            <dt>Version control</dt>
            <dd>{repo.hasGit ? "git repository detected" : "no git history detected"}</dd>
          </div>
        </dl>
      </Panel>

      <Panel title="Index status">
        {indexStatus.kind === "idle" && <p>Not indexed yet.</p>}
        {indexStatus.kind === "indexing" && (
          <p>
            Indexing in progress
            {indexStatus.progress
              ? ` — ${indexStatus.progress.files_done.toLocaleString()} / ${indexStatus.progress.files_total.toLocaleString()} files`
              : "…"}
          </p>
        )}
        {indexStatus.kind === "error" && (
          <p className="bh-mono" style={{ color: "var(--bh-risk-critical-fg)" }}>
            {indexStatus.message}
          </p>
        )}
        {indexStatus.kind === "complete" && (
          <dl className="bh-explorer-view__facts">
            <div>
              <dt>Files indexed</dt>
              <dd>{indexStatus.summary.files_indexed.toLocaleString()}</dd>
            </div>
            <div>
              <dt>Files skipped (unsupported)</dt>
              <dd>{indexStatus.summary.files_skipped_unsupported.toLocaleString()}</dd>
            </div>
            <div>
              <dt>Files skipped (too large)</dt>
              <dd>{indexStatus.summary.files_skipped_too_large.toLocaleString()}</dd>
            </div>
            <div>
              <dt>Symbols indexed</dt>
              <dd>{indexStatus.summary.symbols_indexed.toLocaleString()}</dd>
            </div>
            <div>
              <dt>References</dt>
              <dd>
                {indexStatus.summary.references_resolved.toLocaleString()} resolved /{" "}
                {indexStatus.summary.references_indexed.toLocaleString()} total
              </dd>
            </div>
            <div>
              <dt>Duration</dt>
              <dd>{indexStatus.summary.duration_ms.toLocaleString()}ms</dd>
            </div>
          </dl>
        )}
      </Panel>
    </div>
  );
}

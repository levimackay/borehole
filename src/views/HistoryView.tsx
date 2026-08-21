import { useEffect, useMemo, useState } from "react";
import { useAppState } from "../state/AppStateContext";
import { commands } from "../lib/ipc";
import { useAsync } from "../hooks/useAsync";
import { AsyncSection } from "../components/AsyncSection";
import { ConfidenceBadge } from "../components/ConfidenceBadge";
import { Panel } from "../components/Panel";
import { formatTimestamp, shortSha } from "../lib/format";
import "./HistoryView.css";

export function HistoryView() {
  const { selectedSymbol, repo } = useAppState();
  const [query, setQuery] = useState("");
  const [submitted, setSubmitted] = useState("");

  // Seed the query from the selected symbol, but let the user override it —
  // getHistory takes any path-or-symbol string, not just an id.
  useEffect(() => {
    if (selectedSymbol) {
      setQuery(selectedSymbol.qualified_name);
      setSubmitted(selectedSymbol.qualified_name);
    }
  }, [selectedSymbol]);

  const factory = useMemo(() => {
    if (!submitted) return null;
    return () => commands.getHistory(submitted);
  }, [submitted]);

  const history = useAsync(factory, [submitted]);

  return (
    <div className="bh-view bh-history-view">
      <Panel title="Code evolution">
        <form
          className="bh-history-view__form"
          onSubmit={(e) => {
            e.preventDefault();
            setSubmitted(query.trim());
          }}
        >
          <input
            type="text"
            className="bh-history-view__input"
            placeholder={repo ? "Symbol qualified name or file path…" : "Open a repository first"}
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            disabled={!repo}
            aria-label="Path or qualified symbol name"
          />
          <button type="submit" disabled={!repo || !query.trim()}>
            Look up
          </button>
        </form>

        <AsyncSection
          state={history}
          onRetry={history.retry}
          idleMessage={
            repo
              ? "Select a symbol, or enter a path/qualified name above."
              : "No repository open yet."
          }
        >
          {(evolution) => (
            <div className="bh-history-view__result">
              <div className="bh-history-view__summary">
                <ConfidenceBadge confidence={evolution.confidence} />
                <span className="bh-history-view__target bh-mono">
                  {evolution.target_path_or_symbol}
                </span>
              </div>

              <section>
                <h3>Introduced</h3>
                {evolution.introduced ? (
                  <CommitLine commit={evolution.introduced} />
                ) : (
                  <p className="bh-history-view__unknown">
                    Unknown: no introducing commit found.
                  </p>
                )}
              </section>

              <section>
                <h3>Authors ({evolution.authors.length})</h3>
                {evolution.authors.length > 0 ? (
                  <ul className="bh-history-view__authors">
                    {evolution.authors.map((a) => (
                      <li key={a}>{a}</li>
                    ))}
                  </ul>
                ) : (
                  <p className="bh-history-view__unknown">No authors recorded.</p>
                )}
              </section>

              <section>
                <h3>Major refactors ({evolution.major_refactors.length})</h3>
                {evolution.major_refactors.length > 0 ? (
                  <ul className="bh-history-view__commits">
                    {evolution.major_refactors.map((c) => (
                      <li key={c.sha}>
                        <CommitLine commit={c} />
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="bh-history-view__unknown">None detected.</p>
                )}
              </section>

              <section>
                <h3>All commits ({evolution.all_commits.length})</h3>
                {evolution.all_commits.length > 0 ? (
                  <ul className="bh-history-view__commits">
                    {evolution.all_commits.map((c) => (
                      <li key={c.sha}>
                        <CommitLine commit={c} />
                      </li>
                    ))}
                  </ul>
                ) : (
                  <p className="bh-history-view__unknown">No commit history found.</p>
                )}
              </section>

              <section>
                <h3>Co-changing files ({evolution.co_changing_files.length})</h3>
                {evolution.co_changing_files.length > 0 ? (
                  <table className="bh-history-view__cochange">
                    <thead>
                      <tr>
                        <th>File</th>
                        <th>Co-changes</th>
                        <th>Ratio</th>
                      </tr>
                    </thead>
                    <tbody>
                      {evolution.co_changing_files.map((edge) => (
                        <tr key={edge.file}>
                          <td className="bh-mono">{edge.file}</td>
                          <td>
                            {edge.co_change_count} / {edge.total_commits_either_file}
                          </td>
                          <td>{(edge.ratio * 100).toFixed(0)}%</td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                ) : (
                  <p className="bh-history-view__unknown">No co-change data found.</p>
                )}
              </section>
            </div>
          )}
        </AsyncSection>
      </Panel>
    </div>
  );
}

function CommitLine({
  commit,
}: {
  commit: { sha: string; author_name: string; timestamp: number; summary: string };
}) {
  return (
    <p className="bh-history-view__commit-line">
      <span className="bh-mono">{shortSha(commit.sha)}</span>
      <span>{commit.summary}</span>
      <span className="bh-history-view__commit-meta">
        {commit.author_name} · {formatTimestamp(commit.timestamp)}
      </span>
    </p>
  );
}

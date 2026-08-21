import type { ReactNode } from "react";
import type { AsyncState } from "../hooks/useAsync";
import "./AsyncSection.css";

/**
 * Renders one of the three honest states for an async fetch: loading, error
 * (verbatim message, never swallowed), or success. Never fabricates a
 * fourth "looks fine" state.
 */
export function AsyncSection<T>({
  state,
  onRetry,
  idleMessage,
  children,
}: {
  state: AsyncState<T>;
  onRetry?: () => void;
  idleMessage?: string;
  children: (data: T) => ReactNode;
}) {
  if (state.status === "idle") {
    return <p className="bh-async__idle">{idleMessage ?? "Nothing to show yet."}</p>;
  }
  if (state.status === "loading") {
    return (
      <p className="bh-async__loading" role="status">
        Loading…
      </p>
    );
  }
  if (state.status === "error") {
    return (
      <div className="bh-async__error" role="alert">
        <p className="bh-async__error-message bh-mono">{state.error}</p>
        {onRetry && (
          <button type="button" className="bh-async__retry" onClick={onRetry}>
            Retry
          </button>
        )}
      </div>
    );
  }
  return <>{children(state.data)}</>;
}

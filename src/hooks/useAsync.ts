import { useEffect, useState, useCallback, type DependencyList } from "react";
import { describeError } from "../lib/errors";

export type AsyncState<T> =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "error"; error: string }
  | { status: "success"; data: T };

/**
 * Runs `factory()` whenever `deps` changes and tracks loading/error/success.
 * Pass `null` for factory to represent "not ready yet" (renders as idle,
 * fetches nothing) — e.g. no symbol selected.
 */
export function useAsync<T>(
  factory: (() => Promise<T>) | null,
  deps: DependencyList,
): AsyncState<T> & { retry: () => void } {
  const [nonce, setNonce] = useState(0);
  const [state, setState] = useState<AsyncState<T>>({ status: "idle" });

  useEffect(() => {
    if (!factory) {
      setState({ status: "idle" });
      return;
    }
    let cancelled = false;
    setState({ status: "loading" });
    factory()
      .then((data) => {
        if (!cancelled) setState({ status: "success", data });
      })
      .catch((err: unknown) => {
        if (!cancelled) setState({ status: "error", error: describeError(err) });
      });
    return () => {
      cancelled = true;
    };
    // deps is caller-controlled and factory is expected to close over it.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [...deps, nonce]);

  const retry = useCallback(() => setNonce((n) => n + 1), []);

  return { ...state, retry };
}

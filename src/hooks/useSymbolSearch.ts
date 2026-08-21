import { useMemo } from "react";
import { commands } from "../lib/ipc";
import { useAsync } from "./useAsync";
import { useDebouncedValue } from "./useDebouncedValue";

const DEBOUNCE_MS = 275;

/**
 * Shared debounced symbol search. Returns idle for an empty/whitespace-only
 * query rather than firing a search — the backend contract for what an
 * empty query returns is unspecified, so don't guess at it.
 */
export function useSymbolSearch(query: string, limit = 25) {
  const debounced = useDebouncedValue(query.trim(), DEBOUNCE_MS);

  const factory = useMemo(() => {
    if (!debounced) return null;
    return () => commands.searchSymbols(debounced, limit);
  }, [debounced, limit]);

  return useAsync(factory, [debounced, limit]);
}

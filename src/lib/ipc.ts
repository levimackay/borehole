/**
 * Typed wrapper over Tauri's invoke()/listen(). Components never call
 * `@tauri-apps/api` directly — see docs/ARCHITECTURE.md "Application API /
 * IPC contract" for the full command table (this mirrors it 1:1 with the
 * CLI). Command handlers land in `src-tauri/src/commands/`; until they do,
 * every function here will reject at runtime with a "command not found"
 * error, which is expected during scaffolding — the point of this file
 * existing now is that UI components can be built against a stable,
 * type-checked surface without waiting on the Rust side.
 */

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  BeforeYouChangeThisReport,
  BlastRadius,
  CodeEvolution,
  DependencyGraph,
  IndexProgress,
  IndexSummary,
  SymbolProfile,
  Symbol,
  SymbolId,
  TestRef,
} from "./ipc-types";

export interface RepoHandleRef {
  root: string;
  hasGit: boolean;
}

export const commands = {
  openRepository: (path: string) => invoke<RepoHandleRef>("open_repository", { path }),

  reindex: () => invoke<IndexSummary>("reindex"),

  searchSymbols: (query: string, limit = 25) =>
    invoke<Symbol[]>("search_symbols", { query, limit }),

  getSymbolProfile: (id: SymbolId) => invoke<SymbolProfile>("get_symbol_profile", { id }),

  getCallers: (id: SymbolId, maxDepth?: number) =>
    invoke<DependencyGraph>("get_callers", { id, maxDepth }),

  getCallees: (id: SymbolId, maxDepth?: number) =>
    invoke<DependencyGraph>("get_callees", { id, maxDepth }),

  getBlastRadius: (id: SymbolId) => invoke<BlastRadius>("get_blast_radius", { id }),

  getBeforeYouChangeThis: (id: SymbolId) =>
    invoke<BeforeYouChangeThisReport>("get_before_you_change_this", { id }),

  getHistory: (pathOrSymbol: string) => invoke<CodeEvolution>("get_history", { pathOrSymbol }),

  getRelatedTests: (id: SymbolId) => invoke<TestRef[]>("get_related_tests", { id }),

  exportContext: (pathOrSymbol: string, format: "md" | "json") =>
    invoke<string>("export_context", { pathOrSymbol, format }),
};

export function onIndexProgress(handler: (progress: IndexProgress) => void): Promise<UnlistenFn> {
  return listen<IndexProgress>("index-progress", (event) => handler(event.payload));
}

export function onIndexComplete(handler: (summary: IndexSummary) => void): Promise<UnlistenFn> {
  return listen<IndexSummary>("index-complete", (event) => handler(event.payload));
}

export function onIndexError(handler: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>("index-error", (event) => handler(event.payload));
}

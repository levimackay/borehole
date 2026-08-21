/**
 * TypeScript mirror of the serde types in `engine-core` and
 * `engine-evidence`. Field-for-field — see docs/ARCHITECTURE.md
 * "Application API / IPC contract". Keep this in sync by hand; there is no
 * codegen step yet (a `ts-rs`/`specta` generation pass is a reasonable
 * follow-up once these types stop changing weekly).
 */

// ---- engine-core -----------------------------------------------------

export type Confidence = "high" | "medium" | "low";

export type SymbolKind =
  | "function"
  | "method"
  | "class"
  | "interface"
  | "struct"
  | "enum"
  | "trait"
  | "module"
  | "variable"
  | "constant"
  | "type_alias"
  | "macro";

export type Language = "rust" | "typescript" | "tsx" | "javascript" | "python" | "go";

export interface Span {
  start_line: number;
  start_col: number;
  end_line: number;
  end_col: number;
  start_byte: number;
  end_byte: number;
}

/** Opaque numeric id — treat as an identifier, never do arithmetic on it. */
export type SymbolId = number;

export interface Symbol {
  id: SymbolId;
  name: string;
  qualified_name: string;
  kind: SymbolKind;
  language: Language;
  file: string; // RepoPath, forward-slash-normalized, repo-relative
  span: Span;
  is_exported: boolean;
  doc_comment: string | null;
  signature: string | null;
}

export type ReferenceKind =
  | "call"
  | "import"
  | "extends"
  | "implements"
  | "type_usage"
  | "instantiation";

export interface Reference {
  from_symbol: SymbolId | null;
  from_file: string;
  from_span: Span;
  to_symbol: SymbolId | null;
  to_name: string;
  kind: ReferenceKind;
  confidence: Confidence;
}

export type RiskTier = "low" | "medium" | "high" | "critical";

export type EvidenceRef =
  | { kind: "reference"; file: string; span: Span }
  | { kind: "commit"; sha: string; summary: string }
  | { kind: "test_file"; file: string }
  | { kind: "config_file"; file: string; key: string | null };

// ---- engine-index ------------------------------------------------------

export interface IndexProgress {
  files_done: number;
  files_total: number;
  current_file: number | null;
}

export interface IndexSummary {
  files_indexed: number;
  files_skipped_unsupported: number;
  files_skipped_too_large: number;
  symbols_indexed: number;
  references_indexed: number;
  references_resolved: number;
  duration_ms: number;
}

// ---- engine-graph --------------------------------------------------------

export interface GraphNode {
  symbol: Symbol;
  depth: number;
}

export interface GraphEdge {
  from: SymbolId;
  to: SymbolId;
  reference: Reference;
}

export interface DependencyGraph {
  root: SymbolId | null;
  nodes: GraphNode[];
  edges: GraphEdge[];
  truncated: boolean;
}

// ---- engine-git ----------------------------------------------------------

export interface CommitInfo {
  sha: string;
  author_name: string;
  author_email: string;
  timestamp: number; // unix seconds
  summary: string;
  files_changed: number;
}

export interface CoChangeEdge {
  file: string;
  co_change_count: number;
  total_commits_either_file: number;
  ratio: number;
}

// ---- engine-evidence -------------------------------------------------

export type TestMatchReason = "naming_convention" | "direct_import" | "directory_convention";

export interface TestRef {
  file: string;
  reason: TestMatchReason;
  confidence: Confidence;
}

export interface ConfigRef {
  file: string;
  key: string | null;
  evidence: EvidenceRef;
}

export interface SymbolProfile {
  symbol: Symbol;
  direct_callers: number;
  indirect_callers: number;
  tests: TestRef[];
  config: ConfigRef[];
  introduced: CommitInfo | null;
  modification_count: number;
}

export interface RiskReason {
  text: string;
  evidence: EvidenceRef[];
}

export interface BlastRadius {
  target: Symbol;
  direct_callers: number;
  indirect_callers: number;
  test_suites: number;
  is_public_api: boolean;
  config_files: ConfigRef[];
  risk: RiskTier;
  risk_reasons: RiskReason[];
  caller_graph: DependencyGraph;
}

export interface ReadingListItem {
  symbol_or_file: string;
  why: string;
  evidence: EvidenceRef[];
}

export interface HistoricalWarning {
  text: string;
  commits: EvidenceRef[];
}

export interface BeforeYouChangeThisReport {
  target: Symbol;
  reading_list: ReadingListItem[];
  blast_radius: BlastRadius;
  historical_warnings: HistoricalWarning[];
  confidence: Confidence;
}

export interface CodeEvolution {
  target_path_or_symbol: string;
  introduced: CommitInfo | null;
  major_refactors: CommitInfo[];
  all_commits: CommitInfo[];
  authors: string[];
  co_changing_files: CoChangeEdge[];
  confidence: Confidence;
}

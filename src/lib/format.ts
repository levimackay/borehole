import type { SymbolKind, Language, TestMatchReason } from "./ipc-types";

const KIND_LABEL: Record<SymbolKind, string> = {
  function: "fn",
  method: "method",
  class: "class",
  interface: "interface",
  struct: "struct",
  enum: "enum",
  trait: "trait",
  module: "module",
  variable: "var",
  constant: "const",
  type_alias: "type",
  macro: "macro",
};

export function formatKind(kind: SymbolKind): string {
  return KIND_LABEL[kind];
}

const LANGUAGE_LABEL: Record<Language, string> = {
  rust: "Rust",
  typescript: "TypeScript",
  tsx: "TSX",
  javascript: "JavaScript",
  python: "Python",
  go: "Go",
};

export function formatLanguage(language: Language): string {
  return LANGUAGE_LABEL[language];
}

const TEST_REASON_LABEL: Record<TestMatchReason, string> = {
  naming_convention: "naming convention",
  direct_import: "direct import",
  directory_convention: "directory convention",
};

export function formatTestReason(reason: TestMatchReason): string {
  return TEST_REASON_LABEL[reason];
}

export function formatTimestamp(unixSeconds: number): string {
  return new Date(unixSeconds * 1000).toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function shortSha(sha: string): string {
  return sha.slice(0, 7);
}

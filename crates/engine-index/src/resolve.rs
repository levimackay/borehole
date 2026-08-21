//! Reference resolution: turning `engine_parse::ParsedReference::to_name`
//! (raw identifier text) plus the file's `ImportEdge`s into an actual
//! `SymbolId`, with a `Confidence`.
//!
//! ALGORITHM CONTRACT (implement this, don't redesign it without updating
//! docs/ARCHITECTURE.md "Reference resolution"):
//!
//! 1. Same-file candidates first: if a symbol named `to_name` exists in the
//!    same file, prefer it (`Confidence::High` if it's the only same-file
//!    candidate).
//! 2. Otherwise, check the file's imports: if exactly one `ImportEdge` binds
//!    `to_name` as `local_name`, resolve `source_module` to a file (see
//!    `resolve_module_path`) and look up `imported_name` there
//!    (`Confidence::High` if found, `Confidence::Medium` if the module
//!    resolves but the name isn't found there — e.g. re-export chains not
//!    yet followed).
//! 3. Otherwise, fall back to a global name search across the whole index:
//!    if exactly one symbol anywhere has this name, use it at
//!    `Confidence::Medium`; if more than one, DO NOT silently pick one —
//!    return `to_symbol: None` with `Confidence::Low` (see the
//!    `ambiguous-refs` fixture — merging distinct same-named symbols is a
//!    correctness bug, not a convenience).
//! 4. No candidate anywhere: `to_symbol: None`, `Confidence::Low`. This is
//!    expected and fine for calls into external dependencies.

use engine_core::{Confidence, RepoPath, SymbolId};
use engine_parse::ImportEdge;

pub struct ResolutionContext<'a> {
    pub file: &'a RepoPath,
    pub imports: &'a [ImportEdge],
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub symbol: Option<SymbolId>,
    pub confidence: Confidence,
}

/// Resolve a language-specific module specifier (e.g. `"./session"`,
/// `"auth::session"`, `"..models"`) to a `RepoPath`, relative to the
/// importing file. Returns `None` for specifiers that clearly point outside
/// the repo (bare package names like `"react"`, `"os"`, external crates) —
/// those are legitimately unresolvable and should surface as
/// `Confidence::Low`, not an error.
pub fn resolve_module_path(
    _from_file: &RepoPath,
    _source_module: &str,
    _language: engine_core::Language,
) -> Option<RepoPath> {
    todo!("engine-index::resolve: resolve_module_path — see module doc comment for the algorithm")
}

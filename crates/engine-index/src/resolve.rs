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
//!
//! `resolve_module_path` (below) has no filesystem or database access — its
//! frozen signature is `(from_file, source_module, language) -> Option<RepoPath>`,
//! so it cannot itself verify that the path it computes exists. It therefore
//! returns a *stem* path (language-appropriate join of the specifier against
//! the importing file's directory, extension stripped where the language's
//! import syntax doesn't include one) rather than a verified file path. The
//! caller (`Index`, which has the full set of on-disk file paths from the
//! current walk) is responsible for trying the language's ordered list of
//! suffixes (`.ts`, `.tsx`, `.js`, `.jsx`, `/index.ts`, ... for TS/JS;
//! `.py`, `/__init__.py` for Python; `.rs`, `/mod.rs` for Rust) against that
//! stem and checking existence — see `Index::resolve_import_target` in
//! `lib.rs`. Bare/package specifiers that can't possibly be repo-relative
//! (`"react"`, `"os"`, `"fmt"`, external crate names) return `None` directly
//! from this function since no stem-plus-suffix search could ever find them
//! in the repo.

use engine_core::{Confidence, Language, RepoPath, SymbolId};
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
///
/// See the module doc comment above for why the returned path is a *stem*
/// (extension-less where the language allows it) rather than a
/// existence-verified file path: this function has no filesystem access.
pub fn resolve_module_path(
    from_file: &RepoPath,
    source_module: &str,
    language: Language,
) -> Option<RepoPath> {
    match language {
        Language::TypeScript | Language::Tsx | Language::JavaScript => {
            resolve_ts_js_relative(from_file, source_module)
        }
        Language::Python => resolve_python_relative(from_file, source_module),
        Language::Rust => resolve_rust_path(from_file, source_module),
        // Go import paths are either stdlib/module-path specifiers we have
        // no reliable repo-relative mapping for without parsing go.mod's
        // module directive — out of scope for v1's name-heuristic resolver,
        // see docs/ARCHITECTURE.md. Always unresolvable to a repo file.
        Language::Go => None,
    }
}

/// Directory segments of `path`, i.e. every path component except the final
/// (file name) one.
fn dir_segments(path: &RepoPath) -> Vec<&str> {
    let mut segments: Vec<&str> = path.as_str().split('/').collect();
    segments.pop();
    segments
}

/// TS/JS relative specifiers (`./foo`, `../bar/baz`) only — bare/package
/// specifiers (`react`, `lodash/fp`) are not repo-relative and return
/// `None`.
fn resolve_ts_js_relative(from_file: &RepoPath, spec: &str) -> Option<RepoPath> {
    if !(spec.starts_with("./") || spec.starts_with("../") || spec == "." || spec == "..") {
        return None;
    }
    let mut segments = dir_segments(from_file);
    for part in spec.split('/') {
        match part {
            "." | "" => {}
            ".." => {
                segments.pop();
            }
            other => segments.push(other),
        }
    }
    Some(RepoPath::new(segments.join("/")))
}

/// Python relative imports (`from .foo import x`, `from ..pkg.foo import
/// x`, `from . import x`). One leading dot means "this package" (the
/// directory containing the importing file); each additional dot walks up
/// one more directory — the standard Python relative-import level
/// semantics, simplified to a pure directory-structure mapping (no
/// `__init__.py`-vs-plain-module distinction, since that requires knowing
/// whether the importing file itself is a package `__init__.py`, which
/// `resolve_module_path` isn't given). Absolute (non-dotted) Python imports
/// (`import os`, `from mypkg.sub import x`) are treated as unresolvable
/// here — v1 doesn't attempt to distinguish an internal absolute import
/// from a third-party package one without also knowing the repo's package
/// root, so both surface as `Confidence::Low` via the "nothing found"
/// path, same as an external dependency.
fn resolve_python_relative(from_file: &RepoPath, spec: &str) -> Option<RepoPath> {
    if !spec.starts_with('.') {
        return None;
    }
    let dots = spec.chars().take_while(|&c| c == '.').count();
    let remainder = &spec[dots..];

    let mut segments: Vec<String> = dir_segments(from_file)
        .into_iter()
        .map(String::from)
        .collect();
    // One dot = stay in the current directory; each further dot climbs one
    // more level.
    for _ in 0..dots.saturating_sub(1) {
        segments.pop();
    }
    for part in remainder.split('.') {
        if !part.is_empty() {
            segments.push(part.to_string());
        }
    }
    Some(RepoPath::new(segments.join("/")))
}

/// Rust `crate::`/`super::`/`self::` paths, resolved against the crate's
/// module tree.
///
/// SIMPLIFICATION (documented per the task brief rather than filed as a
/// contract issue: this is an implementation choice forced by
/// `resolve_module_path` having no filesystem access, not a defect in the
/// frozen signature). Finding the *real* crate root means walking up from
/// `from_file` to the nearest `Cargo.toml`, which requires filesystem
/// access this function doesn't have. Instead we locate the first `src/`
/// path segment in `from_file` and treat everything from there as the
/// module tree root. This is exactly right for the standard single-crate
/// layout (crate root directly containing `src/`, covering the
/// `rust-basic` fixture and the common case in this repo's own workspace)
/// and for simple workspaces where each member crate also follows that
/// layout. It would misresolve a crate that doesn't use a `src/`
/// directory at all, or a path that happens to contain a `src` component
/// that isn't the crate root (e.g. a vendored `some/src/thing` that isn't
/// actually a Cargo package) — both are rare enough in real Rust
/// workspaces that this is an acceptable v1 trade rather than a genuine
/// resolver defect.
fn resolve_rust_path(from_file: &RepoPath, spec: &str) -> Option<RepoPath> {
    let parts: Vec<&str> = spec.split("::").collect();
    let (head, rest) = parts.split_first()?;
    match *head {
        "crate" => {
            let src_root = find_src_root(from_file)?;
            let mut segments = src_root;
            segments.extend(rest.iter().map(|s| s.to_string()));
            Some(RepoPath::new(segments.join("/")))
        }
        "super" | "self" => {
            let src_root = find_src_root(from_file)?;
            let mut segments = rust_module_segments(from_file)?; // relative to src_root
            if *head == "super" {
                segments.pop();
            }
            segments.extend(rest.iter().map(|s| s.to_string()));
            let mut full = src_root;
            full.extend(segments);
            Some(RepoPath::new(full.join("/")))
        }
        // External crate (`std::...`, third-party crates) or a bare
        // identifier — not repo-relative.
        _ => None,
    }
}

fn find_src_root(from_file: &RepoPath) -> Option<Vec<String>> {
    let full: Vec<&str> = from_file.as_str().split('/').collect();
    let idx = full.iter().position(|&s| s == "src")?;
    Some(full[..=idx].iter().map(|s| s.to_string()).collect())
}

/// The module path segments (relative to the crate's `src/` root) that
/// `from_file` itself belongs to — `mod.rs`/`lib.rs`/`main.rs` represent
/// their *containing* directory as the module, everything else represents
/// itself (minus the `.rs` extension).
fn rust_module_segments(from_file: &RepoPath) -> Option<Vec<String>> {
    let src_root = find_src_root(from_file)?;
    let full: Vec<&str> = from_file.as_str().split('/').collect();
    if full.len() < src_root.len() {
        return None;
    }
    let mut segments: Vec<String> = full[src_root.len()..]
        .iter()
        .map(|s| s.to_string())
        .collect();
    if let Some(last) = segments.last_mut() {
        let stem = last.strip_suffix(".rs").unwrap_or(last).to_string();
        if stem == "mod" || stem == "lib" || stem == "main" {
            segments.pop();
        } else {
            *last = stem;
        }
    }
    Some(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ts_relative_sibling() {
        let from = RepoPath::new("a.ts");
        let got = resolve_module_path(&from, "./b", Language::TypeScript).unwrap();
        assert_eq!(got.as_str(), "b");
    }

    #[test]
    fn ts_relative_parent_dir() {
        let from = RepoPath::new("packages/app/index.ts");
        let got = resolve_module_path(&from, "../core/index", Language::TypeScript).unwrap();
        assert_eq!(got.as_str(), "packages/core/index");
    }

    #[test]
    fn ts_bare_package_unresolvable() {
        let from = RepoPath::new("a.ts");
        assert!(resolve_module_path(&from, "react", Language::TypeScript).is_none());
    }

    #[test]
    fn python_single_dot_sibling() {
        let from = RepoPath::new("pkg/mod.py");
        let got = resolve_module_path(&from, ".foo", Language::Python).unwrap();
        assert_eq!(got.as_str(), "pkg/foo");
    }

    #[test]
    fn python_double_dot_climbs() {
        let from = RepoPath::new("pkg/sub/mod.py");
        let got = resolve_module_path(&from, "..pkg.foo", Language::Python).unwrap();
        assert_eq!(got.as_str(), "pkg/pkg/foo");
    }

    #[test]
    fn python_absolute_import_unresolvable() {
        let from = RepoPath::new("pkg/mod.py");
        assert!(resolve_module_path(&from, "os", Language::Python).is_none());
    }

    #[test]
    fn rust_crate_path() {
        let from = RepoPath::new("src/lib.rs");
        let got = resolve_module_path(&from, "crate::a::b", Language::Rust).unwrap();
        assert_eq!(got.as_str(), "src/a/b");
    }

    #[test]
    fn rust_super_path() {
        let from = RepoPath::new("src/a/b.rs");
        let got = resolve_module_path(&from, "super::sibling", Language::Rust).unwrap();
        assert_eq!(got.as_str(), "src/a/sibling");
    }

    #[test]
    fn rust_self_path() {
        let from = RepoPath::new("src/a/mod.rs");
        let got = resolve_module_path(&from, "self::inner", Language::Rust).unwrap();
        assert_eq!(got.as_str(), "src/a/inner");
    }

    #[test]
    fn rust_std_unresolvable() {
        let from = RepoPath::new("src/lib.rs");
        assert!(resolve_module_path(&from, "std::collections::HashMap", Language::Rust).is_none());
    }

    #[test]
    fn go_always_unresolvable() {
        let from = RepoPath::new("main.go");
        assert!(resolve_module_path(&from, "fmt", Language::Go).is_none());
        assert!(resolve_module_path(&from, "example.com/mod/pkg", Language::Go).is_none());
    }
}

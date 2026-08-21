//! Integration tests against the materialized fixtures in `fixtures/` —
//! run `fixtures/build-fixtures.sh` first (they're gitignored/generated,
//! not committed). Each test indexes one real fixture repo and asserts on
//! the actual evidence-report output, per the fixture's own `NOTES.md`.

use engine_core::{BoreholeConfig, Confidence, RepoHandle, RepoPath};
use engine_evidence::EvidenceEngine;
use engine_git::GitAnalyzer;
use engine_index::Index;
use std::path::{Path, PathBuf};

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
}

/// Bails out (rather than failing) when fixtures haven't been generated
/// yet, matching the repo's convention that `build-fixtures.sh` is a
/// prerequisite step, not something tests do for you.
macro_rules! require_fixture {
    ($dir:expr) => {
        if !$dir.exists() {
            eprintln!(
                "skipping {}: run fixtures/build-fixtures.sh first",
                $dir.display()
            );
            return;
        }
    };
}

fn index_fixture(dir: &Path) -> (Index, RepoHandle, BoreholeConfig) {
    let repo = RepoHandle::open(dir).expect("open fixture repo");
    let config = BoreholeConfig::default();
    let mut index = Index::open_in_memory().expect("open in-memory index");
    index
        .full_reindex(&repo, &config, |_| {})
        .expect("full_reindex");
    (index, repo, config)
}

// ---------------------------------------------------------------------
// ambiguous-refs: the evidence-layer-level proof that resolver honesty
// reaches the user-facing report.
// ---------------------------------------------------------------------

#[test]
fn ambiguous_refs_blast_radius_excludes_the_unresolved_caller() {
    let dir = fixture_path("ambiguous-refs");
    require_fixture!(dir);
    let (index, repo, config) = index_fixture(&dir);
    let git = GitAnalyzer::open(repo.root()).ok();
    let evidence = EvidenceEngine::new(&index, git.as_ref(), &repo, &config);

    let billing_process = index
        .search_symbols("process", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "process" && s.file.as_str() == "billing/handler.ts")
        .expect("billing::Handler.process symbol");

    let radius = evidence.blast_radius(billing_process.id).unwrap();

    // `caller.ts`'s `h.process()` call never resolved to either candidate
    // (see `engine-index`'s `ambiguous_refs_ref_is_unresolved_low_confidence`
    // test) — it must not be silently counted as a direct caller of
    // billing's `process` just because the names match.
    assert_eq!(
        radius.direct_callers, 0,
        "the ambiguous caller.ts call must not be attributed to billing::Handler.process"
    );
    assert_eq!(radius.indirect_callers, 0);
}

// ---------------------------------------------------------------------
// nested-deps: before_you_change_this reading list includes callees.
// ---------------------------------------------------------------------

#[test]
fn nested_deps_reading_list_includes_middle_as_a_dependency() {
    let dir = fixture_path("nested-deps");
    require_fixture!(dir);
    let (index, repo, config) = index_fixture(&dir);
    let git = GitAnalyzer::open(repo.root()).ok();
    let evidence = EvidenceEngine::new(&index, git.as_ref(), &repo, &config);

    let top = index
        .search_symbols("top", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "top")
        .expect("top symbol");

    let report = evidence.before_you_change_this(top.id).unwrap();

    assert!(
        report
            .reading_list
            .iter()
            .any(|item| item.symbol_or_file.contains("middle") && item.why == "this depends on it"),
        "expected `middle` in the reading list as a dependency, got: {:?}",
        report
            .reading_list
            .iter()
            .map(|i| (&i.symbol_or_file, &i.why))
            .collect::<Vec<_>>()
    );
}

// ---------------------------------------------------------------------
// inheritance: documented decision on whether subclasses count toward
// blast radius.
// ---------------------------------------------------------------------

#[test]
fn inheritance_blast_radius_counts_subclasses_via_extends_edges() {
    let dir = fixture_path("inheritance");
    require_fixture!(dir);
    let (index, repo, config) = index_fixture(&dir);
    let git = GitAnalyzer::open(repo.root()).ok();
    let evidence = EvidenceEngine::new(&index, git.as_ref(), &repo, &config);

    let shape = index
        .search_symbols("Shape", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "Shape")
        .expect("Shape symbol");

    let radius = evidence.blast_radius(shape.id).unwrap();

    // Decision (documented on `EvidenceEngine::caller_counts`): this crate
    // does not call `engine_graph::subclasses_of` separately for blast
    // radius. `Extends`/`Implements` references are ordinary `Reference`
    // rows with `to_symbol = Some(base)`, so `references_to`/
    // `expand_callers` already walk them — Polygon (direct: extends Shape)
    // and Square (indirect: extends Polygon, which extends Shape) both show
    // up through the normal caller-counting path, with no special-cased
    // inheritance handling and no double counting.
    assert_eq!(radius.direct_callers, 1, "Polygon directly extends Shape");
    assert_eq!(
        radius.indirect_callers, 1,
        "Square extends Polygon, which extends Shape — reachable at depth 2"
    );

    let subclass_names: Vec<&str> = radius
        .caller_graph
        .nodes
        .iter()
        .filter(|n| n.symbol.id != shape.id)
        .map(|n| n.symbol.name.as_str())
        .collect();
    assert!(subclass_names.contains(&"Polygon"));
    assert!(subclass_names.contains(&"Square"));
}

// ---------------------------------------------------------------------
// rename-history: code_evolution follows renames with git available.
// ---------------------------------------------------------------------

#[test]
fn rename_history_code_evolution_shows_pre_rename_commits() {
    let dir = fixture_path("rename-history");
    require_fixture!(dir);
    let (index, repo, config) = index_fixture(&dir);
    let git = GitAnalyzer::open(repo.root()).expect("rename-history fixture has real git history");
    let evidence = EvidenceEngine::new(&index, Some(&git), &repo, &config);

    let evolution = evidence
        .code_evolution(&RepoPath::new("auth-session.ts"))
        .unwrap();

    assert_eq!(
        evolution.all_commits.len(),
        4,
        "expected all 4 commits (2 pre-rename, the rename itself, 1 post-rename), got: {:?}",
        evolution
            .all_commits
            .iter()
            .map(|c| &c.summary)
            .collect::<Vec<_>>()
    );
    assert!(evolution
        .all_commits
        .iter()
        .any(|c| c.summary.contains("Add SessionService")));
    assert!(evolution
        .all_commits
        .iter()
        .any(|c| c.summary.to_lowercase().contains("rename")));
    assert!(evolution.introduced.is_some());
    assert_eq!(
        evolution.introduced.unwrap().summary,
        "Add SessionService",
        "introduced should be the oldest (pre-rename) commit"
    );
    assert_eq!(evolution.confidence, Confidence::Medium);
}

// ---------------------------------------------------------------------
// No-git case: code_evolution degrades honestly instead of erroring.
// ---------------------------------------------------------------------

#[test]
fn code_evolution_with_no_git_returns_honest_empty_shape() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(tmp.path().join("plain.ts"), "export function f() {}\n").unwrap();
    let repo = RepoHandle::open(tmp.path()).unwrap();
    let config = BoreholeConfig::default();
    let mut index = Index::open_in_memory().unwrap();
    index.full_reindex(&repo, &config, |_| {}).unwrap();

    // No `.git` at all in this tempdir — `git: None` is the honest
    // representation, not something to fake.
    let evidence = EvidenceEngine::new(&index, None, &repo, &config);
    let evolution = evidence.code_evolution(&RepoPath::new("plain.ts")).unwrap();

    assert!(evolution.introduced.is_none());
    assert!(evolution.major_refactors.is_empty());
    assert!(evolution.all_commits.is_empty());
    assert!(evolution.authors.is_empty());
    assert!(evolution.co_changing_files.is_empty());
    assert_eq!(evolution.confidence, Confidence::Low);
}

// ---------------------------------------------------------------------
// deleted-symbol: querying a since-deleted symbol id returns
// SymbolNotFound, not stale data.
// ---------------------------------------------------------------------

#[test]
fn deleted_symbol_returns_symbol_not_found_after_reindex() {
    let dir = fixture_path("deleted-symbol");
    require_fixture!(dir);

    // Build the index against an early commit where oldHelper still
    // exists, capture its id, then reindex against the current working
    // tree (where it's already deleted per the fixture's own history) and
    // confirm the id no longer resolves.
    let config = BoreholeConfig::default();

    // First index: current working tree state already has oldHelper
    // removed (per NOTES.md), so simulate "existed, then got deleted" by
    // indexing a synthetic copy that still has it, grabbing the id
    // pattern, then reindexing the real (deleted) tree at the same path
    // via a temp copy that mimics engine-index's SymbolId-preserving
    // upsert (same file, same qualified_name/kind identity).
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("legacy.ts"),
        "export function oldHelper(): string {\n  return \"old\";\n}\n\nexport function stillUsed(): string {\n  return \"no longer calls oldHelper\";\n}\n",
    )
    .unwrap();
    let tmp_repo = RepoHandle::open(tmp.path()).unwrap();
    let mut index = Index::open_in_memory().unwrap();
    index
        .full_reindex(&tmp_repo, &config, |_| {})
        .expect("full_reindex with oldHelper present");

    let old_helper = index
        .search_symbols("oldHelper", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "oldHelper")
        .expect("oldHelper symbol should exist before deletion");

    // Now delete it and reindex the same in-memory index against the same
    // path — engine-index's SymbolId-preserving matching means this row is
    // simply removed, not silently kept around.
    std::fs::write(
        tmp.path().join("legacy.ts"),
        "export function stillUsed(): string {\n  return \"no longer calls oldHelper\";\n}\n",
    )
    .unwrap();
    index
        .full_reindex(&tmp_repo, &config, |_| {})
        .expect("full_reindex after deletion");

    let git = GitAnalyzer::open(tmp_repo.root()).ok();
    let evidence = EvidenceEngine::new(&index, git.as_ref(), &tmp_repo, &config);

    let result = evidence.symbol_profile(old_helper.id);
    assert!(
        matches!(result, Err(engine_evidence::EvidenceError::SymbolNotFound(id)) if id == old_helper.id),
        "expected SymbolNotFound for a deleted symbol id, got {:?}",
        result.map(|p| p.symbol.qualified_name)
    );

    // Sanity: the real deleted-symbol fixture repo itself (with its actual
    // git history) is still queryable via file_history on legacy.ts, per
    // its NOTES.md — this doesn't require the symbol to still be indexed.
    let real_git = GitAnalyzer::open(&dir).expect("deleted-symbol fixture has real git history");
    let history = real_git
        .file_history(&RepoPath::new("legacy.ts"), None)
        .unwrap();
    assert!(
        !history.is_empty(),
        "legacy.ts's history should still be visible via git even though oldHelper is gone"
    );
}

// ---------------------------------------------------------------------
// related_tests / config_touches smoke coverage using nested-deps, which
// has no dedicated tests/config fixture — asserts the honest "nothing
// found" shape rather than a false positive.
// ---------------------------------------------------------------------

#[test]
fn nested_deps_has_no_related_tests_or_config_touches() {
    let dir = fixture_path("nested-deps");
    require_fixture!(dir);
    let (index, repo, config) = index_fixture(&dir);
    let git = GitAnalyzer::open(repo.root()).ok();
    let evidence = EvidenceEngine::new(&index, git.as_ref(), &repo, &config);

    let top = index
        .search_symbols("top", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "top")
        .expect("top symbol");

    let tests = evidence.related_tests(top.id).unwrap();
    assert!(tests.is_empty(), "nested-deps has no test files at all");

    let config_touches = evidence.config_touches(top.id).unwrap();
    assert!(
        config_touches.is_empty(),
        "nested-deps' top() reads no env vars"
    );
}

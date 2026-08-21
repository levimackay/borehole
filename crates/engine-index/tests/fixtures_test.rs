//! Integration tests against the materialized fixtures in `fixtures/` —
//! run `fixtures/build-fixtures.sh` first (they're gitignored/generated,
//! not committed). Each test indexes one real fixture repo and asserts on
//! the actual query output, per the fixture's own `NOTES.md`.

use engine_core::{BoreholeConfig, RepoHandle};
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

fn index_fixture(dir: &Path) -> (Index, RepoHandle) {
    let repo = RepoHandle::open(dir).expect("open fixture repo");
    let config = BoreholeConfig::default();
    let mut index = Index::open_in_memory().expect("open in-memory index");
    index
        .full_reindex(&repo, &config, |_| {})
        .expect("full_reindex");
    (index, repo)
}

#[test]
fn nested_deps_expand_callees_reaches_base_at_depth_two() {
    let dir = fixture_path("nested-deps");
    require_fixture!(dir);
    let (index, _repo) = index_fixture(&dir);

    let top = index
        .search_symbols("top", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "top")
        .expect("top symbol");

    // depth 1 should reach `middle` but not `base`.
    let one_hop = engine_graph::expand_callees(&index, top.id, 1).unwrap();
    assert!(one_hop.nodes.iter().any(|n| n.symbol.name == "middle"));
    assert!(!one_hop.nodes.iter().any(|n| n.symbol.name == "base"));

    // depth 2 should reach `base`.
    let two_hop = engine_graph::expand_callees(&index, top.id, 2).unwrap();
    assert!(
        two_hop.nodes.iter().any(|n| n.symbol.name == "base"),
        "expected `base` reachable from `top` at depth 2, got nodes: {:?}",
        two_hop
            .nodes
            .iter()
            .map(|n| n.symbol.name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn cycle_traversal_terminates_and_has_no_duplicate_nodes() {
    let dir = fixture_path("cycle");
    require_fixture!(dir);
    let (index, _repo) = index_fixture(&dir);

    let is_even = index
        .search_symbols("isEven", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "isEven")
        .expect("isEven symbol");

    // Bounded depth well beyond the cycle length — if this hangs or blows
    // up, the cycle guard is broken.
    let callees = engine_graph::expand_callees(&index, is_even.id, 20).unwrap();
    let callers = engine_graph::expand_callers(&index, is_even.id, 20).unwrap();

    for graph in [&callees, &callers] {
        let mut seen = std::collections::HashSet::new();
        for node in &graph.nodes {
            assert!(
                seen.insert(node.symbol.id),
                "duplicate node for symbol {:?} in traversal",
                node.symbol.id
            );
        }
        // isEven <-> isOdd: both directions should reach isOdd.
        assert!(graph.nodes.iter().any(|n| n.symbol.name == "isOdd"));
    }
}

#[test]
fn inheritance_subclasses_of_is_transitive() {
    let dir = fixture_path("inheritance");
    require_fixture!(dir);
    let (index, _repo) = index_fixture(&dir);

    let shape = index
        .search_symbols("Shape", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "Shape")
        .expect("Shape symbol");
    let polygon = index
        .search_symbols("Polygon", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "Polygon")
        .expect("Polygon symbol");

    let shape_subclasses = engine_graph::subclasses_of(&index, shape.id).unwrap();
    let names: Vec<&str> = shape_subclasses.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Polygon"), "names: {names:?}");
    assert!(names.contains(&"Square"), "names: {names:?}");

    let polygon_subclasses = engine_graph::subclasses_of(&index, polygon.id).unwrap();
    let names: Vec<&str> = polygon_subclasses.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(names, vec!["Square"]);
}

#[test]
fn deleted_symbol_vanishes_from_search_after_reindex() {
    let dir = fixture_path("deleted-symbol");
    require_fixture!(dir);

    // Work on a scratch copy so we can mutate `legacy.ts` without touching
    // the shared fixture checkout.
    let scratch = tempfile::tempdir().unwrap();
    let legacy_path = scratch.path().join("legacy.ts");

    let old_content = "export function oldHelper(): string {\n  return \"deprecated\";\n}\n\nexport function stillUsed(): string {\n  return oldHelper() + \"!\";\n}\n";
    std::fs::write(&legacy_path, old_content).unwrap();

    let repo = RepoHandle::open(scratch.path()).unwrap();
    let config = BoreholeConfig::default();
    let mut index = Index::open_in_memory().unwrap();
    index.full_reindex(&repo, &config, |_| {}).unwrap();

    let before = index.search_symbols("oldHelper", 10).unwrap();
    assert!(
        before.iter().any(|s| s.name == "oldHelper"),
        "oldHelper should exist before deletion"
    );

    // Now write the post-deletion content (matches the fixture's current
    // working-tree state) and reindex.
    let new_content =
        std::fs::read_to_string(dir.join("legacy.ts")).expect("fixture legacy.ts (post-deletion)");
    std::fs::write(&legacy_path, new_content).unwrap();
    index.full_reindex(&repo, &config, |_| {}).unwrap();

    let after = index.search_symbols("oldHelper", 10).unwrap();
    assert!(
        !after.iter().any(|s| s.name == "oldHelper"),
        "oldHelper should be gone after reindex, found: {after:?}"
    );
    let still_used = index.search_symbols("stillUsed", 10).unwrap();
    assert!(still_used.iter().any(|s| s.name == "stillUsed"));
}

#[test]
fn monorepo_cross_package_import_resolves() {
    let dir = fixture_path("monorepo");
    require_fixture!(dir);
    let (index, _repo) = index_fixture(&dir);

    let app_entry = index
        .search_symbols("appEntry", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "appEntry")
        .expect("appEntry symbol");
    let core_util = index
        .search_symbols("coreUtil", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "coreUtil")
        .expect("coreUtil symbol");

    let callees = index.references_from(app_entry.id).unwrap();
    let cross_package = callees
        .iter()
        .find(|r| r.to_name == "coreUtil")
        .expect("appEntry should reference coreUtil");

    assert_eq!(cross_package.to_symbol, Some(core_util.id));
    assert_eq!(cross_package.confidence, engine_core::Confidence::High);
}

#[test]
fn generated_files_are_still_indexed_for_navigation() {
    let dir = fixture_path("generated-files");
    require_fixture!(dir);
    let (index, _repo) = index_fixture(&dir);

    // schema.pb.go matches the default generated_path_globs, but this
    // crate's job is indexing for navigation — exclusion from evidence
    // relevance is engine-evidence's concern, not ours. Its symbol must
    // still be findable.
    let generated = index.search_symbols("GeneratedGetter", 10).unwrap();
    assert!(
        generated.iter().any(|s| s.name == "GeneratedGetter"),
        "generated file's symbols should still be indexed"
    );
    let real = index.search_symbols("handWritten", 10).unwrap();
    assert!(real.iter().any(|s| s.name == "handWritten"));
}

#[test]
fn incremental_reindex_skips_unchanged_files_and_preserves_symbol_ids() {
    let scratch = tempfile::tempdir().unwrap();
    std::fs::write(
        scratch.path().join("a.ts"),
        "export function keep(): number {\n  return 1;\n}\n",
    )
    .unwrap();
    std::fs::write(
        scratch.path().join("b.ts"),
        "export function other(): number {\n  return 2;\n}\n",
    )
    .unwrap();

    let repo = RepoHandle::open(scratch.path()).unwrap();
    let config = BoreholeConfig::default();
    let mut index = Index::open_in_memory().unwrap();
    let first = index.full_reindex(&repo, &config, |_| {}).unwrap();
    assert_eq!(first.files_indexed, 2);

    let keep_before = index
        .search_symbols("keep", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "keep")
        .expect("keep symbol");

    // Touch only b.ts (content change, not just mtime) — a.ts is
    // untouched on disk.
    std::fs::write(
        scratch.path().join("b.ts"),
        "export function other(): number {\n  return 3;\n}\n",
    )
    .unwrap();

    let second = index.full_reindex(&repo, &config, |_| {}).unwrap();
    // Only the changed file should have been re-extracted.
    assert_eq!(
        second.files_indexed, 1,
        "only b.ts changed; a.ts should have been skipped entirely"
    );

    let keep_after = index
        .search_symbols("keep", 10)
        .unwrap()
        .into_iter()
        .find(|s| s.name == "keep")
        .expect("keep symbol still present");
    assert_eq!(
        keep_before.id, keep_after.id,
        "SymbolId for an untouched file's symbol must survive reindex"
    );
}

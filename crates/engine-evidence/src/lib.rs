//! Evidence synthesis: everything `engine-index`, `engine-graph`, and
//! `engine-git` know about a symbol, aggregated into the report types the
//! CLI and desktop UI both render directly. This is the brief's "Evidence
//! System" (section 20) and "Blast Radius" (section 14) made concrete.
//!
//! HARD RULE for whoever implements this crate's bodies: every field below
//! that isn't a raw count must carry `Vec<EvidenceRef>` citing exactly what
//! was inspected to produce it. If the underlying data is missing or
//! ambiguous, return a `Confidence::Low` / empty-evidence result and say so
//! — never fabricate a plausible-sounding summary. `Insufficient evidence`
//! is a correct answer (brief section 20).
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"). Both
//! `crates/cli` and `src-tauri` depend on this crate and nothing lower —
//! that's what keeps CLI/desktop output identical for the same query.

use engine_core::{Confidence, EvidenceRef, Reference, RepoPath, RiskTier, Span, Symbol, SymbolId};
use engine_git::{CoChangeEdge, CommitInfo};
use engine_graph::DependencyGraph;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Default ceiling applied to `symbol_profile`'s git-history walk when the
/// user hasn't set an explicit `BoreholeConfig::git_history_limit`. See its
/// use site for the full reasoning — this exists so the *default*
/// (unbounded) configuration still can't be turned into unbounded work by
/// a repository that engineers one file touched by an enormous number of
/// commits.
const SYMBOL_HISTORY_SAFETY_LIMIT: u32 = 2000;

#[derive(Debug, thiserror::Error)]
pub enum EvidenceError {
    #[error(transparent)]
    Index(#[from] engine_index::IndexError),
    #[error(transparent)]
    Graph(#[from] engine_graph::GraphError),
    #[error(transparent)]
    Git(#[from] engine_git::GitError),
    #[error("symbol {0} not found in index")]
    SymbolNotFound(SymbolId),
}

pub type Result<T> = std::result::Result<T, EvidenceError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TestRef {
    pub file: RepoPath,
    /// Why this file was flagged as related — brief section 18 explicitly
    /// requires distinguishing "related test" from "coverage data", and
    /// this is how: it's always a heuristic reason, never a claim of
    /// measured coverage.
    pub reason: TestMatchReason,
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TestMatchReason {
    NamingConvention,    // e.g. session.ts -> session.test.ts
    DirectImport,        // test file imports/references the symbol directly
    DirectoryConvention, // lives under tests/, __tests__/, spec/
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigRef {
    pub file: RepoPath,
    pub key: Option<String>, // e.g. an env var name; None for whole-file config touches
    pub evidence: EvidenceRef,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolProfile {
    pub symbol: Symbol,
    pub direct_callers: u32,
    pub indirect_callers: u32,
    pub tests: Vec<TestRef>,
    pub config: Vec<ConfigRef>,
    pub introduced: Option<CommitInfo>,
    pub modification_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskReason {
    pub text: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlastRadius {
    pub target: Symbol,
    pub direct_callers: u32,
    pub indirect_callers: u32,
    pub test_suites: u32,
    pub is_public_api: bool,
    pub config_files: Vec<ConfigRef>,
    pub risk: RiskTier,
    pub risk_reasons: Vec<RiskReason>,
    pub caller_graph: DependencyGraph,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReadingListItem {
    pub symbol_or_file: String,
    pub why: String,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalWarning {
    pub text: String,
    pub commits: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BeforeYouChangeThisReport {
    pub target: Symbol,
    pub reading_list: Vec<ReadingListItem>,
    pub blast_radius: BlastRadius,
    pub historical_warnings: Vec<HistoricalWarning>,
    /// Overall confidence in this report — the minimum confidence across
    /// every claim it makes, so the UI can show one honest headline number
    /// instead of burying caveats per-field.
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEvolution {
    pub target_path_or_symbol: String,
    pub introduced: Option<CommitInfo>,
    pub major_refactors: Vec<CommitInfo>,
    pub all_commits: Vec<CommitInfo>,
    pub authors: Vec<String>,
    pub co_changing_files: Vec<CoChangeEdge>,
    pub confidence: Confidence,
}

/// Ties an `Index`, an optional `GitAnalyzer`, and a `RepoHandle` together
/// into the synthesis operations the CLI/desktop layers call. Borrowed, not
/// owned — callers control the lifetime of the underlying index/git
/// handles (e.g. the Tauri app keeps one `EvidenceEngine` alive per open
/// repo tab).
///
/// `git` is `None` when the opened directory has no `.git` (a plain folder,
/// or a git repository so fresh it has no commits yet). This is a
/// deliberate, honest degraded mode, not an error: every git-derived field
/// this crate produces (`introduced`, `code_evolution`, historical
/// warnings, temporal-coupling-based risk reasons) is simply omitted or
/// returned empty with `Confidence::Low` when `git` is `None`, per the
/// "Insufficient evidence is a correct answer" rule — see brief section 20
/// and this crate's top-level doc comment. Callers must never synthesize a
/// git-shaped answer (a fake "introduced" commit, an empty-but-labeled-High
/// history) when git access wasn't available.
pub struct EvidenceEngine<'a> {
    pub index: &'a engine_index::Index,
    pub git: Option<&'a engine_git::GitAnalyzer>,
    pub repo: &'a engine_core::RepoHandle,
    pub config: &'a engine_core::BoreholeConfig,
}

impl<'a> EvidenceEngine<'a> {
    pub fn new(
        index: &'a engine_index::Index,
        git: Option<&'a engine_git::GitAnalyzer>,
        repo: &'a engine_core::RepoHandle,
        config: &'a engine_core::BoreholeConfig,
    ) -> Self {
        Self {
            index,
            git,
            repo,
            config,
        }
    }

    pub fn symbol_profile(&self, id: SymbolId) -> Result<SymbolProfile> {
        let symbol = self.require_symbol(id)?;
        let (direct_callers, indirect_callers, _graph) = self.caller_counts(id)?;
        let tests = self.related_tests(id)?;
        let config = self.config_touches(id)?;

        // `BoreholeConfig::git_history_limit` exists to bound *repository-wide*
        // history mining and defaults to `None` (unbounded) — using it alone
        // here would leave the default configuration exposed to a repo that
        // crafts one file touched by an enormous number of commits (e.g. a
        // lockfile or CHANGELOG bumped every commit), which is unbounded work
        // triggered by simply viewing that file's profile. `.or(...)` layers
        // an independent safety ceiling under the user's config: an explicit
        // limit the user set is always respected as-is (even one larger than
        // the ceiling), but the *default* unbounded case still gets bounded.
        let limit = self
            .config
            .git_history_limit
            .or(Some(SYMBOL_HISTORY_SAFETY_LIMIT));
        let (introduced, modification_count) = match self.git {
            Some(git) => {
                let history = git.file_history(&symbol.file, limit)?;
                // If the walk came back exactly at the cap, `history.last()`
                // might not be the file's *true* first commit — reporting it
                // as `introduced` would be a specific factual claim we can't
                // actually back. Leaving it `None` in that edge case is the
                // honest choice; `modification_count` still reports the
                // (possibly-partial) count, which is true as a lower bound.
                let hit_cap = limit.is_some_and(|l| history.len() as u32 == l);
                let introduced = if hit_cap {
                    None
                } else {
                    history.last().cloned()
                };
                let modification_count = history.len() as u32;
                (introduced, modification_count)
            }
            None => (None, 0),
        };

        Ok(SymbolProfile {
            symbol,
            direct_callers,
            indirect_callers,
            tests,
            config,
            introduced,
            modification_count,
        })
    }

    pub fn blast_radius(&self, id: SymbolId) -> Result<BlastRadius> {
        let symbol = self.require_symbol(id)?;
        let (direct_callers, indirect_callers, caller_graph) = self.caller_counts(id)?;
        let tests = self.related_tests(id)?;
        let test_suites = tests.len() as u32;
        // `is_exported` is the honest signal available from a single-file
        // parse: a proxy for "public API", not a full cross-file/
        // cross-package reachability analysis (that would need per-language
        // visibility rules — e.g. Rust's `pub(crate)` vs `pub`, JS default
        // vs named exports vs re-exports — which is out of scope for v1).
        let is_public_api = symbol.is_exported;
        let config_files = self.config_touches(id)?;
        let refs_to = self.index.references_to(id)?;
        let (risk, risk_reasons) = Self::score_risk(
            direct_callers,
            indirect_callers,
            is_public_api,
            test_suites,
            &refs_to,
        );

        Ok(BlastRadius {
            target: symbol,
            direct_callers,
            indirect_callers,
            test_suites,
            is_public_api,
            config_files,
            risk,
            risk_reasons,
            caller_graph,
        })
    }

    pub fn before_you_change_this(&self, id: SymbolId) -> Result<BeforeYouChangeThisReport> {
        let symbol = self.require_symbol(id)?;
        let mut reading_list = Vec::new();
        let mut confidences: Vec<Confidence> = Vec::new();

        // (a) up to 3 direct callers.
        let refs_to = self.index.references_to(id)?;
        let mut seen_callers: HashSet<SymbolId> = HashSet::new();
        for r in &refs_to {
            if seen_callers.len() >= 3 {
                break;
            }
            let Some(from_id) = r.from_symbol else {
                continue;
            };
            if !seen_callers.insert(from_id) {
                continue;
            }
            if let Some(caller) = self.index.find_symbol(from_id)? {
                reading_list.push(ReadingListItem {
                    symbol_or_file: caller.qualified_name.clone(),
                    why: "calls this directly".to_string(),
                    evidence: vec![EvidenceRef::Reference {
                        file: r.from_file.clone(),
                        span: r.from_span,
                    }],
                });
                confidences.push(r.confidence);
            }
        }

        // (b) up to 3 callees/dependencies.
        let refs_from = self.index.references_from(id)?;
        let mut seen_callees: HashSet<SymbolId> = HashSet::new();
        for r in &refs_from {
            if seen_callees.len() >= 3 {
                break;
            }
            let Some(to_id) = r.to_symbol else {
                continue;
            };
            if !seen_callees.insert(to_id) {
                continue;
            }
            if let Some(callee) = self.index.find_symbol(to_id)? {
                reading_list.push(ReadingListItem {
                    symbol_or_file: callee.qualified_name.clone(),
                    why: "this depends on it".to_string(),
                    evidence: vec![EvidenceRef::Reference {
                        file: r.from_file.clone(),
                        span: r.from_span,
                    }],
                });
                confidences.push(r.confidence);
            }
        }

        // (c) related test files.
        let tests = self.related_tests(id)?;
        for t in &tests {
            reading_list.push(ReadingListItem {
                symbol_or_file: t.file.to_string(),
                why: "covers this behavior".to_string(),
                evidence: vec![EvidenceRef::TestFile {
                    file: t.file.clone(),
                }],
            });
            confidences.push(t.confidence);
        }

        let blast_radius = self.blast_radius(id)?;

        // Historical warnings reuse the same major-refactor heuristic as
        // `code_evolution` — only populated when git is available, per the
        // hard rule that git-derived fields never get faked when `git` is
        // `None`.
        let mut historical_warnings = Vec::new();
        if let Some(git) = self.git {
            let commits = git.file_history(&symbol.file, self.config.git_history_limit)?;
            let majors = Self::detect_major_refactors(&commits);
            if !majors.is_empty() {
                // The heuristic itself is not ground truth (see
                // `detect_major_refactors`'s doc comment) — folding this
                // into the overall confidence keeps that honest.
                confidences.push(Confidence::Medium);
            }
            for c in &majors {
                historical_warnings.push(HistoricalWarning {
                    text: format!(
                        "commit {} (\"{}\") touched {} file(s) in the same change and looks like a major refactor of {} — review it before assuming today's behavior matches what came before",
                        short_sha(&c.sha),
                        c.summary,
                        c.files_changed,
                        symbol.file
                    ),
                    commits: vec![EvidenceRef::Commit {
                        sha: c.sha.clone(),
                        summary: c.summary.clone(),
                    }],
                });
            }
        }

        // "Minimum confidence across every confidence-bearing input" — since
        // `Confidence`'s derived `Ord` runs High < Medium < Low (declaration
        // order), the *worst* (least trustworthy) input is the Ord-maximum,
        // so folding with `max` is exactly "never overclaim past your
        // weakest citation." When no confidence-bearing input was actually
        // used (empty reading list, no git), there is nothing to be
        // confident *or* unconfident about — defaulting to `Medium` here
        // rather than `High` is the deliberate "don't overclaim" call for
        // that edge case; see the report-back notes for this judgment.
        let confidence = confidences.into_iter().max().unwrap_or(Confidence::Medium);

        Ok(BeforeYouChangeThisReport {
            target: symbol,
            reading_list,
            blast_radius,
            historical_warnings,
            confidence,
        })
    }

    pub fn code_evolution(&self, path: &RepoPath) -> Result<CodeEvolution> {
        let Some(git) = self.git else {
            // Honest degraded state, not an error — see this crate's
            // top-level doc comment and `EvidenceEngine::git`'s doc
            // comment.
            return Ok(CodeEvolution {
                target_path_or_symbol: path.to_string(),
                introduced: None,
                major_refactors: Vec::new(),
                all_commits: Vec::new(),
                authors: Vec::new(),
                co_changing_files: Vec::new(),
                confidence: Confidence::Low,
            });
        };

        let all_commits = git.file_history(path, self.config.git_history_limit)?;
        let introduced = all_commits.last().cloned();
        let major_refactors = Self::detect_major_refactors(&all_commits);

        let mut authors = Vec::new();
        for c in &all_commits {
            if !authors.contains(&c.author_name) {
                authors.push(c.author_name.clone());
            }
        }

        let co_changing_files = git.temporal_coupling(path, self.config.git_history_limit)?;

        // `CommitInfo` carries no rename-ambiguity signal on its own (see
        // `GitAnalyzer::symbol_history`'s doc comment for the one place
        // that *does* track it, via `SymbolHistory::confidence`) — we have
        // no concrete signal to justify `High` here, so `Medium` is the
        // honest default whenever there's at least some real history, and
        // `Low` when there's none at all to reason about.
        let confidence = if all_commits.is_empty() {
            Confidence::Low
        } else {
            Confidence::Medium
        };

        Ok(CodeEvolution {
            target_path_or_symbol: path.to_string(),
            introduced,
            major_refactors,
            all_commits,
            authors,
            co_changing_files,
            confidence,
        })
    }

    pub fn related_tests(&self, id: SymbolId) -> Result<Vec<TestRef>> {
        let symbol = self.require_symbol(id)?;
        let all_files = self.index.all_files()?;
        let sym_path = Path::new(symbol.file.as_str());
        let stem = sym_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default();
        let sym_dir = sym_path.parent();
        let sym_top = first_component(sym_path);

        let mut best: HashMap<RepoPath, (TestMatchReason, Confidence)> = HashMap::new();

        for f in &all_files {
            if f == &symbol.file {
                continue;
            }
            let fpath = Path::new(f.as_str());

            // Naming convention: same directory, `<stem>.test.*` etc.
            if fpath.parent() == sym_dir && Self::matches_test_naming(fpath, stem) {
                Self::upsert_test_match(
                    &mut best,
                    f,
                    TestMatchReason::NamingConvention,
                    Confidence::Medium,
                );
            }

            // Directory convention: under tests/__tests__/spec/test, same
            // top-level package as the symbol's own file.
            if first_component(fpath) == sym_top && Self::in_test_directory(fpath) {
                Self::upsert_test_match(
                    &mut best,
                    f,
                    TestMatchReason::DirectoryConvention,
                    Confidence::Medium,
                );
            }
        }

        // Direct import: a test-shaped file with a resolved reference to
        // this exact symbol — the strongest signal, since it's confirmed
        // usage rather than a naming heuristic.
        let refs_to = self.index.references_to(id)?;
        for r in &refs_to {
            let fpath = Path::new(r.from_file.as_str());
            if Self::looks_like_test_file(fpath) {
                Self::upsert_test_match(
                    &mut best,
                    &r.from_file,
                    TestMatchReason::DirectImport,
                    Confidence::High,
                );
            }
        }

        let mut out: Vec<TestRef> = best
            .into_iter()
            .map(|(file, (reason, confidence))| TestRef {
                file,
                reason,
                confidence,
            })
            .collect();
        out.sort_by(|a, b| a.file.cmp(&b.file));
        Ok(out)
    }

    pub fn config_touches(&self, id: SymbolId) -> Result<Vec<ConfigRef>> {
        let symbol = self.require_symbol(id)?;

        // A file that can't be resolved/read on disk (e.g. deleted since
        // the last index, or a path that somehow escapes the repo root) is
        // "insufficient evidence," not an error — see this crate's
        // top-level hard rule.
        let Ok(abs_path) = self.repo.resolve(&symbol.file) else {
            return Ok(Vec::new());
        };
        let Ok(source) = std::fs::read_to_string(&abs_path) else {
            return Ok(Vec::new());
        };

        // Env-var access patterns per language. Every pattern's capture
        // group 1 is the variable *name* only — never a value, and never a
        // hardcoded fallback passed to e.g. `.unwrap_or(...)`, since none of
        // these patterns reach outside the access expression itself. This
        // is what keeps `config_touches` safe under PRIVACY.md's "Secrets"
        // rule: it scans source text for the literal identifier used in the
        // access expression, never reads or reports a resolved value.
        let patterns: [Regex; 6] = [
            Regex::new(r"process\.env\.([A-Za-z_][A-Za-z0-9_]*)").expect("valid regex"),
            Regex::new(r#"process\.env\[\s*['"]([A-Za-z_][A-Za-z0-9_]*)['"]\s*\]"#)
                .expect("valid regex"),
            Regex::new(r#"os\.environ(?:\.get)?\[\s*['"]([A-Za-z_][A-Za-z0-9_]*)['"]\s*\]"#)
                .expect("valid regex"),
            Regex::new(r#"os\.getenv\(\s*['"]([A-Za-z_][A-Za-z0-9_]*)['"]"#).expect("valid regex"),
            Regex::new(r#"std::env::var\(\s*"([A-Za-z_][A-Za-z0-9_]*)"\s*\)"#)
                .expect("valid regex"),
            Regex::new(r#"os\.Getenv\(\s*"([A-Za-z_][A-Za-z0-9_]*)"\s*\)"#).expect("valid regex"),
        ];

        let line_starts = Self::line_starts(&source);
        let mut seen_keys: HashSet<String> = HashSet::new();
        let mut out = Vec::new();

        for re in &patterns {
            for caps in re.captures_iter(&source) {
                let whole = caps.get(0).expect("index 0 is always present");
                let key_match = caps.get(1).expect("every pattern has capture group 1");
                let (start_line, start_col) = Self::line_col(&line_starts, whole.start());
                let (end_line, end_col) = Self::line_col(&line_starts, whole.end());

                // Only matches falling within the symbol's own span count
                // as this symbol's config touches.
                if start_line < symbol.span.start_line || start_line > symbol.span.end_line {
                    continue;
                }

                let key = key_match.as_str().to_string();
                // Dedupe by variable name — a symbol reading the same env
                // var three times gets one cited touch, not three
                // identical-looking rows.
                if !seen_keys.insert(key.clone()) {
                    continue;
                }

                out.push(ConfigRef {
                    file: symbol.file.clone(),
                    key: Some(key),
                    evidence: EvidenceRef::Reference {
                        file: symbol.file.clone(),
                        span: Span {
                            start_line,
                            start_col,
                            end_line,
                            end_col,
                            start_byte: whole.start() as u32,
                            end_byte: whole.end() as u32,
                        },
                    },
                });
            }
        }

        out.sort_by(|a, b| a.key.cmp(&b.key));
        Ok(out)
    }

    // -------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------

    fn require_symbol(&self, id: SymbolId) -> Result<Symbol> {
        self.index
            .find_symbol(id)?
            .ok_or(EvidenceError::SymbolNotFound(id))
    }

    /// `direct_callers` = every resolved reference row pointing at `id`
    /// (call sites, including ones with no containing symbol, e.g.
    /// module-level code — "direct caller" means "call site," not "named
    /// calling symbol"). `indirect_callers` = every symbol reachable in
    /// `expand_callers`'s bounded-depth traversal beyond depth 1, i.e. the
    /// traversal's total node count minus the root minus the count of
    /// *distinct* symbols that call `id` directly. Also returns the graph
    /// itself so `blast_radius` doesn't have to traverse twice.
    ///
    /// Note on inheritance: `expand_callers`/`references_to` don't filter
    /// by `ReferenceKind`, so an `Extends`/`Implements` reference (a
    /// subclass of `id`) is already a row in `references_to(id)` and is
    /// already walked by `expand_callers` — see `blast_radius`'s doc
    /// comment on the `subclasses_of` decision.
    fn caller_counts(&self, id: SymbolId) -> Result<(u32, u32, DependencyGraph)> {
        let refs_to = self.index.references_to(id)?;
        let direct_callers = refs_to.len() as u32;
        let distinct_direct: HashSet<SymbolId> =
            refs_to.iter().filter_map(|r| r.from_symbol).collect();

        let graph = engine_graph::expand_callers(self.index, id, self.config.max_graph_depth)?;
        let reachable_callers = (graph.nodes.len() as u32).saturating_sub(1); // exclude root
        let indirect_callers = reachable_callers.saturating_sub(distinct_direct.len() as u32);

        Ok((direct_callers, indirect_callers, graph))
    }

    /// Risk scoring rules (documented plainly, deliberately simple — see
    /// this crate's report-back notes for the reasoning):
    ///
    /// - Start at `Low`.
    /// - `direct_callers >= 1 && is_public_api` -> at least `Medium`
    ///   (exported symbols with any caller may have consumers outside this
    ///   repo that the engine can't see at all).
    /// - `direct_callers >= 5 || indirect_callers >= 15` -> at least `High`
    ///   (a wide blast radius, regardless of export status).
    /// - `test_suites == 0 && direct_callers > 0` -> bump one tier from
    ///   wherever the above landed (untested code with real callers is
    ///   riskier to touch than the same code with a test net under it).
    ///   This reason cites no `EvidenceRef` — the claim *is* the absence of
    ///   evidence, and there is nothing honest to cite for "we didn't find
    ///   a test file."
    ///
    /// Every tier bump gets a `RiskReason` citing up to 3 sampled caller
    /// references as evidence where evidence exists.
    fn score_risk(
        direct_callers: u32,
        indirect_callers: u32,
        is_public_api: bool,
        test_suites: u32,
        refs_to: &[Reference],
    ) -> (RiskTier, Vec<RiskReason>) {
        let mut tier = RiskTier::Low;
        let mut reasons = Vec::new();
        let sample_evidence = || -> Vec<EvidenceRef> {
            refs_to
                .iter()
                .take(3)
                .map(|r| EvidenceRef::Reference {
                    file: r.from_file.clone(),
                    span: r.from_span,
                })
                .collect()
        };

        if direct_callers >= 1 && is_public_api {
            tier = tier.max(RiskTier::Medium);
            reasons.push(RiskReason {
                text: format!(
                    "{direct_callers} caller(s) depend on this symbol and it is part of the public API (exported) — external code the engine can't see may also rely on its current behavior"
                ),
                evidence: sample_evidence(),
            });
        }

        if direct_callers >= 5 || indirect_callers >= 15 {
            tier = tier.max(RiskTier::High);
            reasons.push(RiskReason {
                text: format!(
                    "wide blast radius: {direct_callers} direct caller(s) and {indirect_callers} indirect caller(s) reachable through the call graph"
                ),
                evidence: sample_evidence(),
            });
        }

        if test_suites == 0 && direct_callers > 0 {
            tier = bump_tier(tier);
            reasons.push(RiskReason {
                text: "no related test file was found for this symbol, even though it has callers — a change here has no automated safety net".to_string(),
                evidence: Vec::new(),
            });
        }

        (tier, reasons)
    }

    /// "Major refactor" heuristic — `CommitInfo` has no lines-changed
    /// count, only `files_changed`, so this is necessarily a heuristic, not
    /// ground truth: a commit counts if it touched at least 5 files in one
    /// change (a broad, multi-file change), OR its summary contains a
    /// refactor-signal keyword (case-insensitive: "refactor", "rewrite",
    /// "restructure", "redesign"). A commit that touches one file and says
    /// "quick fix" will never be flagged by this heuristic, and a genuinely
    /// large single-file rewrite won't be flagged by the file-count half of
    /// it either — that's the honest limit of what's available from
    /// `engine-git`'s current contract, not a claim of semantic diff
    /// analysis.
    fn detect_major_refactors(commits: &[CommitInfo]) -> Vec<CommitInfo> {
        const REFACTOR_KEYWORDS: [&str; 4] = ["refactor", "rewrite", "restructure", "redesign"];
        commits
            .iter()
            .filter(|c| {
                c.files_changed >= 5 || {
                    let lower = c.summary.to_lowercase();
                    REFACTOR_KEYWORDS.iter().any(|k| lower.contains(k))
                }
            })
            .cloned()
            .collect()
    }

    /// `<stem>.test.*`, `<stem>.spec.*`, `<stem>_test.*`, `test_<stem>.*`.
    fn matches_test_naming(path: &Path, stem: &str) -> bool {
        if stem.is_empty() {
            return false;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        let dotted = [
            format!("{stem}.test."),
            format!("{stem}.spec."),
            format!("{stem}_test."),
        ];
        if dotted.iter().any(|p| name.starts_with(p.as_str())) {
            return true;
        }
        name.starts_with(&format!("test_{stem}."))
    }

    /// Any path segment (excluding the filename itself) named `tests`,
    /// `__tests__`, `spec`, or `test`.
    fn in_test_directory(path: &Path) -> bool {
        path.parent()
            .into_iter()
            .flat_map(|p| p.components())
            .any(|c| {
                matches!(
                    c.as_os_str().to_str(),
                    Some("tests" | "__tests__" | "spec" | "test")
                )
            })
    }

    /// Generic "does this look like a test file at all" check, used only
    /// for the direct-import heuristic — deliberately looser than
    /// [`Self::matches_test_naming`] since the confirming evidence here is
    /// the resolved reference itself, not the name match.
    fn looks_like_test_file(path: &Path) -> bool {
        if Self::in_test_directory(path) {
            return true;
        }
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            return false;
        };
        let lower = name.to_lowercase();
        lower.contains(".test.")
            || lower.contains(".spec.")
            || lower.contains("_test.")
            || lower.starts_with("test_")
    }

    /// Insert a (reason, confidence) for `file`, keeping whichever is
    /// strongest when a file already has an entry: `DirectImport` beats the
    /// naming/directory heuristics, and within an equal-strength tie the
    /// better (lower-Ord) `Confidence` wins.
    fn upsert_test_match(
        best: &mut HashMap<RepoPath, (TestMatchReason, Confidence)>,
        file: &RepoPath,
        reason: TestMatchReason,
        confidence: Confidence,
    ) {
        match best.get(file) {
            None => {
                best.insert(file.clone(), (reason, confidence));
            }
            Some((existing_reason, existing_confidence)) => {
                let existing_rank = reason_rank(*existing_reason);
                let new_rank = reason_rank(reason);
                if new_rank > existing_rank
                    || (new_rank == existing_rank && confidence < *existing_confidence)
                {
                    best.insert(file.clone(), (reason, confidence));
                }
            }
        }
    }

    fn line_starts(source: &str) -> Vec<usize> {
        let mut starts = vec![0usize];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i + 1);
            }
        }
        starts
    }

    /// 1-indexed (line, col) for a 0-indexed byte offset, given the file's
    /// precomputed line-start table.
    fn line_col(line_starts: &[usize], byte_offset: usize) -> (u32, u32) {
        let line_idx = match line_starts.binary_search(&byte_offset) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };
        let col = (byte_offset - line_starts[line_idx]) as u32 + 1;
        (line_idx as u32 + 1, col)
    }
}

fn reason_rank(reason: TestMatchReason) -> u8 {
    match reason {
        TestMatchReason::DirectImport => 2,
        TestMatchReason::NamingConvention => 1,
        TestMatchReason::DirectoryConvention => 0,
    }
}

fn bump_tier(tier: RiskTier) -> RiskTier {
    match tier {
        RiskTier::Low => RiskTier::Medium,
        RiskTier::Medium => RiskTier::High,
        RiskTier::High => RiskTier::Critical,
        RiskTier::Critical => RiskTier::Critical,
    }
}

fn short_sha(sha: &str) -> &str {
    &sha[..7.min(sha.len())]
}

/// First path component as an owned string, for cheap `Path` comparisons
/// that don't need to fight component lifetimes.
fn first_component(path: &Path) -> Option<String> {
    path.components()
        .next()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
}

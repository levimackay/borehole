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

use engine_core::{Confidence, EvidenceRef, RepoPath, RiskTier, Symbol, SymbolId};
use engine_git::{CoChangeEdge, CommitInfo};
use engine_graph::DependencyGraph;
use serde::{Deserialize, Serialize};

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
    NamingConvention, // e.g. session.ts -> session.test.ts
    DirectImport,     // test file imports/references the symbol directly
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

/// Ties an `Index`, a `GitAnalyzer`, and a `RepoHandle` together into the
/// synthesis operations the CLI/desktop layers call. Borrowed, not owned —
/// callers control the lifetime of the underlying index/git handles (e.g.
/// the Tauri app keeps one `EvidenceEngine` alive per open repo tab).
pub struct EvidenceEngine<'a> {
    pub index: &'a engine_index::Index,
    pub git: &'a engine_git::GitAnalyzer,
    pub repo: &'a engine_core::RepoHandle,
    pub config: &'a engine_core::BoreholeConfig,
}

impl<'a> EvidenceEngine<'a> {
    pub fn new(
        index: &'a engine_index::Index,
        git: &'a engine_git::GitAnalyzer,
        repo: &'a engine_core::RepoHandle,
        config: &'a engine_core::BoreholeConfig,
    ) -> Self {
        Self { index, git, repo, config }
    }

    pub fn symbol_profile(&self, id: SymbolId) -> Result<SymbolProfile> {
        let _ = id;
        todo!("engine-evidence: symbol_profile — see docs/ARCHITECTURE.md")
    }

    pub fn blast_radius(&self, id: SymbolId) -> Result<BlastRadius> {
        let _ = id;
        todo!("engine-evidence: blast_radius — risk scoring rules in docs/ARCHITECTURE.md")
    }

    pub fn before_you_change_this(&self, id: SymbolId) -> Result<BeforeYouChangeThisReport> {
        let _ = id;
        todo!("engine-evidence: before_you_change_this — see docs/ARCHITECTURE.md")
    }

    pub fn code_evolution(&self, path: &RepoPath) -> Result<CodeEvolution> {
        let _ = path;
        todo!("engine-evidence: code_evolution — see docs/ARCHITECTURE.md")
    }

    pub fn related_tests(&self, id: SymbolId) -> Result<Vec<TestRef>> {
        let _ = id;
        todo!("engine-evidence: related_tests — naming + directory + import heuristics")
    }

    pub fn config_touches(&self, id: SymbolId) -> Result<Vec<ConfigRef>> {
        let _ = id;
        todo!("engine-evidence: config_touches — env var + config file heuristics, never leak secret values")
    }
}

//! Git history mining via git2 (libgit2 bindings) — never the `git` binary
//! (brief section 23: shelling out to a repo-controlled `git` wrapper or
//! hook script is a command-injection surface; git2 talks to the on-disk
//! object database directly and has no shell in the loop).
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"). Consumed
//! by `engine-evidence`.

use engine_core::{Confidence, RepoPath, Span};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error(transparent)]
    Git2(#[from] git2::Error),
    #[error("{0} is not a git repository")]
    NotAGitRepo(String),
}

pub type Result<T> = std::result::Result<T, GitError>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitInfo {
    pub sha: String,
    pub author_name: String,
    pub author_email: String,
    pub timestamp: i64, // unix seconds, UTC
    pub summary: String,
    pub files_changed: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolHistory {
    pub commits: Vec<CommitInfo>,
    /// `High` when every commit was matched by exact line-range
    /// intersection against the symbol's current span across renames;
    /// `Low` when history had to fall back to whole-file attribution
    /// because line tracking broke (e.g. the symbol moved across files, or
    /// a rename couldn't be followed past a similarity threshold). Never
    /// silently claim `High` confidence on a heuristic match — see brief
    /// section 16: "do not pretend semantic tracking is perfect."
    pub confidence: Confidence,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoChangeEdge {
    pub file: RepoPath,
    pub co_change_count: u32,
    pub total_commits_either_file: u32,
    pub ratio: f32, // co_change_count / total_commits_either_file, in [0,1]
}

/// Read-only handle onto one repository's git history. Opens the on-disk
/// `.git` directory via git2; never fetches, never writes (brief section
/// 26: "do not automatically fetch remote repositories").
pub struct GitAnalyzer {
    pub(crate) repo: git2::Repository,
}

impl GitAnalyzer {
    pub fn open(root: &Path) -> Result<Self> {
        let repo = git2::Repository::open(root)
            .map_err(|_| GitError::NotAGitRepo(root.display().to_string()))?;
        Ok(Self { repo })
    }

    /// Full commit history touching `path`, following renames. `limit`
    /// caps the number of commits walked (brief section 24: bounded work
    /// on huge histories), most-recent first.
    pub fn file_history(&self, path: &RepoPath, limit: Option<u32>) -> Result<Vec<CommitInfo>> {
        let _ = (path, limit);
        todo!("engine-git: file_history — walk revwalk filtered to path, following renames")
    }

    /// Best-effort history scoped to one symbol's byte span within its
    /// current file, by intersecting each commit's diff hunks against the
    /// span. See [`SymbolHistory::confidence`] for the honesty contract.
    pub fn symbol_history(
        &self,
        path: &RepoPath,
        span: Span,
        limit: Option<u32>,
    ) -> Result<SymbolHistory> {
        let _ = (path, span, limit);
        todo!("engine-git: symbol_history — diff-hunk intersection over file_history")
    }

    /// Files that changed together with `path` across history — a raw
    /// correlation signal (brief section 17), not a claim of architectural
    /// dependency; that framing belongs in `engine-evidence`'s presentation
    /// layer, not here.
    pub fn temporal_coupling(&self, path: &RepoPath, limit: Option<u32>) -> Result<Vec<CoChangeEdge>> {
        let _ = (path, limit);
        todo!("engine-git: temporal_coupling — co-occurrence across commit file-lists")
    }

    pub fn introduced_commit(&self, path: &RepoPath) -> Result<Option<CommitInfo>> {
        let _ = path;
        todo!("engine-git: introduced_commit — oldest commit in file_history")
    }
}

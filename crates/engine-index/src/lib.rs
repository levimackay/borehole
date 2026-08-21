//! SQLite-backed symbol/reference index with incremental re-indexing.
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"). Consumed
//! by `engine-graph` and `engine-evidence`.

use engine_core::{BoreholeConfig, RepoHandle, RepoPath, Symbol, SymbolId};
use serde::{Deserialize, Serialize};
use std::path::Path;

pub mod resolve;
pub mod schema;

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Parse(#[from] engine_parse::ParseError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

pub type Result<T> = std::result::Result<T, IndexError>;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct IndexProgress {
    pub files_done: u32,
    pub files_total: u32,
    pub current_file: Option<u32>, // interned file id, avoids allocating a string per event
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexSummary {
    pub files_indexed: u32,
    pub files_skipped_unsupported: u32,
    pub files_skipped_too_large: u32,
    pub symbols_indexed: u32,
    pub references_indexed: u32,
    pub references_resolved: u32,
    pub duration_ms: u64,
}

/// The persistent, queryable index for one repository. Backed by a single
/// SQLite file at `<repo_root>/.borehole/index.db` (or `:memory:` for
/// tests/fixtures). Holds the only `rusqlite::Connection` in the process
/// for a given repo — callers never touch SQL directly.
pub struct Index {
    pub(crate) conn: rusqlite::Connection,
}

impl Index {
    pub fn open(db_path: &Path) -> Result<Self> {
        let conn = rusqlite::Connection::open(db_path)?;
        let index = Self { conn };
        index.ensure_schema()?;
        Ok(index)
    }

    pub fn open_in_memory() -> Result<Self> {
        let conn = rusqlite::Connection::open_in_memory()?;
        let index = Self { conn };
        index.ensure_schema()?;
        Ok(index)
    }

    fn ensure_schema(&self) -> Result<()> {
        schema::create_schema(&self.conn)?;
        Ok(())
    }

    /// Full (re)index of the repository. Existing rows for files whose
    /// content hash hasn't changed since the last index are left untouched
    /// (their `SymbolId`s are preserved); changed/new files are
    /// re-extracted; files no longer present are removed. `on_progress` is
    /// called after each file so callers (CLI progress bar, Tauri event
    /// emitter) can render it without this crate knowing about either.
    pub fn full_reindex(
        &mut self,
        repo: &RepoHandle,
        config: &BoreholeConfig,
        on_progress: impl FnMut(IndexProgress),
    ) -> Result<IndexSummary> {
        let _ = on_progress;
        indexing_todo(repo, config)
    }

    pub fn find_symbol(&self, id: SymbolId) -> Result<Option<Symbol>> {
        let _ = id;
        todo!("engine-index: find_symbol — see docs/ARCHITECTURE.md")
    }

    pub fn search_symbols(&self, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        let _ = (query, limit);
        todo!("engine-index: search_symbols — see docs/ARCHITECTURE.md")
    }

    pub fn symbols_in_file(&self, file: &RepoPath) -> Result<Vec<Symbol>> {
        let _ = file;
        todo!("engine-index: symbols_in_file — see docs/ARCHITECTURE.md")
    }

    /// Callers: references whose `to_symbol == Some(id)`.
    pub fn references_to(&self, id: SymbolId) -> Result<Vec<engine_core::Reference>> {
        let _ = id;
        todo!("engine-index: references_to — see docs/ARCHITECTURE.md")
    }

    /// Callees: references made *by* this symbol's own span.
    pub fn references_from(&self, id: SymbolId) -> Result<Vec<engine_core::Reference>> {
        let _ = id;
        todo!("engine-index: references_from — see docs/ARCHITECTURE.md")
    }

    pub fn all_files(&self) -> Result<Vec<RepoPath>> {
        todo!("engine-index: all_files — see docs/ARCHITECTURE.md")
    }
}

fn indexing_todo(_repo: &RepoHandle, _config: &BoreholeConfig) -> Result<IndexSummary> {
    todo!("engine-index: full_reindex — see docs/ARCHITECTURE.md")
}

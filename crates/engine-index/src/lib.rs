//! SQLite-backed symbol/reference index with incremental re-indexing.
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"). Consumed
//! by `engine-graph` and `engine-evidence`.

use engine_core::{
    BoreholeConfig, Confidence, Language, Reference, RepoHandle, RepoPath, Span, Symbol, SymbolId,
    SymbolKind,
};
use engine_parse::{ImportEdge, ParsedReference, ParsedSymbol};
use rusqlite::{params, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::time::Instant;

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
    ///
    /// Implementation is two-pass: pass A walks the repo and brings every
    /// file's `files`/`symbols`/`imports` rows up to date (including
    /// `SymbolId`-preserving matching of unchanged symbols by
    /// `(qualified_name, kind)`); pass B resolves and inserts `refs` for
    /// every file touched in pass A. The pass split matters — a reference
    /// in an early file can point at a symbol in a file processed later in
    /// the same walk, so resolution can't happen until every file's symbol
    /// table is final.
    pub fn full_reindex(
        &mut self,
        repo: &RepoHandle,
        config: &BoreholeConfig,
        mut on_progress: impl FnMut(IndexProgress),
    ) -> Result<IndexSummary> {
        let start = Instant::now();
        let walked = engine_core::RepoWalker::new(repo, config).walk()?;
        let files_total = walked.len() as u32;
        let known_paths: HashSet<String> =
            walked.iter().map(|f| f.path.as_str().to_string()).collect();

        let mut summary = IndexSummary {
            files_indexed: 0,
            files_skipped_unsupported: 0,
            files_skipped_too_large: 0,
            symbols_indexed: 0,
            references_indexed: 0,
            references_resolved: 0,
            duration_ms: 0,
        };

        let tx = self.conn.transaction()?;
        cleanup_deleted_files(&tx, &known_paths)?;

        let mut deferred: Vec<DeferredFile> = Vec::new();
        for (done, wf) in walked.iter().enumerate() {
            let outcome = process_file(&tx, wf, config, &mut summary)?;
            on_progress(IndexProgress {
                files_done: done as u32 + 1,
                files_total,
                current_file: outcome.file_id.map(|id| id as u32),
            });
            if let Some(item) = outcome.deferred {
                deferred.push(item);
            }
        }

        for item in &deferred {
            resolve_and_insert_refs(&tx, &known_paths, item, &mut summary)?;
        }

        tx.commit()?;
        summary.duration_ms = start.elapsed().as_millis() as u64;
        Ok(summary)
    }

    pub fn find_symbol(&self, id: SymbolId) -> Result<Option<Symbol>> {
        self.conn
            .query_row(SYMBOL_SELECT_BY_ID, params![id.0], row_to_symbol)
            .optional()
            .map_err(IndexError::from)
    }

    /// Substring/fuzzy match on `name` and `qualified_name`, ranked exact
    /// match first, then prefix match, then substring match, then
    /// alphabetically within a rank.
    pub fn search_symbols(&self, query: &str, limit: usize) -> Result<Vec<Symbol>> {
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let escaped = escape_like(query);
        let contains_pattern = format!("%{escaped}%");
        let prefix_pattern = format!("{escaped}%");
        let mut stmt = self.conn.prepare(
            "SELECT s.id, s.name, s.qualified_name, s.kind, s.language, f.path,
                    s.start_line, s.start_col, s.end_line, s.end_col, s.start_byte, s.end_byte,
                    s.is_exported, s.doc_comment, s.signature,
                CASE
                    WHEN s.name = ?1 OR s.qualified_name = ?1 THEN 0
                    WHEN s.name LIKE ?2 ESCAPE '\\' OR s.qualified_name LIKE ?2 ESCAPE '\\' THEN 1
                    ELSE 2
                END AS rank
             FROM symbols s JOIN files f ON f.id = s.file_id
             WHERE s.name LIKE ?3 ESCAPE '\\' OR s.qualified_name LIKE ?3 ESCAPE '\\'
             ORDER BY rank ASC, LENGTH(s.qualified_name) ASC, s.qualified_name ASC
             LIMIT ?4",
        )?;
        let rows = stmt.query_map(
            params![query, prefix_pattern, contains_pattern, limit as i64],
            row_to_symbol,
        )?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(IndexError::from)
    }

    pub fn symbols_in_file(&self, file: &RepoPath) -> Result<Vec<Symbol>> {
        let mut stmt = self.conn.prepare(&format!(
            "{SYMBOL_SELECT_COLUMNS} FROM symbols s JOIN files f ON f.id = s.file_id \
             WHERE f.path = ?1 ORDER BY s.start_line, s.start_col"
        ))?;
        let rows = stmt.query_map(params![file.as_str()], row_to_symbol)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(IndexError::from)
    }

    /// Callers: references whose `to_symbol == Some(id)`.
    pub fn references_to(&self, id: SymbolId) -> Result<Vec<Reference>> {
        let mut stmt = self.conn.prepare(&format!(
            "{REFERENCE_SELECT_COLUMNS} FROM refs r JOIN files f ON f.id = r.from_file_id \
             WHERE r.to_symbol_id = ?1"
        ))?;
        let rows = stmt.query_map(params![id.0], row_to_reference)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(IndexError::from)
    }

    /// Callees: references made *by* this symbol's own span.
    pub fn references_from(&self, id: SymbolId) -> Result<Vec<Reference>> {
        let mut stmt = self.conn.prepare(&format!(
            "{REFERENCE_SELECT_COLUMNS} FROM refs r JOIN files f ON f.id = r.from_file_id \
             WHERE r.from_symbol_id = ?1"
        ))?;
        let rows = stmt.query_map(params![id.0], row_to_reference)?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(IndexError::from)
    }

    pub fn all_files(&self) -> Result<Vec<RepoPath>> {
        let mut stmt = self.conn.prepare("SELECT path FROM files ORDER BY path")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        let mut out = Vec::new();
        for r in rows {
            out.push(RepoPath::new(r?));
        }
        Ok(out)
    }
}

// ---------------------------------------------------------------------
// Row <-> domain type mapping
// ---------------------------------------------------------------------

const SYMBOL_SELECT_COLUMNS: &str =
    "SELECT s.id, s.name, s.qualified_name, s.kind, s.language, f.path, \
     s.start_line, s.start_col, s.end_line, s.end_col, s.start_byte, s.end_byte, \
     s.is_exported, s.doc_comment, s.signature";

const SYMBOL_SELECT_BY_ID: &str =
    "SELECT s.id, s.name, s.qualified_name, s.kind, s.language, f.path, \
     s.start_line, s.start_col, s.end_line, s.end_col, s.start_byte, s.end_byte, \
     s.is_exported, s.doc_comment, s.signature \
     FROM symbols s JOIN files f ON f.id = s.file_id WHERE s.id = ?1";

const REFERENCE_SELECT_COLUMNS: &str = "SELECT r.from_symbol_id, f.path, \
     r.from_start_line, r.from_start_col, r.from_end_line, r.from_end_col, \
     r.from_start_byte, r.from_end_byte, r.to_symbol_id, r.to_name, r.kind, r.confidence";

fn row_to_symbol(row: &rusqlite::Row) -> rusqlite::Result<Symbol> {
    Ok(Symbol {
        id: SymbolId(row.get(0)?),
        name: row.get(1)?,
        qualified_name: row.get(2)?,
        kind: decode_enum(row.get(3)?)?,
        language: decode_enum(row.get(4)?)?,
        file: RepoPath::new(row.get::<_, String>(5)?),
        span: Span {
            start_line: row.get::<_, i64>(6)? as u32,
            start_col: row.get::<_, i64>(7)? as u32,
            end_line: row.get::<_, i64>(8)? as u32,
            end_col: row.get::<_, i64>(9)? as u32,
            start_byte: row.get::<_, i64>(10)? as u32,
            end_byte: row.get::<_, i64>(11)? as u32,
        },
        is_exported: row.get::<_, i64>(12)? != 0,
        doc_comment: row.get(13)?,
        signature: row.get(14)?,
    })
}

fn row_to_reference(row: &rusqlite::Row) -> rusqlite::Result<Reference> {
    Ok(Reference {
        from_symbol: row.get::<_, Option<i64>>(0)?.map(SymbolId),
        from_file: RepoPath::new(row.get::<_, String>(1)?),
        from_span: Span {
            start_line: row.get::<_, i64>(2)? as u32,
            start_col: row.get::<_, i64>(3)? as u32,
            end_line: row.get::<_, i64>(4)? as u32,
            end_col: row.get::<_, i64>(5)? as u32,
            start_byte: row.get::<_, i64>(6)? as u32,
            end_byte: row.get::<_, i64>(7)? as u32,
        },
        to_symbol: row.get::<_, Option<i64>>(8)?.map(SymbolId),
        to_name: row.get(9)?,
        kind: decode_enum(row.get(10)?)?,
        confidence: decode_enum(row.get(11)?)?,
    })
}

fn decode_enum<T: serde::de::DeserializeOwned>(s: String) -> rusqlite::Result<T> {
    serde_json::from_value(serde_json::Value::String(s)).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn to_text<T: serde::Serialize>(v: &T) -> String {
    match serde_json::to_value(v).expect("engine-core enums are always serializable") {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }
}

fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

// ---------------------------------------------------------------------
// full_reindex internals
// ---------------------------------------------------------------------

struct FileOutcome {
    /// The row id in `files` for this path, if it has one after processing
    /// (files with no detected language, or a language excluded by
    /// config, never get a row and this is `None`).
    file_id: Option<i64>,
    /// Present only for files that were newly parsed or re-parsed this
    /// run — these still need reference resolution in pass B, once every
    /// file's symbol table for this run is final.
    deferred: Option<DeferredFile>,
}

/// Bare symbol name -> ids in one file (for same-file resolution and for
/// resolving `ParsedReference::from_symbol_name`, both of which key off the
/// parser's bare identifier text, never `qualified_name`).
type SymbolsByName = HashMap<String, Vec<SymbolId>>;
/// One file's symbols with their spans, for the from-symbol
/// span-containment fallback used when the parser didn't attribute a
/// reference to a containing symbol by name (e.g. `extends`/`implements`
/// clauses — see `determine_from_symbol`).
type SymbolsBySpan = Vec<(Span, SymbolId)>;

struct DeferredFile {
    file_id: i64,
    path: RepoPath,
    language: Language,
    references: Vec<ParsedReference>,
    imports: Vec<ImportEdge>,
    symbols_by_name: SymbolsByName,
    symbols_by_span: SymbolsBySpan,
}

fn process_file(
    tx: &Transaction,
    wf: &engine_core::repo::WalkedFile,
    config: &BoreholeConfig,
    summary: &mut IndexSummary,
) -> Result<FileOutcome> {
    let Some(language) = wf.language else {
        return Ok(FileOutcome {
            file_id: None,
            deferred: None,
        });
    };
    if !config.languages.contains(&language) {
        summary.files_skipped_unsupported += 1;
        return Ok(FileOutcome {
            file_id: None,
            deferred: None,
        });
    }

    let meta = std::fs::metadata(&wf.absolute_path)?;
    let size = meta.len();
    let existing = fetch_file_row(tx, &wf.path)?;
    let existing_id = existing.as_ref().map(|(id, _)| *id);

    if size > config.max_file_size_bytes {
        let hash = hash_metadata(size, &meta);
        let unchanged = existing.as_ref().map(|(_, h)| h == &hash).unwrap_or(false);
        if let Some(id) = existing_id {
            if !unchanged {
                clear_file_contents(tx, id)?;
            }
        }
        let file_id = upsert_file_row(tx, &wf.path, language, &hash, size, existing_id)?;
        summary.files_skipped_too_large += 1;
        return Ok(FileOutcome {
            file_id: Some(file_id),
            deferred: None,
        });
    }

    let bytes = std::fs::read(&wf.absolute_path)?;
    let hash = hash_bytes(&bytes);

    if let Some((_, existing_hash)) = &existing {
        if existing_hash == &hash {
            // Unchanged: skip re-extraction entirely, including symbol
            // re-matching — this file's rows are already correct.
            return Ok(FileOutcome {
                file_id: existing_id,
                deferred: None,
            });
        }
    }

    let source = String::from_utf8_lossy(&bytes).into_owned();
    let parsed = engine_parse::parse_file(language, &source, &wf.path)?;

    let file_id = upsert_file_row(tx, &wf.path, language, &hash, size, existing_id)?;
    let (symbols_by_name, symbols_by_span) =
        upsert_symbols(tx, file_id, language, &parsed.symbols)?;
    replace_imports(tx, file_id, &parsed.imports)?;
    // Refs are recomputed wholesale for a changed file — cheaper and
    // simpler than diffing them, and (unlike symbols) nothing outside this
    // crate holds a stable identity for a single `Reference` row.
    tx.execute("DELETE FROM refs WHERE from_file_id = ?1", params![file_id])?;

    summary.files_indexed += 1;
    summary.symbols_indexed += parsed.symbols.len() as u32;

    Ok(FileOutcome {
        file_id: Some(file_id),
        deferred: Some(DeferredFile {
            file_id,
            path: wf.path.clone(),
            language,
            references: parsed.references,
            imports: parsed.imports,
            symbols_by_name,
            symbols_by_span,
        }),
    })
}

fn fetch_file_row(tx: &Transaction, path: &RepoPath) -> Result<Option<(i64, String)>> {
    tx.query_row(
        "SELECT id, content_hash FROM files WHERE path = ?1",
        params![path.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )
    .optional()
    .map_err(IndexError::from)
}

fn upsert_file_row(
    tx: &Transaction,
    path: &RepoPath,
    language: Language,
    hash: &str,
    size: u64,
    existing_id: Option<i64>,
) -> Result<i64> {
    let now = now_unix();
    if let Some(id) = existing_id {
        tx.execute(
            "UPDATE files SET language = ?1, content_hash = ?2, size_bytes = ?3, indexed_at = ?4 \
             WHERE id = ?5",
            params![to_text(&language), hash, size as i64, now, id],
        )?;
        Ok(id)
    } else {
        tx.execute(
            "INSERT INTO files (path, language, content_hash, size_bytes, indexed_at) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![path.as_str(), to_text(&language), hash, size as i64, now],
        )?;
        Ok(tx.last_insert_rowid())
    }
}

fn clear_file_contents(tx: &Transaction, file_id: i64) -> Result<()> {
    tx.execute("DELETE FROM refs WHERE from_file_id = ?1", params![file_id])?;
    tx.execute("DELETE FROM symbols WHERE file_id = ?1", params![file_id])?;
    tx.execute("DELETE FROM imports WHERE file_id = ?1", params![file_id])?;
    Ok(())
}

fn cleanup_deleted_files(tx: &Transaction, known_paths: &HashSet<String>) -> Result<()> {
    let stale: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id, path FROM files")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, path) = row?;
            if !known_paths.contains(&path) {
                out.push(id);
            }
        }
        out
    };
    for id in stale {
        // Cascades symbols/refs(from this file)/imports; also SETs NULL any
        // other file's refs pointing at a symbol that lived in this file.
        tx.execute("DELETE FROM files WHERE id = ?1", params![id])?;
    }
    Ok(())
}

/// Insert-or-update this file's symbols, preserving `SymbolId`s for
/// symbols that match an existing row by `(qualified_name, kind)` — see
/// `full_reindex`'s doc comment. Returns the file's own name->ids and
/// span->id maps for use in pass B.
fn upsert_symbols(
    tx: &Transaction,
    file_id: i64,
    language: Language,
    parsed_symbols: &[ParsedSymbol],
) -> Result<(SymbolsByName, SymbolsBySpan)> {
    let mut existing_by_key: HashMap<(String, SymbolKind), Vec<i64>> = HashMap::new();
    {
        let mut stmt =
            tx.prepare("SELECT id, qualified_name, kind FROM symbols WHERE file_id = ?1")?;
        let rows = stmt.query_map(params![file_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, qn, kind_text) = row?;
            let kind: SymbolKind = decode_enum(kind_text)?;
            existing_by_key.entry((qn, kind)).or_default().push(id);
        }
    }

    let mut by_name: HashMap<String, Vec<SymbolId>> = HashMap::new();
    let mut by_span: Vec<(Span, SymbolId)> = Vec::new();

    for ps in parsed_symbols {
        let key = (ps.qualified_name.clone(), ps.kind);
        let id = match existing_by_key.get_mut(&key).and_then(Vec::pop) {
            Some(id) => {
                update_symbol_row(tx, id, file_id, language, ps)?;
                id
            }
            None => insert_symbol_row(tx, file_id, language, ps)?,
        };
        let sid = SymbolId(id);
        by_name.entry(ps.name.clone()).or_default().push(sid);
        by_span.push((ps.span, sid));
    }

    // Whatever's left in existing_by_key didn't get matched to a parsed
    // symbol this time — it no longer exists in the file.
    for (_, ids) in existing_by_key {
        for id in ids {
            tx.execute("DELETE FROM symbols WHERE id = ?1", params![id])?;
        }
    }

    Ok((by_name, by_span))
}

fn insert_symbol_row(
    tx: &Transaction,
    file_id: i64,
    language: Language,
    ps: &ParsedSymbol,
) -> Result<i64> {
    tx.execute(
        "INSERT INTO symbols (file_id, name, qualified_name, kind, language, \
            start_line, start_col, end_line, end_col, start_byte, end_byte, \
            is_exported, doc_comment, signature) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
        params![
            file_id,
            ps.name,
            ps.qualified_name,
            to_text(&ps.kind),
            to_text(&language),
            ps.span.start_line as i64,
            ps.span.start_col as i64,
            ps.span.end_line as i64,
            ps.span.end_col as i64,
            ps.span.start_byte as i64,
            ps.span.end_byte as i64,
            ps.is_exported as i64,
            ps.doc_comment,
            ps.signature,
        ],
    )?;
    Ok(tx.last_insert_rowid())
}

fn update_symbol_row(
    tx: &Transaction,
    id: i64,
    file_id: i64,
    language: Language,
    ps: &ParsedSymbol,
) -> Result<()> {
    tx.execute(
        "UPDATE symbols SET file_id=?1, name=?2, qualified_name=?3, kind=?4, language=?5, \
            start_line=?6, start_col=?7, end_line=?8, end_col=?9, start_byte=?10, end_byte=?11, \
            is_exported=?12, doc_comment=?13, signature=?14 \
         WHERE id=?15",
        params![
            file_id,
            ps.name,
            ps.qualified_name,
            to_text(&ps.kind),
            to_text(&language),
            ps.span.start_line as i64,
            ps.span.start_col as i64,
            ps.span.end_line as i64,
            ps.span.end_col as i64,
            ps.span.start_byte as i64,
            ps.span.end_byte as i64,
            ps.is_exported as i64,
            ps.doc_comment,
            ps.signature,
            id,
        ],
    )?;
    Ok(())
}

fn replace_imports(tx: &Transaction, file_id: i64, imports: &[ImportEdge]) -> Result<()> {
    tx.execute("DELETE FROM imports WHERE file_id = ?1", params![file_id])?;
    for im in imports {
        tx.execute(
            "INSERT INTO imports (file_id, local_name, source_module, imported_name, \
                start_line, start_col, end_line, end_col, start_byte, end_byte) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
            params![
                file_id,
                im.local_name,
                im.source_module,
                im.imported_name,
                im.span.start_line as i64,
                im.span.start_col as i64,
                im.span.end_line as i64,
                im.span.end_col as i64,
                im.span.start_byte as i64,
                im.span.end_byte as i64,
            ],
        )?;
    }
    Ok(())
}

fn hash_bytes(bytes: &[u8]) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:x}", hasher.finish())
}

/// Change-detection hash for files too large to read in full: size plus
/// mtime (when the platform provides one). Not a content hash — a file
/// that changes without changing size or mtime within the same second
/// could be missed, an acceptable trade for never reading a multi-megabyte
/// generated/vendored file into memory just to hash it.
fn hash_metadata(size: u64, meta: &std::fs::Metadata) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    size.hash(&mut hasher);
    if let Ok(modified) = meta.modified() {
        if let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) {
            dur.as_nanos().hash(&mut hasher);
        }
    }
    format!("{:x}", hasher.finish())
}

fn now_unix() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

// ---------------------------------------------------------------------
// Pass B: reference resolution
// ---------------------------------------------------------------------

fn resolve_and_insert_refs(
    tx: &Transaction,
    known_paths: &HashSet<String>,
    item: &DeferredFile,
    summary: &mut IndexSummary,
) -> Result<()> {
    for pr in &item.references {
        summary.references_indexed += 1;
        let from_symbol = determine_from_symbol(item, pr);
        let resolved = resolve_reference(tx, known_paths, item, pr)?;
        if resolved.symbol.is_some() {
            summary.references_resolved += 1;
        }
        insert_ref_row(tx, item.file_id, from_symbol, pr, &resolved)?;
    }
    Ok(())
}

/// Which of this file's own symbols contains/produced this reference.
/// `ParsedReference::from_symbol_name` (when set) is always the bare
/// `name` the extractor was tracking, never `qualified_name` — see the
/// extractors in `engine-parse`. When it's `None` (extractors leave it
/// unset for e.g. `extends`/`implements` clauses, which aren't inside a
/// method body), fall back to the innermost same-file symbol whose span
/// contains the reference's span.
fn determine_from_symbol(item: &DeferredFile, pr: &ParsedReference) -> Option<SymbolId> {
    if let Some(name) = &pr.from_symbol_name {
        return match item.symbols_by_name.get(name) {
            Some(ids) if ids.len() == 1 => Some(ids[0]),
            _ => None, // no match, or ambiguous within file — don't guess
        };
    }
    let mut best: Option<(u32, SymbolId)> = None;
    for (span, id) in &item.symbols_by_span {
        if span_contains(span, &pr.from_span) {
            let extent = span.end_byte.saturating_sub(span.start_byte);
            if best.map(|(e, _)| extent < e).unwrap_or(true) {
                best = Some((extent, *id));
            }
        }
    }
    best.map(|(_, id)| id)
}

fn span_contains(outer: &Span, inner: &Span) -> bool {
    outer.start_byte <= inner.start_byte && inner.end_byte <= outer.end_byte
}

fn resolve_reference(
    tx: &Transaction,
    known_paths: &HashSet<String>,
    item: &DeferredFile,
    pr: &ParsedReference,
) -> Result<resolve::Resolved> {
    // Step 1: same-file.
    if let Some(matches) = item.symbols_by_name.get(&pr.to_name) {
        return Ok(match matches.len() {
            1 => resolve::Resolved {
                symbol: Some(matches[0]),
                confidence: Confidence::High,
            },
            _ => resolve::Resolved {
                symbol: None,
                confidence: Confidence::Low,
            },
        });
    }

    // Step 2: import-scoped.
    let binding_imports: Vec<&ImportEdge> = item
        .imports
        .iter()
        .filter(|im| im.local_name == pr.to_name)
        .collect();
    if binding_imports.len() == 1 {
        let im = binding_imports[0];
        let target_path =
            resolve::resolve_module_path(&item.path, &im.source_module, item.language)
                .and_then(|stem| find_import_target(known_paths, stem.as_str(), item.language));
        if let Some(target_path) = target_path {
            if let Some(target_file_id) = fetch_file_id(tx, &target_path)? {
                if let Some(sym_id) =
                    find_symbol_by_name_in_file(tx, target_file_id, &im.imported_name)?
                {
                    return Ok(resolve::Resolved {
                        symbol: Some(sym_id),
                        confidence: Confidence::High,
                    });
                }
            }
            return Ok(resolve::Resolved {
                symbol: None,
                confidence: Confidence::Medium,
            });
        }
        return Ok(resolve::Resolved {
            symbol: None,
            confidence: Confidence::Low,
        });
    } else if binding_imports.len() > 1 {
        return Ok(resolve::Resolved {
            symbol: None,
            confidence: Confidence::Low,
        });
    }

    // Step 3: global-unique, else refuse to guess.
    let candidates = find_symbols_by_name_global(tx, &pr.to_name)?;
    Ok(match candidates.len() {
        1 => resolve::Resolved {
            symbol: Some(candidates[0]),
            confidence: Confidence::Medium,
        },
        _ => resolve::Resolved {
            symbol: None,
            confidence: Confidence::Low,
        },
    })
}

/// `resolve_module_path` returns an extension-less stem (see its doc
/// comment) since it has no way to check file existence itself. This tries
/// the stem verbatim, then the language's ordered suffix list, against the
/// set of paths the current repo walk actually found on disk.
fn find_import_target(
    known_paths: &HashSet<String>,
    stem: &str,
    language: Language,
) -> Option<String> {
    if known_paths.contains(stem) {
        return Some(stem.to_string());
    }
    let suffixes: &[&str] = match language {
        Language::TypeScript | Language::Tsx | Language::JavaScript => &[
            ".ts",
            ".tsx",
            ".js",
            ".jsx",
            "/index.ts",
            "/index.tsx",
            "/index.js",
            "/index.jsx",
        ],
        Language::Python => &[".py", "/__init__.py"],
        Language::Rust => &[".rs", "/mod.rs"],
        Language::Go => &[],
    };
    for suffix in suffixes {
        let candidate = format!("{stem}{suffix}");
        if known_paths.contains(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn fetch_file_id(tx: &Transaction, path: &str) -> Result<Option<i64>> {
    tx.query_row("SELECT id FROM files WHERE path = ?1", params![path], |r| {
        r.get(0)
    })
    .optional()
    .map_err(IndexError::from)
}

fn find_symbol_by_name_in_file(
    tx: &Transaction,
    file_id: i64,
    name: &str,
) -> Result<Option<SymbolId>> {
    let ids: Vec<i64> = {
        let mut stmt = tx.prepare("SELECT id FROM symbols WHERE file_id = ?1 AND name = ?2")?;
        let rows = stmt.query_map(params![file_id, name], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<Vec<i64>>>()?
    };
    Ok(match ids.len() {
        1 => Some(SymbolId(ids[0])),
        _ => None,
    })
}

fn find_symbols_by_name_global(tx: &Transaction, name: &str) -> Result<Vec<SymbolId>> {
    let mut stmt = tx.prepare("SELECT id FROM symbols WHERE name = ?1")?;
    let rows = stmt.query_map(params![name], |r| r.get::<_, i64>(0))?;
    let mut out = Vec::new();
    for r in rows {
        out.push(SymbolId(r?));
    }
    Ok(out)
}

fn insert_ref_row(
    tx: &Transaction,
    file_id: i64,
    from_symbol: Option<SymbolId>,
    pr: &ParsedReference,
    resolved: &resolve::Resolved,
) -> Result<()> {
    tx.execute(
        "INSERT INTO refs (from_file_id, from_symbol_id, \
            from_start_line, from_start_col, from_end_line, from_end_col, \
            from_start_byte, from_end_byte, to_symbol_id, to_name, kind, confidence) \
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
        params![
            file_id,
            from_symbol.map(|s| s.0),
            pr.from_span.start_line as i64,
            pr.from_span.start_col as i64,
            pr.from_span.end_line as i64,
            pr.from_span.end_col as i64,
            pr.from_span.start_byte as i64,
            pr.from_span.end_byte as i64,
            resolved.symbol.map(|s| s.0),
            pr.to_name,
            to_text(&pr.kind),
            to_text(&resolved.confidence),
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------
// Fixture-driven tests that need raw-row access (`Index::conn` is
// `pub(crate)`, invisible to `tests/` integration tests, which are a
// separate crate) — everything else lives in `tests/fixtures_test.rs`.
// ---------------------------------------------------------------------

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
    }

    /// The single most important test in this crate: two unrelated
    /// `Handler.process` methods (billing vs. shipping) share a name, and
    /// `caller.ts` calls `.process()` on a value of unknown origin with no
    /// import to disambiguate. The resolver must refuse to guess — see
    /// `docs/ARCHITECTURE.md` "Reference resolution algorithm" step 4 and
    /// `fixtures/ambiguous-refs/NOTES.md`.
    #[test]
    fn ambiguous_refs_ref_is_unresolved_low_confidence() {
        let dir = fixture_path("ambiguous-refs");
        if !dir.exists() {
            eprintln!("skipping: run fixtures/build-fixtures.sh first");
            return;
        }
        let repo = RepoHandle::open(&dir).unwrap();
        let config = BoreholeConfig::default();
        let mut index = Index::open_in_memory().unwrap();
        index.full_reindex(&repo, &config, |_| {}).unwrap();

        // Sanity: both same-named methods really exist as distinct symbols
        // (otherwise "unresolved" would be trivially true for the wrong
        // reason).
        let candidates = index.search_symbols("process", 10).unwrap();
        let process_methods: Vec<_> = candidates.iter().filter(|s| s.name == "process").collect();
        assert_eq!(
            process_methods.len(),
            2,
            "expected billing::Handler.process and shipping::Handler.process, got {:?}",
            process_methods
        );

        // The `h.process()` call in caller.ts is neither inside a named
        // symbol nor import-scoped, so it can only be reached by querying
        // the raw row — a ref with neither end resolved to a symbol isn't
        // reachable via references_to/references_from at all.
        let (to_symbol_id, confidence): (Option<i64>, String) = index
            .conn
            .query_row(
                "SELECT r.to_symbol_id, r.confidence FROM refs r \
                 JOIN files f ON f.id = r.from_file_id \
                 WHERE f.path = 'caller.ts' AND r.to_name = 'process'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();

        assert_eq!(
            to_symbol_id, None,
            "ambiguous same-named ref must not be silently resolved to either candidate"
        );
        assert_eq!(confidence, "low");
    }
}

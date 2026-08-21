use crate::error::{CommandError, CommandResult};
use crate::state::{AppState, OpenRepo};
use engine_core::{BoreholeConfig, RepoHandle, SymbolId};
use engine_git::GitAnalyzer;
use engine_index::Index;
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

#[derive(Debug, Serialize)]
pub struct RepoHandleRef {
    pub root: String,
    pub has_git: bool,
}

fn index_db_path(repo: &RepoHandle) -> std::path::PathBuf {
    repo.root().join(".borehole").join("index.db")
}

#[tauri::command]
pub fn open_repository(
    path: String,
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<RepoHandleRef> {
    let repo = RepoHandle::open(&path)?;
    let config = BoreholeConfig::load(repo.root())?;
    std::fs::create_dir_all(repo.root().join(".borehole"))?;
    let mut index = Index::open(&index_db_path(&repo))?;
    let has_git = repo.has_git();

    // Index synchronously on open for v1 simplicity; emitting progress
    // events lets the UI render a bar even though this call blocks the
    // command's own return. A background-indexing redesign (returning
    // immediately and streaming `index-complete` later) is straightforward
    // to layer on top of this without changing the event names, once
    // indexing time on large repos makes synchronous open feel slow.
    let summary = index.full_reindex(&repo, &config, |progress| {
        let _ = app.emit("index-progress", progress);
    });
    match &summary {
        Ok(s) => {
            let _ = app.emit("index-complete", s);
        }
        Err(e) => {
            let _ = app.emit("index-error", e.to_string());
        }
    }
    summary.map_err(CommandError::from)?;

    // A repository with no `.git` (e.g. a plain directory) is still
    // openable for structural analysis — git-backed evidence just degrades
    // to "unavailable" rather than blocking repo open entirely. See
    // `engine_evidence::EvidenceEngine`'s doc comment on `git: Option<..>`.
    let git = GitAnalyzer::open(repo.root()).ok();
    let root = repo.root().to_string_lossy().to_string();

    let mut guard = state.open_repo.lock().map_err(|_| CommandError {
        message: "state lock poisoned".into(),
    })?;
    *guard = Some(OpenRepo {
        repo,
        index,
        git,
        config,
    });

    Ok(RepoHandleRef { root, has_git })
}

fn with_open_repo<T>(
    state: &State<'_, AppState>,
    f: impl FnOnce(&OpenRepo) -> CommandResult<T>,
) -> CommandResult<T> {
    let guard = state.open_repo.lock().map_err(|_| CommandError {
        message: "state lock poisoned".into(),
    })?;
    let open = guard.as_ref().ok_or_else(|| CommandError {
        message: "no repository is open — call open_repository first".into(),
    })?;
    f(open)
}

#[tauri::command]
pub fn reindex(
    state: State<'_, AppState>,
    app: AppHandle,
) -> CommandResult<engine_index::IndexSummary> {
    let mut guard = state.open_repo.lock().map_err(|_| CommandError {
        message: "state lock poisoned".into(),
    })?;
    let open = guard.as_mut().ok_or_else(|| CommandError {
        message: "no repository is open".into(),
    })?;
    let summary = open
        .index
        .full_reindex(&open.repo, &open.config, |progress| {
            let _ = app.emit("index-progress", progress);
        })?;
    let _ = app.emit("index-complete", &summary);
    Ok(summary)
}

/// Hard ceiling on `search_symbols`'s `limit`, independent of whatever the
/// caller asks for. The query itself is safely parameterized (no injection
/// risk), but with no cap a caller on the IPC boundary could request an
/// enormous limit — SQLite treats a negative `LIMIT` (e.g. `usize::MAX` cast
/// to `i64`, which wraps negative) as "no limit at all" — and force the
/// whole matching symbol table back across IPC on a large repo.
const MAX_SEARCH_LIMIT: usize = 500;

#[tauri::command]
pub fn search_symbols(
    query: String,
    limit: Option<usize>,
    state: State<'_, AppState>,
) -> CommandResult<Vec<engine_core::Symbol>> {
    with_open_repo(&state, |open| {
        let limit = limit.unwrap_or(25).min(MAX_SEARCH_LIMIT);
        Ok(open.index.search_symbols(&query, limit)?)
    })
}

#[tauri::command]
pub fn get_symbol_profile(
    id: SymbolId,
    state: State<'_, AppState>,
) -> CommandResult<engine_evidence::SymbolProfile> {
    with_open_repo(&state, |open| Ok(open.evidence().symbol_profile(id)?))
}

#[tauri::command]
pub fn get_callers(
    id: SymbolId,
    max_depth: Option<u32>,
    state: State<'_, AppState>,
) -> CommandResult<engine_graph::DependencyGraph> {
    with_open_repo(&state, |open| {
        let depth = max_depth.unwrap_or(open.config.max_graph_depth);
        Ok(engine_graph::expand_callers(&open.index, id, depth)?)
    })
}

#[tauri::command]
pub fn get_callees(
    id: SymbolId,
    max_depth: Option<u32>,
    state: State<'_, AppState>,
) -> CommandResult<engine_graph::DependencyGraph> {
    with_open_repo(&state, |open| {
        let depth = max_depth.unwrap_or(open.config.max_graph_depth);
        Ok(engine_graph::expand_callees(&open.index, id, depth)?)
    })
}

#[tauri::command]
pub fn get_blast_radius(
    id: SymbolId,
    state: State<'_, AppState>,
) -> CommandResult<engine_evidence::BlastRadius> {
    with_open_repo(&state, |open| Ok(open.evidence().blast_radius(id)?))
}

#[tauri::command]
pub fn get_before_you_change_this(
    id: SymbolId,
    state: State<'_, AppState>,
) -> CommandResult<engine_evidence::BeforeYouChangeThisReport> {
    with_open_repo(&state, |open| {
        Ok(open.evidence().before_you_change_this(id)?)
    })
}

#[tauri::command]
pub fn get_history(
    path_or_symbol: String,
    state: State<'_, AppState>,
) -> CommandResult<engine_evidence::CodeEvolution> {
    with_open_repo(&state, |open| {
        let path = engine_core::RepoPath::new(path_or_symbol);
        Ok(open.evidence().code_evolution(&path)?)
    })
}

#[tauri::command]
pub fn get_related_tests(
    id: SymbolId,
    state: State<'_, AppState>,
) -> CommandResult<Vec<engine_evidence::TestRef>> {
    with_open_repo(&state, |open| Ok(open.evidence().related_tests(id)?))
}

/// Resolves a user-typed target to a `SymbolId`: exact-match search, top
/// result wins. Mirrors `crates/cli/src/commands.rs`'s
/// `resolve_target_to_symbol` — small enough that duplicating it here beats
/// threading a shared helper across the CLI/desktop crate boundary for one
/// function.
fn resolve_target(index: &Index, target: &str) -> CommandResult<SymbolId> {
    index
        .search_symbols(target, 1)?
        .into_iter()
        .next()
        .map(|s| s.id)
        .ok_or_else(|| CommandError {
            message: format!("no symbol matching '{target}'"),
        })
}

/// Bundles everything the evidence engine knows about one symbol —
/// profile, blast radius, and the before-you-change-this reading list —
/// into a single document, for pasting into a code review, an issue, or an
/// AI coding assistant's context window (brief section 29: "Export
/// Context"). Every fact in the output already came from `EvidenceEngine`;
/// this function only selects and formats it, never adds new claims.
#[tauri::command]
pub fn export_context(
    path_or_symbol: String,
    format: String,
    state: State<'_, AppState>,
) -> CommandResult<String> {
    with_open_repo(&state, |open| {
        let id = resolve_target(&open.index, &path_or_symbol)?;
        let evidence = open.evidence();
        let profile = evidence.symbol_profile(id)?;
        let before = evidence.before_you_change_this(id)?;

        match format.as_str() {
            "json" => {
                #[derive(serde::Serialize)]
                struct ExportBundle {
                    profile: engine_evidence::SymbolProfile,
                    before_you_change_this: engine_evidence::BeforeYouChangeThisReport,
                }
                let bundle = ExportBundle {
                    profile,
                    before_you_change_this: before,
                };
                serde_json::to_string_pretty(&bundle).map_err(CommandError::from)
            }
            "md" => Ok(render_context_markdown(&profile, &before)),
            other => Err(CommandError {
                message: format!("unknown export format '{other}' (expected 'md' or 'json')"),
            }),
        }
    })
}

fn render_context_markdown(
    profile: &engine_evidence::SymbolProfile,
    before: &engine_evidence::BeforeYouChangeThisReport,
) -> String {
    use std::fmt::Write;
    let s = &profile.symbol;
    let mut out = String::new();
    let _ = writeln!(out, "# {} ({})", s.qualified_name, s.kind);
    let _ = writeln!(out, "\n`{}:{}`\n", s.file, s.span.start_line);
    if let Some(sig) = &s.signature {
        let _ = writeln!(out, "```\n{sig}\n```\n");
    }
    if let Some(doc) = &s.doc_comment {
        let _ = writeln!(out, "{doc}\n");
    }

    let _ = writeln!(out, "## Evidence");
    let _ = writeln!(out, "- Direct callers: {}", profile.direct_callers);
    let _ = writeln!(out, "- Indirect callers: {}", profile.indirect_callers);
    let _ = writeln!(out, "- Related tests: {}", profile.tests.len());
    if let Some(commit) = &profile.introduced {
        let _ = writeln!(
            out,
            "- Introduced in commit `{}`: {}",
            &commit.sha[..commit.sha.len().min(7)],
            commit.summary
        );
    }
    let _ = writeln!(
        out,
        "- Modification count: {}\n",
        profile.modification_count
    );

    let _ = writeln!(out, "## Before you change this");
    if before.reading_list.is_empty() {
        let _ = writeln!(
            out,
            "No specific reading list items — see blast radius below.\n"
        );
    } else {
        for item in &before.reading_list {
            let _ = writeln!(out, "1. **{}** — {}", item.symbol_or_file, item.why);
        }
        out.push('\n');
    }

    let br = &before.blast_radius;
    let _ = writeln!(out, "## Blast radius");
    let _ = writeln!(out, "- Risk: **{}**", br.risk.to_string().to_uppercase());
    for reason in &br.risk_reasons {
        let _ = writeln!(out, "  - {}", reason.text);
    }
    let _ = writeln!(
        out,
        "- Public API: {}",
        if br.is_public_api { "yes" } else { "no" }
    );
    let _ = writeln!(out, "- Test suites: {}\n", br.test_suites);

    if !before.historical_warnings.is_empty() {
        let _ = writeln!(out, "## Historical warnings");
        for w in &before.historical_warnings {
            let _ = writeln!(out, "- {}", w.text);
        }
        out.push('\n');
    }

    let _ = writeln!(
        out,
        "*Confidence: {}. Generated by Borehole — evidence-backed, not fabricated.*",
        before.confidence
    );
    out
}

use crate::Command;
use engine_core::{BoreholeConfig, RepoHandle};
use engine_git::GitAnalyzer;
use engine_index::Index;
use std::path::{Path, PathBuf};

fn index_db_path(repo: &RepoHandle) -> PathBuf {
    repo.root().join(".borehole").join("index.db")
}

fn open_repo(path: &Path) -> anyhow::Result<RepoHandle> {
    RepoHandle::open(path)
        .map_err(|e| anyhow::anyhow!("{e}\n\nIs {} a real directory?", path.display()))
}

pub fn run(command: Command, json: bool) -> anyhow::Result<()> {
    match command {
        Command::Analyze { path } => {
            let repo = open_repo(&path)?;
            let config = BoreholeConfig::load(repo.root())?;
            std::fs::create_dir_all(repo.root().join(".borehole"))?;
            let mut index = Index::open(&index_db_path(&repo))?;
            let summary = index.full_reindex(&repo, &config, |progress| {
                if !json {
                    eprint!(
                        "\rIndexing... {}/{} files",
                        progress.files_done, progress.files_total
                    );
                }
            })?;
            if json {
                println!("{}", serde_json::to_string_pretty(&summary)?);
            } else {
                eprintln!();
                println!(
                    "Indexed {} files, {} symbols, {} references ({} resolved).",
                    summary.files_indexed,
                    summary.symbols_indexed,
                    summary.references_indexed,
                    summary.references_resolved
                );
            }
            Ok(())
        }
        Command::Symbol { name, repo } => {
            let repo = open_repo(&repo)?;
            let index = Index::open(&index_db_path(&repo))?;
            let results = index.search_symbols(&name, 25)?;
            emit(&results, json, |r| {
                for s in r {
                    println!(
                        "{}  {}  {}:{}",
                        s.kind, s.qualified_name, s.file, s.span.start_line
                    );
                }
            })
        }
        Command::Callers { symbol, repo } => {
            run_graph_query(&repo, &symbol, json, GraphQuery::Callers)
        }
        Command::Callees { symbol, repo } => {
            run_graph_query(&repo, &symbol, json, GraphQuery::Callees)
        }
        Command::Impact {
            target,
            repo,
            before,
        } => {
            let (repo, index, git, config) = open_engine(&repo)?;
            let evidence =
                engine_evidence::EvidenceEngine::new(&index, git.as_ref(), &repo, &config);
            let id = resolve_target_to_symbol(&index, &target)?;
            if before {
                let report = evidence.before_you_change_this(id)?;
                emit(&report, json, print_before_you_change_this)
            } else {
                let report = evidence.blast_radius(id)?;
                emit(&report, json, print_blast_radius)
            }
        }
        Command::History { target, repo } => {
            let (repo, index, git, config) = open_engine(&repo)?;
            let evidence =
                engine_evidence::EvidenceEngine::new(&index, git.as_ref(), &repo, &config);
            let path = engine_core::RepoPath::new(target);
            let evolution = evidence.code_evolution(&path)?;
            emit(&evolution, json, |e| {
                println!("Commits: {}", e.all_commits.len());
                for c in &e.major_refactors {
                    println!(
                        "  [refactor] {} {}",
                        &c.sha[..7.min(c.sha.len())],
                        c.summary
                    );
                }
            })
        }
        Command::Tests { symbol, repo } => {
            let (repo, index, git, config) = open_engine(&repo)?;
            let evidence =
                engine_evidence::EvidenceEngine::new(&index, git.as_ref(), &repo, &config);
            let id = resolve_target_to_symbol(&index, &symbol)?;
            let tests = evidence.related_tests(id)?;
            emit(&tests, json, |ts| {
                for t in ts {
                    println!("{}  ({:?}, {})", t.file, t.reason, t.confidence);
                }
            })
        }
        Command::Explain { target, repo } => {
            let (repo, index, git, config) = open_engine(&repo)?;
            let evidence =
                engine_evidence::EvidenceEngine::new(&index, git.as_ref(), &repo, &config);
            let id = resolve_target_to_symbol(&index, &target)?;
            let profile = evidence.symbol_profile(id)?;
            emit(&profile, json, |p| {
                println!("{} ({})", p.symbol.qualified_name, p.symbol.kind);
                println!(
                    "  {} direct callers, {} indirect",
                    p.direct_callers, p.indirect_callers
                );
                println!("  {} related test files", p.tests.len());
            })
        }
    }
}

enum GraphQuery {
    Callers,
    Callees,
}

fn run_graph_query(repo: &Path, symbol: &str, json: bool, which: GraphQuery) -> anyhow::Result<()> {
    let repo = open_repo(repo)?;
    let index = Index::open(&index_db_path(&repo))?;
    let id = resolve_target_to_symbol(&index, symbol)?;
    let config = BoreholeConfig::load(repo.root())?;
    let graph = match which {
        GraphQuery::Callers => engine_graph::expand_callers(&index, id, config.max_graph_depth)?,
        GraphQuery::Callees => engine_graph::expand_callees(&index, id, config.max_graph_depth)?,
    };
    emit(&graph, json, |g| {
        for node in &g.nodes {
            println!(
                "{}  {}  ({})",
                "  ".repeat(node.depth as usize),
                node.symbol.qualified_name,
                node.symbol.file
            );
        }
        if g.truncated {
            println!("(truncated at max depth — re-run with a higher limit to see more)");
        }
    })
}

fn open_engine(
    path: &Path,
) -> anyhow::Result<(RepoHandle, Index, Option<GitAnalyzer>, BoreholeConfig)> {
    let repo = open_repo(path)?;
    let config = BoreholeConfig::load(repo.root())?;
    let index = Index::open(&index_db_path(&repo))?;
    // No `.git` (or `git2` couldn't open it, e.g. no commits yet) degrades
    // history-dependent evidence rather than blocking every command — see
    // `engine_evidence::EvidenceEngine`'s doc comment on `git: Option<..>`.
    let git = GitAnalyzer::open(repo.root()).ok();
    if git.is_none() && !repo.has_git() {
        eprintln!("Note: no .git directory found — history-based evidence will be unavailable.");
    }
    Ok((repo, index, git, config))
}

/// Resolves a CLI-provided target string to a `SymbolId`: tries an exact
/// qualified-name match first, then falls back to `search_symbols` and
/// takes the top match, erroring if nothing matches at all. Ambiguity
/// beyond "top match" is surfaced by `borehole symbol <name>` listing all
/// candidates, not silently guessed here.
fn resolve_target_to_symbol(index: &Index, target: &str) -> anyhow::Result<engine_core::SymbolId> {
    let matches = index.search_symbols(target, 1)?;
    matches.into_iter().next().map(|s| s.id).ok_or_else(|| {
        anyhow::anyhow!("no symbol matching '{target}' — try `borehole symbol {target}` to search")
    })
}

fn emit<T: serde::Serialize>(value: &T, json: bool, print: impl FnOnce(&T)) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        print(value);
    }
    Ok(())
}

fn print_blast_radius(r: &engine_evidence::BlastRadius) {
    println!("BLAST RADIUS: {}", r.target.qualified_name);
    println!();
    println!("Direct callers:   {}", r.direct_callers);
    println!("Indirect callers: {}", r.indirect_callers);
    println!("Test suites:      {}", r.test_suites);
    println!(
        "Public API:       {}",
        if r.is_public_api { "yes" } else { "no" }
    );
    println!("Config files:     {}", r.config_files.len());
    println!();
    println!("Risk: {}", r.risk.to_string().to_uppercase());
    for reason in &r.risk_reasons {
        println!("  - {}", reason.text);
    }
}

fn print_before_you_change_this(r: &engine_evidence::BeforeYouChangeThisReport) {
    println!("BEFORE YOU CHANGE THIS: {}", r.target.qualified_name);
    println!();
    println!("Understand first:");
    for (i, item) in r.reading_list.iter().enumerate() {
        println!("  {}. {} — {}", i + 1, item.symbol_or_file, item.why);
    }
    println!();
    print_blast_radius(&r.blast_radius);
    if !r.historical_warnings.is_empty() {
        println!();
        println!("Historical warning:");
        for w in &r.historical_warnings {
            println!("  {}", w.text);
        }
    }
    println!();
    println!("Confidence: {}", r.confidence);
}

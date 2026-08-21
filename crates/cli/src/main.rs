//! `borehole` — CLI surface over the same `engine-evidence` API the desktop
//! app calls. See docs/ARCHITECTURE.md "Application API / IPC contract".

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod commands;

#[derive(Parser)]
#[command(
    name = "borehole",
    version,
    about = "Understand unfamiliar code before you change it."
)]
struct Cli {
    #[command(subcommand)]
    command: Command,

    /// Emit machine-readable JSON instead of formatted text.
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand)]
enum Command {
    /// Index a repository (or refresh an existing index).
    Analyze { path: PathBuf },
    /// Search for a symbol by name.
    Symbol {
        name: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
    /// List callers of a symbol.
    Callers {
        symbol: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
    /// List callees of a symbol.
    Callees {
        symbol: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
    /// Blast-radius / change-impact report for a symbol or path.
    Impact {
        target: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
        /// Include the full "before you change this" reading list.
        #[arg(long)]
        before: bool,
    },
    /// Git history for a symbol or path.
    History {
        target: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
    /// Related tests for a symbol.
    Tests {
        symbol: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
    /// Full evidence-backed explanation of a symbol or path.
    Explain {
        target: String,
        #[arg(default_value = ".")]
        repo: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    commands::run(cli.command, cli.json)
}

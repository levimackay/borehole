use serde::{Deserialize, Serialize};

/// User-editable indexing/analysis configuration. Loaded from
/// `.borehole/config.toml` at the repository root when present; every field
/// has a default so a repository with no config file at all still indexes
/// correctly (brief requirement: "Never require users to configure 30
/// things just to start").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct BoreholeConfig {
    /// Directory names ignored in addition to .gitignore, matched anywhere
    /// in the path (not just at the root).
    pub ignored_dirs: Vec<String>,
    /// File extensions never indexed even if a language would otherwise
    /// claim them (e.g. generated `.g.ts`).
    pub ignored_extensions: Vec<String>,
    /// Path glob patterns (relative to repo root) treated as generated and
    /// excluded from evidence relevance (still indexed for navigation, but
    /// never cited as "before you change this" reading and never counted
    /// in temporal coupling).
    pub generated_path_globs: Vec<String>,
    /// Languages to index. Defaults to all supported languages; narrowing
    /// this speeds up indexing on large polyglot repos where only one
    /// stack matters to the user.
    pub languages: Vec<crate::Language>,
    /// Maximum graph traversal depth for callers/callees/blast-radius
    /// expansion, to bound worst-case traversal time on densely connected
    /// code.
    pub max_graph_depth: u32,
    /// Maximum file size (bytes) to parse; larger files are indexed as
    /// file-level entries only (no symbol extraction) to bound memory use
    /// on generated/vendored mega-files that slipped past ignore rules.
    pub max_file_size_bytes: u64,
    /// How many commits of history to mine for temporal coupling and code
    /// evolution. `None` means the full history.
    pub git_history_limit: Option<u32>,
}

impl Default for BoreholeConfig {
    fn default() -> Self {
        Self {
            ignored_dirs: default_ignored_dirs(),
            ignored_extensions: Vec::new(),
            generated_path_globs: default_generated_globs(),
            languages: crate::Language::all().to_vec(),
            max_graph_depth: 6,
            max_file_size_bytes: 2 * 1024 * 1024,
            git_history_limit: None,
        }
    }
}

fn default_ignored_dirs() -> Vec<String> {
    [
        ".git",
        "node_modules",
        "target",
        "dist",
        "build",
        "vendor",
        ".venv",
        "venv",
        "__pycache__",
        ".next",
        ".nuxt",
        "coverage",
        ".borehole",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

fn default_generated_globs() -> Vec<String> {
    ["**/*.pb.go", "**/*_pb2.py", "**/*.generated.*", "**/*.g.ts"]
        .iter()
        .map(|s| s.to_string())
        .collect()
}

impl BoreholeConfig {
    /// Load from `<repo_root>/.borehole/config.toml`, falling back to
    /// defaults if the file doesn't exist. A malformed config file is a
    /// hard error rather than a silent fallback — a user who wrote a typo
    /// in their config should see that, not get unexplained default
    /// behavior.
    pub fn load(repo_root: &std::path::Path) -> anyhow::Result<Self> {
        let path = repo_root.join(".borehole").join("config.toml");
        if !path.exists() {
            return Ok(Self::default());
        }
        let text = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("reading {}: {e}", path.display()))?;
        toml::from_str(&text).map_err(|e| anyhow::anyhow!("parsing {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_usable_with_no_config_file() {
        let cfg = BoreholeConfig::default();
        assert!(cfg.ignored_dirs.contains(&"node_modules".to_string()));
        assert!(!cfg.languages.is_empty());
        assert!(cfg.max_graph_depth > 0);
    }
}

use crate::BoreholeConfig;
use ignore::WalkBuilder;
use std::path::Path;

/// Builds a directory walker that respects `.gitignore` (and `.ignore`,
/// `.git/info/exclude`) plus the config's extra `ignored_dirs`. This is the
/// single place ignore semantics are decided — nothing else in the engine
/// should hand-roll a "should I skip this path" check, so ignore behavior
/// stays consistent between indexing, git analysis, and CLI file listing.
pub fn build_walker(repo_root: &Path, config: &BoreholeConfig) -> WalkBuilder {
    let mut builder = WalkBuilder::new(repo_root);
    builder
        .hidden(false) // .gitignore already hides dotfiles it wants hidden; don't blanket-skip dotfiles like .github/
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        // `.gitignore` should apply even when the walked directory isn't
        // itself a git repo (e.g. a fixture directory in tests, or a repo
        // opened mid-clone before `.git` exists) — without this the
        // `ignore` crate silently skips all git-related ignore sources.
        .require_git(false);

    let extra_dirs = config.ignored_dirs.clone();
    builder.filter_entry(move |entry| {
        let name = entry.file_name().to_string_lossy();
        !extra_dirs.iter().any(|d| d == name.as_ref())
    });

    builder
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn respects_gitignore_and_extra_dirs() {
        let dir = tempdir().unwrap();
        let root = dir.path();
        fs::write(root.join(".gitignore"), "ignored_by_gitignore.txt\n").unwrap();
        fs::write(root.join("ignored_by_gitignore.txt"), "x").unwrap();
        fs::write(root.join("kept.txt"), "x").unwrap();
        fs::create_dir(root.join("node_modules")).unwrap();
        fs::write(root.join("node_modules/pkg.js"), "x").unwrap();

        let config = BoreholeConfig::default();
        let walker = build_walker(root, &config);
        let paths: Vec<String> = walker
            .build()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.path().file_name().unwrap().to_string_lossy().to_string())
            .collect();

        assert!(paths.contains(&"kept.txt".to_string()));
        assert!(!paths.contains(&"ignored_by_gitignore.txt".to_string()));
        assert!(!paths.contains(&"pkg.js".to_string()));
    }
}

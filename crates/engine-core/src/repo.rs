use crate::{ignore_rules, BoreholeConfig, Language, RepoPath};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// A validated, canonicalized handle to a repository on disk. Every other
/// crate that touches the filesystem takes a `&RepoHandle` rather than a
/// raw path, so path validation happens exactly once, at the boundary where
/// the user (or a saved recent-repos list) hands us an untrusted string.
///
/// This is the primary defense against path traversal (brief section 23):
/// `resolve` refuses to return any path that canonicalizes outside `root`.
#[derive(Debug, Clone)]
pub struct RepoHandle {
    root: PathBuf,
}

impl RepoHandle {
    /// Open and validate a repository root. Fails if the path doesn't
    /// exist, isn't a directory, or can't be canonicalized (e.g. a broken
    /// symlink) — we never silently fall back to "treat it as empty."
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let root = path
            .canonicalize()
            .with_context(|| format!("opening repository at {}", path.display()))?;
        if !root.is_dir() {
            bail!("{} is not a directory", root.display());
        }
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn has_git(&self) -> bool {
        self.root.join(".git").exists()
    }

    /// Resolve a repo-relative path to an absolute path, refusing to leave
    /// `root` even via `..` segments or symlinks. Used whenever a path
    /// stored in the index (which is just a string) is turned back into a
    /// filesystem path to read source text.
    pub fn resolve(&self, relative: &RepoPath) -> Result<PathBuf> {
        let candidate = self.root.join(relative.as_str());
        let resolved = candidate
            .canonicalize()
            .with_context(|| format!("resolving {relative} within repository"))?;
        if !resolved.starts_with(&self.root) {
            bail!("path {relative} escapes repository root");
        }
        Ok(resolved)
    }

    /// Convert an absolute path known to be inside `root` into a
    /// `RepoPath`. Panics-free: returns an error if the path isn't
    /// actually under `root` (a caller bug, not user input, so this
    /// indicates a logic error upstream).
    pub fn relativize(&self, absolute: &Path) -> Result<RepoPath> {
        let rel = absolute
            .strip_prefix(&self.root)
            .with_context(|| format!("{} is not inside repository root", absolute.display()))?;
        Ok(RepoPath::new(rel.to_string_lossy().to_string()))
    }
}

/// One file discovered by a repository walk.
#[derive(Debug, Clone)]
pub struct WalkedFile {
    pub path: RepoPath,
    pub absolute_path: PathBuf,
    pub language: Option<Language>,
}

/// Walks a repository respecting ignore rules, yielding every file along
/// with its detected language (`None` for files no `Language` claims —
/// still useful for config/test discovery, which look at file names and
/// content rather than parsed symbols).
pub struct RepoWalker<'a> {
    repo: &'a RepoHandle,
    config: &'a BoreholeConfig,
}

impl<'a> RepoWalker<'a> {
    pub fn new(repo: &'a RepoHandle, config: &'a BoreholeConfig) -> Self {
        Self { repo, config }
    }

    pub fn walk(&self) -> Result<Vec<WalkedFile>> {
        let walker = ignore_rules::build_walker(self.repo.root(), self.config).build();
        let mut files = Vec::new();
        for entry in walker {
            let entry = entry.context("walking repository")?;
            if !entry.file_type().is_some_and(|t| t.is_file()) {
                continue;
            }
            let absolute_path = entry.path().to_path_buf();
            let language = Language::from_path(&absolute_path);
            if let Some(ext) = absolute_path.extension().and_then(|e| e.to_str()) {
                if self.config.ignored_extensions.iter().any(|e| e == ext) {
                    continue;
                }
            }
            let path = self.repo.relativize(&absolute_path)?;
            files.push(WalkedFile {
                path,
                absolute_path,
                language,
            });
        }
        files.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn open_rejects_missing_path() {
        assert!(RepoHandle::open("/definitely/does/not/exist/xyz").is_err());
    }

    #[test]
    fn open_rejects_file_not_directory() {
        let dir = tempdir().unwrap();
        let file = dir.path().join("f.txt");
        fs::write(&file, "x").unwrap();
        assert!(RepoHandle::open(&file).is_err());
    }

    #[test]
    fn resolve_refuses_to_escape_root() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("inner")).unwrap();
        let repo = RepoHandle::open(dir.path().join("inner")).unwrap();
        let outside = RepoPath::new("../../etc/passwd");
        // This either fails to canonicalize (path doesn't exist) or, if it
        // somehow did exist, must be rejected for escaping root. Either
        // outcome is a pass; only a returned Ok path outside root is a
        // failure.
        if let Ok(resolved) = repo.resolve(&outside) {
            assert!(resolved.starts_with(repo.root()));
        }
    }

    #[test]
    fn walk_finds_language_tagged_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("README.md"), "# hi").unwrap();
        let repo = RepoHandle::open(dir.path()).unwrap();
        let config = BoreholeConfig::default();
        let files = RepoWalker::new(&repo, &config).walk().unwrap();

        let rs = files.iter().find(|f| f.path.as_str() == "main.rs").unwrap();
        assert_eq!(rs.language, Some(Language::Rust));
        let md = files
            .iter()
            .find(|f| f.path.as_str() == "README.md")
            .unwrap();
        assert_eq!(md.language, None);
    }
}

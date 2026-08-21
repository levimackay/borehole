//! Git history mining via git2 (libgit2 bindings) — never the `git` binary
//! (brief section 23: shelling out to a repo-controlled `git` wrapper or
//! hook script is a command-injection surface; git2 talks to the on-disk
//! object database directly and has no shell in the loop).
//!
//! The one carve-out: `#[cfg(test)]` code in this file is allowed to shell
//! out to the real `git` binary via `std::process::Command`, purely to build
//! deterministic fixture repositories to test against. That code never ships
//! — it's compiled only when running `cargo test`, is never reachable from
//! the shipped library surface, and never touches a user-supplied repo path
//! (it only ever operates on a `tempfile::tempdir()` this test created).
//!
//! CONTRACT (frozen — see docs/ARCHITECTURE.md "Crate contracts"). Consumed
//! by `engine-evidence`.

use engine_core::{Confidence, RepoPath, Span};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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

/// One commit that matched the tracked path during a backward walk, plus
/// enough context to (a) report it as a `CommitInfo`, (b) intersect its
/// diff hunks against a symbol span, and (c) enumerate the other files it
/// touched for temporal coupling. Kept private — never part of the frozen
/// public contract.
struct WalkEntry<'repo> {
    info: CommitInfo,
    /// The full commit diff (this commit vs. its first parent, or vs. the
    /// empty tree for a root commit), with rename detection already run.
    /// "Full" meaning not scoped to the tracked path — `files_changed` and
    /// temporal coupling both need the complete file list.
    diff: git2::Diff<'repo>,
    /// Index into `diff.deltas()` of the delta that matched the tracked
    /// path at this point in the walk.
    delta_idx: usize,
    /// The path we were looking for when we found this commit — i.e. the
    /// tracked file's name *as of this commit's tree*. Renames change this
    /// for older (parent-ward) commits.
    tracked_path: String,
    /// True if the matched delta was a rename, meaning commits older than
    /// this one are being searched for under a *different* name than the
    /// one the caller originally asked about.
    renamed_from_here: bool,
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
        let entries = self.walk_tracked_file(path, limit)?;
        Ok(entries.into_iter().map(|e| e.info).collect())
    }

    /// Best-effort history scoped to one symbol's byte span within its
    /// current file, by intersecting each commit's diff hunks against the
    /// span. See [`SymbolHistory::confidence`] for the honesty contract.
    ///
    /// Assumption (undocumented elsewhere since `engine-parse` doesn't
    /// exist yet to cross-check against): `Span::start_line`/`end_line`
    /// are 1-indexed "to match editor and git blame conventions"
    /// (engine-core doc comment), and git2's `DiffHunk::old_start()` /
    /// `new_start()` are also 1-indexed line numbers. So comparing them
    /// directly, with no +1/-1 translation, is correct — verified against
    /// git2's own docs (`git_diff_hunk.new_start`: "Starting line number in
    /// new_file"), which match libgit2's/git's 1-indexed hunk header
    /// convention (`@@ -a,b +c,d @@` where `c` is 1-indexed).
    pub fn symbol_history(
        &self,
        path: &RepoPath,
        span: Span,
        limit: Option<u32>,
    ) -> Result<SymbolHistory> {
        let entries = self.walk_tracked_file(path, limit)?;

        let mut commits = Vec::new();
        let mut rename_crossed = false;
        let mut any_rename_in_window = false;

        for entry in &entries {
            if !rename_crossed {
                if Self::hunk_intersects_span(&entry.diff, entry.delta_idx, span)? {
                    commits.push(entry.info.clone());
                }
            } else {
                // Pre-rename portion of the window: we can no longer trust
                // that line numbers in the old file line up with `span`
                // (which was computed against the current file), so fall
                // back to whole-file attribution rather than guessing.
                commits.push(entry.info.clone());
            }

            if entry.renamed_from_here {
                rename_crossed = true;
                any_rename_in_window = true;
            }
        }

        let confidence = if any_rename_in_window {
            Confidence::Low
        } else {
            Confidence::High
        };

        Ok(SymbolHistory {
            commits,
            confidence,
        })
    }

    /// Files that changed together with `path` across history — a raw
    /// correlation signal (brief section 17), not a claim of architectural
    /// dependency; that framing belongs in `engine-evidence`'s presentation
    /// layer, not here.
    ///
    /// `total_commits_either_file` is computed as
    /// `|commits touching path| + |commits touching other| - |co-changed
    /// commits|` (inclusion-exclusion). Both commit counts are bounded by
    /// the same `limit` as the primary walk (or unbounded if `limit` is
    /// `None`) — this keeps the cost of this method to at most
    /// `1 + N` bounded history walks, where N is the number of distinct
    /// other files ever touched alongside `path` within the window,
    /// instead of an unbounded full-repo scan. The tradeoff: if `other`
    /// has a long independent history and `limit` is small, some
    /// co-changed commits found via `path`'s window might fall outside
    /// `other`'s own `limit`-bounded window, which would very slightly
    /// skew the ratio. Documented, accepted simplification — see
    /// `ARCHITECTURE.md` "Confidence is not optional" (this is a
    /// correlation heuristic, not an exact accounting).
    ///
    /// Renames of the *other* file (not `path`) are not followed across
    /// this join — a file that was renamed mid-window is treated as two
    /// distinct co-change partners, one per name it held. Only `path`
    /// itself gets full rename-following treatment.
    pub fn temporal_coupling(
        &self,
        path: &RepoPath,
        limit: Option<u32>,
    ) -> Result<Vec<CoChangeEdge>> {
        let entries = self.walk_tracked_file(path, limit)?;
        if entries.is_empty() {
            return Ok(Vec::new());
        }
        let path_commit_count = entries.len() as u32;

        let mut co_counts: HashMap<String, u32> = HashMap::new();
        for entry in &entries {
            for other in Self::other_files_touched(&entry.diff, &entry.tracked_path) {
                *co_counts.entry(other).or_insert(0) += 1;
            }
        }

        let mut result = Vec::with_capacity(co_counts.len());
        for (other_file, co_count) in co_counts {
            let other_repo_path = RepoPath::new(other_file.clone());
            let other_entries = self.walk_tracked_file(&other_repo_path, limit)?;
            let other_count = other_entries.len() as u32;
            // co_count is bounded above by min(path_commit_count, other_count)
            // in the common case; saturating_sub guards the documented edge
            // case above where windowing can make that not quite hold.
            let total = path_commit_count
                .saturating_add(other_count)
                .saturating_sub(co_count);
            let ratio = if total > 0 {
                co_count as f32 / total as f32
            } else {
                0.0
            };
            result.push(CoChangeEdge {
                file: RepoPath::new(other_file),
                co_change_count: co_count,
                total_commits_either_file: total,
                ratio,
            });
        }

        result.sort_by(|a, b| b.ratio.total_cmp(&a.ratio));
        Ok(result)
    }

    /// Oldest commit in `path`'s history (following renames all the way
    /// back to the commit that first created the file at its current
    /// name's origin). `Ok(None)` when the file has no history — including
    /// when the repository has no commits at all.
    ///
    /// `limit` bounds the walk exactly like `file_history`'s — pass `None`
    /// only when the caller has already reasoned about the walk being
    /// bounded some other way (e.g. a genuinely small file), since an
    /// unbounded walk here costs one full first-parent history traversal
    /// with rename detection. This parameter was added (this method has no
    /// callers yet, so widening it here is a zero-risk contract update
    /// rather than a breaking change) after a security review found the
    /// original no-limit-parameter signature had no way to be called
    /// safely from a caller with an untrusted/unbounded repository.
    pub fn introduced_commit(
        &self,
        path: &RepoPath,
        limit: Option<u32>,
    ) -> Result<Option<CommitInfo>> {
        let entries = self.walk_tracked_file(path, limit)?;
        Ok(entries.into_iter().next_back().map(|e| e.info))
    }

    /// Shared walk used by all four public methods. Walks first-parent
    /// history from HEAD (see module-level note on merge commits below),
    /// tracking the tracked file's name backward through renames, and
    /// collecting an entry for every commit whose diff touches the
    /// currently-tracked name. Stops as soon as `limit` matches have been
    /// collected — the revwalk itself is a lazy iterator and we `break`
    /// out of it, so a `limit` of e.g. 20 on a 50,000-commit repo does not
    /// walk the full 50,000.
    ///
    /// Known limitation: merge commits are diffed against their first
    /// parent only (`Revwalk::simplify_first_parent`), matching `git log
    /// --first-parent` semantics rather than full TREESAME-across-all-
    /// parents tracking. This is a documented, accepted simplification
    /// (see brief section on merge handling) — a change that only entered
    /// history via a non-first-parent merge side branch will not surface
    /// here.
    ///
    /// Empty-repo handling: if the repository has no commits at all
    /// (`repo.head()` fails because HEAD is unborn), this returns an empty
    /// `Vec` rather than an error. That choice is applied consistently
    /// across all four public methods (they all route through this
    /// helper) so callers never need to special-case "brand new repo" —
    /// it just looks like "no history yet," which is the useful behavior
    /// for a UI trying to render a history panel.
    fn walk_tracked_file(&self, path: &RepoPath, limit: Option<u32>) -> Result<Vec<WalkEntry<'_>>> {
        let mut results = Vec::new();

        let head_oid = match self.repo.head() {
            Ok(head) => head.target(),
            Err(_) => None, // unborn HEAD: repo has zero commits
        };
        let Some(head_oid) = head_oid else {
            return Ok(results);
        };

        let mut revwalk = self.repo.revwalk()?;
        revwalk.push(head_oid)?;
        revwalk.set_sorting(git2::Sort::TOPOLOGICAL | git2::Sort::TIME)?;
        revwalk.simplify_first_parent()?;

        let limit = limit.map(|l| l as usize);
        let mut tracked_path = path.as_str().to_string();

        for oid in revwalk {
            if let Some(l) = limit {
                if results.len() >= l {
                    break;
                }
            }
            let oid = oid?;
            let commit = self.repo.find_commit(oid)?;
            let diff = self.diff_for_commit(&commit)?;

            let matched_idx = diff.deltas().position(|delta| {
                let new_matches = delta
                    .new_file()
                    .path()
                    .and_then(|p| p.to_str())
                    .is_some_and(|p| p == tracked_path);
                let old_matches = delta
                    .old_file()
                    .path()
                    .and_then(|p| p.to_str())
                    .is_some_and(|p| p == tracked_path);
                new_matches || old_matches
            });

            let Some(idx) = matched_idx else {
                // This commit doesn't touch the currently-tracked name;
                // keep walking backward with the same name.
                continue;
            };

            let delta = diff
                .get_delta(idx)
                .expect("idx came from this diff's deltas");
            let files_changed = diff.deltas().len() as u32;
            let info = Self::commit_info_from(&commit, files_changed);
            let status = delta.status();

            let mut renamed_from_here = false;
            let mut next_tracked = tracked_path.clone();
            if status == git2::Delta::Renamed {
                if let Some(old_str) = delta.old_file().path().and_then(|p| p.to_str()) {
                    if old_str != tracked_path {
                        next_tracked = old_str.to_string();
                        renamed_from_here = true;
                    }
                }
            }

            let stop_here = status == git2::Delta::Added;

            results.push(WalkEntry {
                info,
                diff,
                delta_idx: idx,
                tracked_path: tracked_path.clone(),
                renamed_from_here,
            });

            if stop_here {
                // The file was created here — either genuinely new, or
                // re-created at this path after an earlier deletion. Either
                // way there is no older history to link it to: a
                // delete-then-recreate at the same path must not be walked
                // through as if it were one continuous file history.
                break;
            }

            tracked_path = next_tracked;
        }

        Ok(results)
    }

    /// Diff a commit against its first parent (or the empty tree, for a
    /// root commit), with rename detection enabled.
    fn diff_for_commit(&self, commit: &git2::Commit) -> Result<git2::Diff<'_>> {
        let tree = commit.tree()?;
        let mut diff_opts = git2::DiffOptions::new();
        let mut diff = if commit.parent_count() == 0 {
            self.repo
                .diff_tree_to_tree(None, Some(&tree), Some(&mut diff_opts))?
        } else {
            let parent = commit.parent(0)?;
            let parent_tree = parent.tree()?;
            self.repo
                .diff_tree_to_tree(Some(&parent_tree), Some(&tree), Some(&mut diff_opts))?
        };
        let mut find_opts = git2::DiffFindOptions::new();
        find_opts.renames(true);
        diff.find_similar(Some(&mut find_opts))?;
        Ok(diff)
    }

    fn commit_info_from(commit: &git2::Commit, files_changed: u32) -> CommitInfo {
        let author = commit.author();
        CommitInfo {
            sha: commit.id().to_string(),
            author_name: author.name().unwrap_or_default().to_string(),
            author_email: author.email().unwrap_or_default().to_string(),
            // Author time (not committer time) to stay consistent with
            // author_name/author_email above.
            timestamp: author.when().seconds(),
            // git2 0.21 changed `summary()` from `Option<&str>` to
            // `Result<Option<&str>, Error>` (surfacing invalid-UTF-8 as an
            // error rather than lossily converting). `.ok()` treats that
            // error the same way the rest of this function already treats
            // a missing summary — falls through to the empty-string
            // default rather than failing the whole commit lookup over one
            // unreadable message.
            summary: commit
                .summary()
                .ok()
                .flatten()
                .unwrap_or_default()
                .to_string(),
            files_changed,
        }
    }

    /// Whether any hunk in the delta at `delta_idx` overlaps `span`'s
    /// `[start_line, end_line]`. Uses the new-side (post-change) line
    /// range normally; falls back to the old-side range when the file was
    /// deleted by this commit (there is no new side to speak of).
    fn hunk_intersects_span(diff: &git2::Diff, delta_idx: usize, span: Span) -> Result<bool> {
        let delta = diff
            .get_delta(delta_idx)
            .expect("delta_idx came from this diff's deltas");
        let use_old_side = delta.status() == git2::Delta::Deleted;

        let Some(patch) = git2::Patch::from_diff(diff, delta_idx)? else {
            return Ok(false);
        };

        for h in 0..patch.num_hunks() {
            let (hunk, _lines) = patch.hunk(h)?;
            let (start, len) = if use_old_side {
                (hunk.old_start(), hunk.old_lines())
            } else {
                (hunk.new_start(), hunk.new_lines())
            };
            if len == 0 {
                continue;
            }
            let end = start + len - 1;
            if start <= span.end_line && span.start_line <= end {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Every file path (other than `tracked_path` itself) touched by a
    /// full commit diff, one entry per delta.
    fn other_files_touched(diff: &git2::Diff, tracked_path: &str) -> Vec<String> {
        let mut out = Vec::new();
        for delta in diff.deltas() {
            let new_p = delta.new_file().path().and_then(|p| p.to_str());
            let old_p = delta.old_file().path().and_then(|p| p.to_str());
            if new_p == Some(tracked_path) || old_p == Some(tracked_path) {
                continue;
            }
            // Prefer the new-side path (post-change name); fall back to the
            // old side for a pure deletion, which has no new side.
            if let Some(p) = new_p.or(old_p) {
                out.push(p.to_string());
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    // ---- test-only fixture scaffolding -----------------------------------
    // Shells out to the real `git` binary to build deterministic fixture
    // repositories. This is the carve-out documented at the top of this
    // file: test-only code, never compiled into the shipped library, never
    // fed anything but a tempdir this test created.

    fn run_git(dir: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(dir)
            // Hermetic: never read the host's global/system git config.
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test Author")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .status()
            .expect("failed to spawn git");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo() -> TempDir {
        let dir = TempDir::new().expect("tempdir");
        run_git(dir.path(), &["-c", "init.defaultBranch=main", "init", "-q"]);
        dir
    }

    fn write_file(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, contents).unwrap();
    }

    fn git_add(dir: &Path, rel: &str) {
        run_git(dir, &["add", rel]);
    }

    fn git_mv(dir: &Path, from: &str, to: &str) {
        run_git(dir, &["mv", from, to]);
    }

    fn git_rm(dir: &Path, rel: &str) {
        run_git(dir, &["rm", "-q", rel]);
    }

    /// Commit with fixed, caller-chosen author/committer dates so ordering
    /// and every other test assertion is fully deterministic regardless of
    /// wall-clock time.
    fn git_commit(dir: &Path, message: &str, epoch_seconds: i64) -> String {
        let date = format!("{epoch_seconds} +0000");
        let status = Command::new("git")
            .args(["commit", "-q", "-m", message])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .env("GIT_AUTHOR_NAME", "Test Author")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test Author")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .env("GIT_AUTHOR_DATE", &date)
            .env("GIT_COMMITTER_DATE", &date)
            .status()
            .expect("failed to spawn git commit");
        assert!(status.success(), "git commit failed");

        let out = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(dir)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .expect("failed to spawn git rev-parse");
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_string()
    }

    fn open(dir: &TempDir) -> GitAnalyzer {
        GitAnalyzer::open(dir.path()).expect("open fixture repo")
    }

    // ---- fixtures -----------------------------------------------------

    #[test]
    fn linear_history_single_file() {
        let dir = init_repo();
        write_file(dir.path(), "a.txt", "v1\n");
        git_add(dir.path(), "a.txt");
        let c1 = git_commit(dir.path(), "add a.txt", 1_700_000_000);

        write_file(dir.path(), "a.txt", "v2\n");
        git_add(dir.path(), "a.txt");
        let c2 = git_commit(dir.path(), "update a.txt to v2", 1_700_000_100);

        write_file(dir.path(), "a.txt", "v3\n");
        git_add(dir.path(), "a.txt");
        let c3 = git_commit(dir.path(), "update a.txt to v3", 1_700_000_200);

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("a.txt"), None)
            .unwrap();

        let shas: Vec<&str> = history.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(shas, vec![c3.as_str(), c2.as_str(), c1.as_str()]);
        assert_eq!(history[0].summary, "update a.txt to v3");
        assert_eq!(history[2].summary, "add a.txt");
        for c in &history {
            assert_eq!(c.files_changed, 1);
            assert_eq!(c.author_name, "Test Author");
        }
        // most-recent-first ordering by timestamp
        assert!(history[0].timestamp > history[1].timestamp);
        assert!(history[1].timestamp > history[2].timestamp);
    }

    #[test]
    fn single_rename_is_followed() {
        let dir = init_repo();
        write_file(dir.path(), "a.txt", "v1\n");
        git_add(dir.path(), "a.txt");
        let c1 = git_commit(dir.path(), "add a.txt", 1_700_000_000);

        git_mv(dir.path(), "a.txt", "b.txt");
        let c2 = git_commit(dir.path(), "rename a.txt to b.txt", 1_700_000_100);

        write_file(dir.path(), "b.txt", "v2\n");
        git_add(dir.path(), "b.txt");
        let c3 = git_commit(dir.path(), "update b.txt", 1_700_000_200);

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("b.txt"), None)
            .unwrap();

        let shas: Vec<&str> = history.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(shas, vec![c3.as_str(), c2.as_str(), c1.as_str()]);
    }

    #[test]
    fn double_rename_is_followed() {
        let dir = init_repo();
        write_file(dir.path(), "a.txt", "v1\n");
        git_add(dir.path(), "a.txt");
        let c1 = git_commit(dir.path(), "add a.txt", 1_700_000_000);

        git_mv(dir.path(), "a.txt", "b.txt");
        let c2 = git_commit(dir.path(), "rename a.txt to b.txt", 1_700_000_100);

        git_mv(dir.path(), "b.txt", "c.txt");
        let c3 = git_commit(dir.path(), "rename b.txt to c.txt", 1_700_000_200);

        write_file(dir.path(), "c.txt", "v2\n");
        git_add(dir.path(), "c.txt");
        let c4 = git_commit(dir.path(), "update c.txt", 1_700_000_300);

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("c.txt"), None)
            .unwrap();

        let shas: Vec<&str> = history.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(
            shas,
            vec![c4.as_str(), c3.as_str(), c2.as_str(), c1.as_str()]
        );

        let introduced = analyzer
            .introduced_commit(&RepoPath::new("c.txt"), None)
            .unwrap()
            .unwrap();
        assert_eq!(introduced.sha, c1);
    }

    #[test]
    fn delete_then_recreate_is_a_new_history() {
        let dir = init_repo();
        write_file(dir.path(), "a.txt", "old-v1\n");
        git_add(dir.path(), "a.txt");
        let _c1 = git_commit(dir.path(), "add old a.txt", 1_700_000_000);

        write_file(dir.path(), "a.txt", "old-v2\n");
        git_add(dir.path(), "a.txt");
        let _c2 = git_commit(dir.path(), "update old a.txt", 1_700_000_100);

        git_rm(dir.path(), "a.txt");
        let _c3 = git_commit(dir.path(), "delete a.txt", 1_700_000_200);

        // unrelated commit touching a different file, so the deletion isn't
        // the tip of history
        write_file(dir.path(), "other.txt", "x\n");
        git_add(dir.path(), "other.txt");
        let _c_other = git_commit(dir.path(), "unrelated change", 1_700_000_250);

        write_file(dir.path(), "a.txt", "new-v1\n");
        git_add(dir.path(), "a.txt");
        let c5 = git_commit(dir.path(), "recreate a.txt", 1_700_000_300);

        write_file(dir.path(), "a.txt", "new-v2\n");
        git_add(dir.path(), "a.txt");
        let c6 = git_commit(dir.path(), "update recreated a.txt", 1_700_000_400);

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("a.txt"), None)
            .unwrap();

        let shas: Vec<&str> = history.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(
            shas,
            vec![c6.as_str(), c5.as_str()],
            "history must not walk through the delete/recreate boundary"
        );

        let introduced = analyzer
            .introduced_commit(&RepoPath::new("a.txt"), None)
            .unwrap()
            .unwrap();
        assert_eq!(introduced.sha, c5);
    }

    #[test]
    fn limit_bounds_the_walk_and_keeps_most_recent_first() {
        let dir = init_repo();
        let mut shas = Vec::new();
        for i in 0..5 {
            write_file(dir.path(), "a.txt", &format!("v{i}\n"));
            git_add(dir.path(), "a.txt");
            let sha = git_commit(dir.path(), &format!("update {i}"), 1_700_000_000 + i);
            shas.push(sha);
        }
        shas.reverse(); // most recent first

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("a.txt"), Some(2))
            .unwrap();

        assert_eq!(history.len(), 2);
        let got: Vec<&str> = history.iter().map(|c| c.sha.as_str()).collect();
        assert_eq!(got, vec![shas[0].as_str(), shas[1].as_str()]);
    }

    #[test]
    fn nonexistent_path_returns_empty_history_not_error() {
        let dir = init_repo();
        write_file(dir.path(), "a.txt", "v1\n");
        git_add(dir.path(), "a.txt");
        git_commit(dir.path(), "add a.txt", 1_700_000_000);

        let analyzer = open(&dir);
        let history = analyzer
            .file_history(&RepoPath::new("never-existed.txt"), None)
            .unwrap();
        assert!(history.is_empty());

        let introduced = analyzer
            .introduced_commit(&RepoPath::new("never-existed.txt"), None)
            .unwrap();
        assert!(introduced.is_none());

        let coupling = analyzer
            .temporal_coupling(&RepoPath::new("never-existed.txt"), None)
            .unwrap();
        assert!(coupling.is_empty());
    }

    #[test]
    fn truly_empty_repo_never_panics() {
        let dir = init_repo(); // git init, zero commits, unborn HEAD
        let analyzer = open(&dir);

        let history = analyzer
            .file_history(&RepoPath::new("a.txt"), None)
            .unwrap();
        assert!(history.is_empty());

        let introduced = analyzer
            .introduced_commit(&RepoPath::new("a.txt"), None)
            .unwrap();
        assert!(introduced.is_none());

        let coupling = analyzer
            .temporal_coupling(&RepoPath::new("a.txt"), None)
            .unwrap();
        assert!(coupling.is_empty());

        let span = Span {
            start_line: 1,
            start_col: 1,
            end_line: 1,
            end_col: 1,
            start_byte: 0,
            end_byte: 0,
        };
        let symbols = analyzer
            .symbol_history(&RepoPath::new("a.txt"), span, None)
            .unwrap();
        assert!(symbols.commits.is_empty());
    }

    #[test]
    fn temporal_coupling_hand_computed_ratio() {
        let dir = init_repo();

        // c1: a.txt + b.txt together
        write_file(dir.path(), "a.txt", "1\n");
        write_file(dir.path(), "b.txt", "1\n");
        git_add(dir.path(), "a.txt");
        git_add(dir.path(), "b.txt");
        git_commit(dir.path(), "add a and b", 1_700_000_000);

        // c2: a.txt + b.txt together again
        write_file(dir.path(), "a.txt", "2\n");
        write_file(dir.path(), "b.txt", "2\n");
        git_add(dir.path(), "a.txt");
        git_add(dir.path(), "b.txt");
        git_commit(dir.path(), "update a and b", 1_700_000_100);

        // c3: a.txt alone
        write_file(dir.path(), "a.txt", "3\n");
        git_add(dir.path(), "a.txt");
        git_commit(dir.path(), "update a alone", 1_700_000_200);

        // c4: c.txt alone (unrelated, never touches a.txt or b.txt)
        write_file(dir.path(), "c.txt", "1\n");
        git_add(dir.path(), "c.txt");
        git_commit(dir.path(), "add c alone", 1_700_000_300);

        // a.txt commits: c1, c2, c3 => 3
        // b.txt commits: c1, c2 => 2
        // co-change (a & b together): c1, c2 => 2
        // total_either = 3 + 2 - 2 = 3
        // ratio = 2/3
        let analyzer = open(&dir);
        let coupling = analyzer
            .temporal_coupling(&RepoPath::new("a.txt"), None)
            .unwrap();

        assert_eq!(coupling.len(), 1, "only b.txt co-changed with a.txt");
        let edge = &coupling[0];
        assert_eq!(edge.file.as_str(), "b.txt");
        assert_eq!(edge.co_change_count, 2);
        assert_eq!(edge.total_commits_either_file, 3);
        assert!((edge.ratio - (2.0 / 3.0)).abs() < 1e-6);
    }

    #[test]
    fn symbol_history_line_range_intersection() {
        let dir = init_repo();

        let base: Vec<String> = (1..=12).map(|n| format!("line{n}")).collect();
        write_file(dir.path(), "f.txt", &format!("{}\n", base.join("\n")));
        git_add(dir.path(), "f.txt");
        let c1 = git_commit(dir.path(), "create f.txt", 1_700_000_000);

        // Change inside the tracked span (lines 2-4): edit line 3.
        let mut inside = base.clone();
        inside[2] = "line3-changed".to_string();
        write_file(dir.path(), "f.txt", &format!("{}\n", inside.join("\n")));
        git_add(dir.path(), "f.txt");
        let c2 = git_commit(dir.path(), "edit inside span", 1_700_000_100);

        // Change well outside the span: edit line 10.
        let mut outside = inside.clone();
        outside[9] = "line10-changed".to_string();
        write_file(dir.path(), "f.txt", &format!("{}\n", outside.join("\n")));
        git_add(dir.path(), "f.txt");
        let c3 = git_commit(dir.path(), "edit outside span", 1_700_000_200);

        let analyzer = open(&dir);
        let span = Span {
            start_line: 2,
            start_col: 1,
            end_line: 4,
            end_col: 1,
            start_byte: 0,
            end_byte: 0,
        };
        let history = analyzer
            .symbol_history(&RepoPath::new("f.txt"), span, None)
            .unwrap();

        assert_eq!(history.confidence, Confidence::High);
        let shas: Vec<&str> = history.commits.iter().map(|c| c.sha.as_str()).collect();
        assert!(
            shas.contains(&c1.as_str()),
            "creation commit touches the span"
        );
        assert!(
            shas.contains(&c2.as_str()),
            "edit inside span must be included"
        );
        assert!(
            !shas.contains(&c3.as_str()),
            "edit outside span must be excluded"
        );
    }

    #[test]
    fn symbol_history_low_confidence_across_rename() {
        let dir = init_repo();
        let base: Vec<String> = (1..=12).map(|n| format!("line{n}")).collect();
        write_file(dir.path(), "a.txt", &format!("{}\n", base.join("\n")));
        git_add(dir.path(), "a.txt");
        git_commit(dir.path(), "create a.txt", 1_700_000_000);

        git_mv(dir.path(), "a.txt", "b.txt");
        git_commit(dir.path(), "rename a.txt to b.txt", 1_700_000_100);

        let mut edited = base.clone();
        edited[2] = "line3-changed".to_string();
        write_file(dir.path(), "b.txt", &format!("{}\n", edited.join("\n")));
        git_add(dir.path(), "b.txt");
        let c3 = git_commit(dir.path(), "edit b.txt after rename", 1_700_000_200);

        let analyzer = open(&dir);
        let span = Span {
            start_line: 2,
            start_col: 1,
            end_line: 4,
            end_col: 1,
            start_byte: 0,
            end_byte: 0,
        };
        let history = analyzer
            .symbol_history(&RepoPath::new("b.txt"), span, None)
            .unwrap();

        assert_eq!(history.confidence, Confidence::Low);
        // Never silently drops to empty just because confidence is low.
        assert!(!history.commits.is_empty());
        let shas: Vec<&str> = history.commits.iter().map(|c| c.sha.as_str()).collect();
        assert!(shas.contains(&c3.as_str()));
    }
}

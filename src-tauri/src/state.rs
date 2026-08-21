use engine_core::{BoreholeConfig, RepoHandle};
use engine_git::GitAnalyzer;
use engine_index::Index;
use std::sync::Mutex;

/// Everything needed to answer a query about one open repository. Held in
/// Tauri's managed state behind a mutex — this app opens one repository at
/// a time in v1 (multi-tab/multi-repo is a documented roadmap item, not a
/// v1 requirement per the brief).
pub struct OpenRepo {
    pub repo: RepoHandle,
    pub index: Index,
    /// `None` when the opened directory has no usable `.git` — see
    /// `engine_evidence::EvidenceEngine`'s doc comment for how git-derived
    /// evidence degrades in that case.
    pub git: Option<GitAnalyzer>,
    pub config: BoreholeConfig,
}

#[derive(Default)]
pub struct AppState {
    pub open_repo: Mutex<Option<OpenRepo>>,
}

impl OpenRepo {
    pub fn evidence(&self) -> engine_evidence::EvidenceEngine<'_> {
        engine_evidence::EvidenceEngine::new(
            &self.index,
            self.git.as_ref(),
            &self.repo,
            &self.config,
        )
    }
}

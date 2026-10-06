//! Application-wide state managed by Tauri.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::git::error::GitError;
use crate::git::exec::GitBinary;
use crate::repo_manager::RepoManager;

/// Shared state injected into commands via `tauri::State`.
pub struct AppState {
    /// Root of the bundled MinGit (Windows release builds), if present.
    pub bundled_git_dir: Option<PathBuf>,
    /// Set by `check_git` once a supported git is found.
    git: RwLock<Option<GitBinary>>,
    pub repos: RepoManager,
}

impl AppState {
    /// Creates state with no git resolved yet.
    pub fn new(bundled_git_dir: Option<PathBuf>, repos: RepoManager) -> Self {
        Self {
            bundled_git_dir,
            git: RwLock::new(None),
            repos,
        }
    }

    /// The git binary verified at startup.
    pub fn git(&self) -> Result<GitBinary, GitError> {
        self.git
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
            .ok_or(GitError::NotFound {
                searched: Vec::new(),
            })
    }

    /// Records the git binary after a successful version check.
    pub fn set_git(&self, git: GitBinary) {
        *self.git.write().unwrap_or_else(|p| p.into_inner()) = Some(git);
    }
}

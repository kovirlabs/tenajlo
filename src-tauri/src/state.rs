//! Application-wide state managed by Tauri.

use std::path::PathBuf;
use std::sync::RwLock;

use crate::auth::accounts::AccountManager;
use crate::auth::broker::PromptBroker;
use crate::auth::remote_auth::{self, AuthRequest, PrepareError, RemoteAuth};
use crate::auth::trampoline::Trampoline;
use crate::forgejo::ForgejoClient;
use crate::git::error::GitError;
use crate::git::exec::GitBinary;
use crate::operations::Operations;
use crate::repo_manager::RepoManager;
use crate::settings::SettingsManager;
use crate::watcher::RepoWatcher;

/// Shared state injected into commands via `tauri::State`.
pub struct AppState {
    /// Root of the bundled MinGit (Windows release builds), if present.
    pub bundled_git_dir: Option<PathBuf>,
    /// Set by `check_git` once a supported git is found.
    git: RwLock<Option<GitBinary>>,
    pub repos: RepoManager,
    pub accounts: AccountManager,
    pub settings: SettingsManager,
    pub forgejo: ForgejoClient,
    pub watcher: RepoWatcher,
    pub operations: Operations,
    pub prompts: PromptBroker,
    /// Askpass server; `None` if it couldn't start (remote ops then can't prompt).
    pub trampoline: Option<Trampoline>,
    /// The `tenajlo-askpass` sidecar; `None` if missing.
    pub askpass: Option<PathBuf>,
}

impl AppState {
    /// Creates state with no git resolved yet.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bundled_git_dir: Option<PathBuf>,
        repos: RepoManager,
        accounts: AccountManager,
        settings: SettingsManager,
        forgejo: ForgejoClient,
        prompts: PromptBroker,
        trampoline: Option<Trampoline>,
        askpass: Option<PathBuf>,
    ) -> Self {
        Self {
            bundled_git_dir,
            git: RwLock::new(None),
            repos,
            accounts,
            settings,
            forgejo,
            watcher: RepoWatcher::default(),
            operations: Operations::default(),
            prompts,
            trampoline,
            askpass,
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

    /// Credentials for one remote operation (spec §6.2). Keep the result alive until git exits.
    pub async fn prepare_auth(&self, request: AuthRequest<'_>) -> Result<RemoteAuth, PrepareError> {
        remote_auth::prepare(
            &self.accounts,
            self.trampoline.as_ref(),
            self.askpass.as_deref(),
            request,
        )
        .await
    }

    /// Records the git binary after a successful version check.
    pub fn set_git(&self, git: GitBinary) {
        *self.git.write().unwrap_or_else(|p| p.into_inner()) = Some(git);
    }
}

//! Application-wide state managed by Tauri.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use uuid::Uuid;

use crate::auth::accounts::AccountManager;
use crate::auth::broker::PromptBroker;
use crate::auth::remote_auth::{self, AuthRequest, PrepareError, RemoteAuth};
use crate::auth::saved_secrets::SavedSecrets;
use crate::auth::trampoline::Trampoline;
use crate::forgejo::ForgejoClient;
use crate::git::error::GitError;
use crate::git::exec::GitBinary;
use crate::operations::Operations;
use crate::repo_links::RepoLinks;
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
    /// Passwords and SSH passphrases the user chose to remember.
    pub saved_secrets: Arc<SavedSecrets>,
    pub settings: SettingsManager,
    pub forgejo: ForgejoClient,
    pub watcher: RepoWatcher,
    pub operations: Operations,
    pub prompts: PromptBroker,
    /// Askpass server; `None` if it couldn't start (remote ops then can't prompt).
    pub trampoline: Option<Trampoline>,
    /// The `tenajlo-askpass` sidecar; `None` if missing.
    pub askpass: Option<PathBuf>,
    pub lock_support: LockSupport,
    /// The Forgejo repository behind each open repository's remote (v1.4 links).
    pub repo_links: RepoLinks,
}

/// Repositories whose server answered a lock listing, so pushes verify locks (spec §8.1).
#[derive(Default)]
pub struct LockSupport(Mutex<HashSet<Uuid>>);

impl LockSupport {
    /// Records whether `repo`'s server supports Git LFS locks.
    pub fn set(&self, repo: Uuid, supported: bool) {
        let mut set = self.0.lock().unwrap_or_else(|p| p.into_inner());
        if supported {
            set.insert(repo);
        } else {
            set.remove(&repo);
        }
    }

    /// True once a lock listing for `repo` succeeded this session.
    pub fn get(&self, repo: Uuid) -> bool {
        self.0
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(&repo)
    }
}

impl AppState {
    /// Creates state with no git resolved yet.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bundled_git_dir: Option<PathBuf>,
        repos: RepoManager,
        accounts: AccountManager,
        saved_secrets: Arc<SavedSecrets>,
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
            saved_secrets,
            settings,
            forgejo,
            watcher: RepoWatcher::default(),
            operations: Operations::default(),
            prompts,
            trampoline,
            askpass,
            lock_support: LockSupport::default(),
            repo_links: RepoLinks::default(),
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

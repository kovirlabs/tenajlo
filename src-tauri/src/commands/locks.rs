//! Git LFS file lock commands (spec §8.1, v1.4).

use serde::Serialize;
use tauri::{AppHandle, State};
use uuid::Uuid;

use super::remote_op::{explain_failure, remote_run};
use super::{parse_id, pick, Pick};
use crate::auth::remote_auth::AuthRequest;
use crate::error::AppError;
use crate::git::error::{GitError, GitErrorKind};
use crate::git::lfs_locks::{self, lock_remote};
use crate::git::parse::lfs_locks::LfsLock;
use crate::state::AppState;

/// The repository's file locks.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsLocks {
    /// False when locks don't apply: no Git LFS, git-lfs missing, no remote, or a server
    /// without the locking API. The UI then hides everything lock-related.
    pub available: bool,
    pub locks: Vec<LfsLock>,
}

impl LfsLocks {
    fn unavailable() -> Self {
        Self {
            available: false,
            locks: Vec::new(),
        }
    }
}

/// Every lock on the server. `background` (the refresh after a fetch) never prompts.
#[tauri::command]
#[specta::specta]
pub async fn list_lfs_locks(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    background: bool,
) -> Result<LfsLocks, AppError> {
    let id = parse_id(&repo_id)?;
    let root = state.repos.root(id)?;
    let git = state.git()?;
    let Some((name, url)) = lock_remote(&git, &root).await? else {
        state.lock_support.set(id, false);
        return Ok(LfsLocks::unavailable());
    };
    let op_id = Uuid::new_v4().to_string();
    let (cancel, _op) = state.operations.start(&op_id);
    let auth = state
        .prepare_auth(AuthRequest {
            repo_id: &repo_id,
            op_id: &op_id,
            cancel: cancel.clone(),
            remote_url: Some(&url),
            interactive: !background,
        })
        .await?;
    let run = remote_run(&app, &auth, &cancel, &repo_id, &op_id);
    match lfs_locks::list(&git, &root, &name, run).await {
        Ok(locks) => {
            state.lock_support.set(id, true);
            Ok(LfsLocks {
                available: true,
                locks,
            })
        }
        Err(GitError::Failed {
            kind: GitErrorKind::LfsLocksUnsupported,
            ..
        }) => {
            state.lock_support.set(id, false);
            Ok(LfsLocks::unavailable())
        }
        Err(e) => Err(explain_failure(&state, &auth, e).await),
    }
}

/// Locks `path` (repository-relative, as in the changes list).
#[tauri::command]
#[specta::specta]
pub async fn lock_lfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<LfsLock, AppError> {
    if !lfs_locks::valid_path(&path) {
        return Err(AppError::invalid_input("That file can't be locked."));
    }
    lock_path(&app, &state, &repo_id, &path).await
}

/// Lets the user pick a file in the repository and locks it. `None` if they cancelled.
#[tauri::command]
#[specta::specta]
pub async fn lock_picked_lfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Option<LfsLock>, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let Some(picked) = pick(&app, "Choose a file to lock", Pick::File).await? else {
        return Ok(None);
    };
    let Some(path) = lfs_locks::repo_relative(&root, &picked) else {
        return Err(AppError::invalid_input(
            "Choose a file inside this repository's folder.",
        ));
    };
    lock_path(&app, &state, &repo_id, &path).await.map(Some)
}

async fn lock_path(
    app: &AppHandle,
    state: &AppState,
    repo_id: &str,
    path: &str,
) -> Result<LfsLock, AppError> {
    let id = parse_id(repo_id)?;
    let op_id = Uuid::new_v4().to_string();
    let (cancel, _op) = state.operations.start(&op_id);
    let (root, _guard) = state.repos.lock_repo_cancellable(id, &cancel).await?;
    let git = state.git()?;
    let (name, url) = lock_remote(&git, &root)
        .await?
        .ok_or_else(|| GitError::failed("does not support the Git LFS locking API", b"", None))?;
    let auth = state
        .prepare_auth(AuthRequest {
            repo_id,
            op_id: &op_id,
            cancel: cancel.clone(),
            remote_url: Some(&url),
            interactive: true,
        })
        .await?;
    let run = remote_run(app, &auth, &cancel, repo_id, &op_id);
    match lfs_locks::lock(&git, &root, &name, path, run).await {
        Ok(lock) => Ok(lock),
        Err(e) => Err(explain_failure(state, &auth, e).await),
    }
}

/// Removes lock `lock_id`. `force` removes someone else's lock (the UI confirms first).
#[tauri::command]
#[specta::specta]
pub async fn unlock_lfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    lock_id: String,
    force: bool,
) -> Result<(), AppError> {
    if !lfs_locks::valid_lock_id(&lock_id) {
        return Err(AppError::invalid_input("That lock id isn't valid."));
    }
    let id = parse_id(&repo_id)?;
    let op_id = Uuid::new_v4().to_string();
    let (cancel, _op) = state.operations.start(&op_id);
    let (root, _guard) = state.repos.lock_repo_cancellable(id, &cancel).await?;
    let git = state.git()?;
    let (name, url) = lock_remote(&git, &root)
        .await?
        .ok_or_else(|| GitError::failed("does not support the Git LFS locking API", b"", None))?;
    let auth = state
        .prepare_auth(AuthRequest {
            repo_id: &repo_id,
            op_id: &op_id,
            cancel: cancel.clone(),
            remote_url: Some(&url),
            interactive: true,
        })
        .await?;
    let run = remote_run(&app, &auth, &cancel, &repo_id, &op_id);
    match lfs_locks::unlock(&git, &root, &name, &lock_id, force, run).await {
        Ok(()) => Ok(()),
        Err(e) => Err(explain_failure(&state, &auth, e).await),
    }
}

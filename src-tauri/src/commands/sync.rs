//! Fetch / pull / push / publish commands and the sync button state.

use tauri::{AppHandle, State};

use super::remote_op::{explain_failure, remote_run};
use super::{check_op_id, parse_id};
use crate::auth::remote_auth::AuthRequest;
use crate::error::AppError;
use crate::git::remote;
use crate::git::sync_plan::{self, Plan, SyncRequest};
use crate::git::sync_state::{self, SyncState};
use crate::git::{branch_name, lfs, status};
use crate::state::AppState;
use crate::store::settings::PullStrategy;

/// The toolbar sync button state for the current branch.
#[tauri::command]
#[specta::specta]
pub async fn get_sync_state(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<SyncState, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let remotes = remote::list_remotes(&git, &root).await?;
    let fetched = remote::last_fetched(&git, &root).await?;
    Ok(sync_state::sync_state(&current, &remotes, fetched))
}

/// Runs a fetch, pull, push or publish. `op_id` (a UUID from the UI) identifies its progress
/// events and lets `cancel_operation` stop it. `background` (the periodic fetch) never shows
/// sign-in prompts: without an account token or the user's own helper, it just fails.
#[tauri::command]
#[specta::specta]
pub async fn sync(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    op_id: String,
    request: SyncRequest,
    background: bool,
) -> Result<(), AppError> {
    if background && request != SyncRequest::Fetch {
        return Err(AppError::invalid_input(
            "Only fetching runs in the background.",
        ));
    }
    let id = parse_id(&repo_id)?;
    check_op_id(&op_id)?;
    let (root, _guard) = state.repos.lock_repo(id).await?;
    let git = state.git()?;
    let (cancel, _op) = state.operations.start(&op_id);

    let current = status::status(&git, &root).await?;
    let remotes = remote::list_remotes(&git, &root).await?;
    let plan = sync_plan::plan(request, &current, &remotes)?;
    if let Plan::Push { remote_branch, .. } = &plan {
        branch_name::validate(&git, &root, remote_branch).await?;
    }

    // Credentials: the account for this remote's server, else the user's helpers + prompt.
    let url = remote::remote_url(&git, &root, plan.remote(), plan.is_push()).await?;
    let auth = state
        .prepare_auth(AuthRequest {
            repo_id: &repo_id,
            op_id: &op_id,
            cancel: cancel.clone(),
            remote_url: url.as_deref(),
            interactive: !background,
        })
        .await?;
    let make_run = || remote_run(&app, &auth, &cancel, &repo_id, &op_id);

    // LFS objects go up first, so the push never depends on a pre-push hook.
    if let Plan::Push { remote, branch, .. } | Plan::Publish { remote, branch } = &plan {
        let lfs = lfs::status(&git, &root);
        if lfs.missing() {
            return Err(AppError::lfs_missing());
        }
        if lfs.used {
            if let Err(e) = lfs::push_objects(&git, &root, remote, branch, make_run()).await {
                return Err(explain_failure(&state, &auth, e).await);
            }
        }
    }

    let run = make_run();
    let result = match plan {
        Plan::Fetch { remote } => remote::fetch(&git, &root, &remote, run).await,
        Plan::Pull { merge: true, .. } => remote::pull_merge(&git, &root, run).await,
        Plan::Pull { merge: false, .. } => match state.settings.get().pull_strategy {
            PullStrategy::FastForwardOnly => remote::pull(&git, &root, run).await,
            PullStrategy::Merge => remote::pull_merge(&git, &root, run).await,
            PullStrategy::Rebase => remote::pull_rebase(&git, &root, run).await,
        },
        Plan::Push {
            remote,
            branch,
            remote_branch,
        } => remote::push(&git, &root, &remote, &branch, &remote_branch, false, run).await,
        Plan::Publish { remote, branch } => {
            remote::push(&git, &root, &remote, &branch, &branch, true, run).await
        }
    };
    match result {
        Ok(()) => Ok(()),
        Err(e) => Err(explain_failure(&state, &auth, e).await),
    }
}

//! Fetch / pull / push / publish commands and the sync button state.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use super::repos::parse_id;
use crate::auth::remote_auth::{self, AuthDiagnosis, PrepareError, RemoteAuth};
use crate::error::{AppError, AppErrorKind};
use crate::git::error::{GitError, GitErrorKind};
use crate::git::parse::progress::Progress;
use crate::git::parse::status::WorkingDirectoryStatus;
use crate::git::remote::{self, RemoteRun};
use crate::git::sync_state::{self, split_upstream, SyncAction, SyncState};
use crate::git::{branch_name, status};
use crate::state::AppState;
use crate::store::settings::PullStrategy;

/// Minimum spacing between progress events (spec §5.4: ~10 Hz).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// What the user clicked. Rust re-derives the details (remote, branch) itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub enum SyncRequest {
    Fetch,
    Pull,
    /// "Merge the server's changes" after a fast-forward pull found diverged branches.
    PullMerge,
    Push,
    Publish,
}

/// Progress of a running remote operation.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct GitProgress {
    pub repo_id: String,
    pub op_id: String,
    pub progress: Progress,
}

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
/// events and lets `cancel_operation` stop it.
#[tauri::command]
#[specta::specta]
pub async fn sync(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    op_id: String,
    request: SyncRequest,
) -> Result<(), AppError> {
    let id = parse_id(&repo_id)?;
    if uuid::Uuid::parse_str(&op_id).is_err() {
        return Err(AppError::invalid_input("Invalid operation id."));
    }
    let (root, _guard) = state.repos.lock_repo(id).await?;
    let git = state.git()?;
    let (cancel, _op) = state.operations.start(&op_id);

    let current = status::status(&git, &root).await?;
    let remotes = remote::list_remotes(&git, &root).await?;
    let plan = plan(request, &current, &remotes)?;
    if let Plan::Push { remote_branch, .. } = &plan {
        branch_name::validate(&git, &root, remote_branch).await?;
    }

    // Credentials: the account for this remote's server, else the user's helpers + prompt.
    let url = remote::remote_url(&git, &root, plan.remote(), plan.is_push()).await?;
    let auth = remote_auth::prepare(
        &state.accounts,
        state.trampoline.as_ref(),
        state.askpass.as_deref(),
        &repo_id,
        &op_id,
        cancel.clone(),
        url.as_deref(),
    )
    .await
    .map_err(prepare_error)?;
    let run = RemoteRun {
        env: auth.env.clone(),
        config: auth.config.clone(),
        cancel,
        on_progress: progress_emitter(app, repo_id.clone(), op_id.clone()),
    };

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

/// What `sync` will run, decided by Rust from the repository state.
enum Plan {
    Fetch {
        remote: String,
    },
    Pull {
        remote: String,
        merge: bool,
    },
    Push {
        remote: String,
        branch: String,
        remote_branch: String,
    },
    Publish {
        remote: String,
        branch: String,
    },
}

impl Plan {
    fn remote(&self) -> &str {
        match self {
            Plan::Fetch { remote }
            | Plan::Pull { remote, .. }
            | Plan::Push { remote, .. }
            | Plan::Publish { remote, .. } => remote,
        }
    }

    fn is_push(&self) -> bool {
        matches!(self, Plan::Push { .. } | Plan::Publish { .. })
    }
}

fn plan(
    request: SyncRequest,
    current: &WorkingDirectoryStatus,
    remotes: &[String],
) -> Result<Plan, AppError> {
    let branch = current.branch.name.clone();
    let not_published =
        || AppError::invalid_input("This branch isn't on the server yet. Publish it first.");
    let upstream = || {
        current
            .branch
            .upstream
            .as_deref()
            .filter(|_| !current.branch.upstream_gone)
            .and_then(|u| split_upstream(u, remotes))
    };
    Ok(match request {
        SyncRequest::Fetch => match sync_state::sync_state(current, remotes, None).action {
            SyncAction::NoRemote => return Err(no_remote()),
            SyncAction::Fetch { remote }
            | SyncAction::Pull { remote }
            | SyncAction::Push { remote }
            | SyncAction::Publish { remote } => Plan::Fetch { remote },
        },
        SyncRequest::Pull | SyncRequest::PullMerge => {
            let (remote, _) = upstream().ok_or_else(not_published)?;
            Plan::Pull {
                remote,
                merge: request == SyncRequest::PullMerge,
            }
        }
        SyncRequest::Push => {
            let branch = branch.ok_or_else(detached)?;
            let (remote, remote_branch) = upstream().ok_or_else(not_published)?;
            Plan::Push {
                remote,
                branch,
                remote_branch,
            }
        }
        SyncRequest::Publish => Plan::Publish {
            branch: branch.ok_or_else(detached)?,
            remote: sync_state::default_remote(remotes)
                .ok_or_else(no_remote)?
                .to_owned(),
        },
    })
}

/// Maps a failed remote operation to an error. When git rejected the account's token,
/// checks the token to tell "sign in again" from "no access" (spec §6.2).
pub(crate) async fn explain_failure(
    state: &AppState,
    auth: &RemoteAuth,
    err: GitError,
) -> AppError {
    let auth_failed = matches!(
        err,
        GitError::Failed {
            kind: GitErrorKind::AuthFailed,
            ..
        }
    );
    let Some(account) = auth
        .account
        .as_ref()
        .filter(|_| auth_failed && auth.account_used())
    else {
        return err.into();
    };
    let git_error: AppError = err.into();
    match remote_auth::diagnose(&state.accounts, &state.forgejo, account).await {
        AuthDiagnosis::TokenRejected => AppError::sign_in_required(account, git_error.details),
        AuthDiagnosis::NoAccess => AppError::account_lacks_access(account, git_error),
        AuthDiagnosis::Unknown => git_error,
    }
}

pub(crate) fn prepare_error(err: PrepareError) -> AppError {
    match err {
        PrepareError::SignInRequired(account) => AppError::sign_in_required(&account, None),
        PrepareError::Trampoline(e) => AppError::with_details(
            AppErrorKind::Internal,
            "Couldn't prepare sign-in prompts.",
            e.to_string(),
        ),
    }
}

fn no_remote() -> AppError {
    AppError::invalid_input("This repository isn't connected to a server.")
}

fn detached() -> AppError {
    AppError::invalid_input("Switch to a branch first.")
}

/// Emits progress at most every [`PROGRESS_INTERVAL`], plus every phase change and completion.
pub(crate) fn progress_emitter(
    app: AppHandle,
    repo_id: String,
    op_id: String,
) -> Box<dyn FnMut(Progress) + Send> {
    let mut last: Option<(Instant, String)> = None;
    Box::new(move |progress| {
        let now = Instant::now();
        let due = match &last {
            None => true,
            Some((at, phase)) => {
                *phase != progress.phase
                    || progress.percent == Some(100)
                    || now.duration_since(*at) >= PROGRESS_INTERVAL
            }
        };
        if due {
            last = Some((now, progress.phase.clone()));
            let event = GitProgress {
                repo_id: repo_id.clone(),
                op_id: op_id.clone(),
                progress,
            };
            let _ = event.emit(&app);
        }
    })
}

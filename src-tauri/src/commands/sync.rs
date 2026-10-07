//! Fetch / pull / push / publish commands and the sync button state.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_specta::Event;

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::parse::progress::Progress;
use crate::git::remote::{self, RemoteRun};
use crate::git::sync_state::{self, split_upstream, SyncAction, SyncState};
use crate::git::{branch_name, status};
use crate::state::AppState;

/// Minimum spacing between progress events (spec §5.4: ~10 Hz).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// What the user clicked. Rust re-derives the details (remote, branch) itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub enum SyncRequest {
    Fetch,
    Pull,
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

    // Route credential prompts for this operation through the UI.
    let token = match (&state.trampoline, &state.askpass) {
        (Some(t), Some(_)) => Some(t.register(&repo_id, &op_id, cancel.clone()).map_err(|e| {
            AppError::with_details(
                crate::error::AppErrorKind::Internal,
                "Couldn't prepare sign-in prompts.",
                e.to_string(),
            )
        })?),
        _ => None,
    };
    let env = match (&token, &state.askpass) {
        (Some(t), Some(askpass)) => t.env(askpass),
        _ => Vec::new(),
    };
    let run = RemoteRun {
        env,
        cancel,
        on_progress: progress_emitter(app, repo_id.clone(), op_id.clone()),
    };

    let current = status::status(&git, &root).await?;
    let remotes = remote::list_remotes(&git, &root).await?;
    let sync = sync_state::sync_state(&current, &remotes, None);
    let branch = current.branch.name.clone();

    match request {
        SyncRequest::Fetch => {
            let remote = match sync.action {
                SyncAction::NoRemote => return Err(no_remote()),
                SyncAction::Fetch { remote }
                | SyncAction::Pull { remote }
                | SyncAction::Push { remote }
                | SyncAction::Publish { remote } => remote,
            };
            remote::fetch(&git, &root, &remote, run).await?;
        }
        SyncRequest::Pull => {
            if current.branch.upstream.is_none() {
                return Err(AppError::invalid_input(
                    "This branch isn't on the server yet. Publish it first.",
                ));
            }
            remote::pull(&git, &root, run).await?;
        }
        SyncRequest::Push => {
            let branch = branch.ok_or_else(detached)?;
            let (remote, remote_branch) = current
                .branch
                .upstream
                .as_deref()
                .filter(|_| !current.branch.upstream_gone)
                .and_then(|u| split_upstream(u, &remotes))
                .ok_or_else(|| {
                    AppError::invalid_input(
                        "This branch isn't on the server yet. Publish it first.",
                    )
                })?;
            branch_name::validate(&git, &root, &remote_branch).await?;
            remote::push(&git, &root, &remote, &branch, &remote_branch, false, run).await?;
        }
        SyncRequest::Publish => {
            let branch = branch.ok_or_else(detached)?;
            let remote = sync_state::default_remote(&remotes)
                .ok_or_else(no_remote)?
                .to_owned();
            remote::push(&git, &root, &remote, &branch, &branch, true, run).await?;
        }
    }
    Ok(())
}

fn no_remote() -> AppError {
    AppError::invalid_input("This repository isn't connected to a server.")
}

fn detached() -> AppError {
    AppError::invalid_input("Switch to a branch first.")
}

/// Emits progress at most every [`PROGRESS_INTERVAL`], plus every phase change and completion.
fn progress_emitter(
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

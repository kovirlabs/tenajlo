//! Shared plumbing for commands that talk to a server through git (sync, clone): progress
//! events, the run context, and explaining failures.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

use crate::auth::remote_auth::{self, AuthDiagnosis, RemoteAuth};
use crate::error::AppError;
use crate::git::error::{GitError, GitErrorKind};
use crate::git::parse::progress::Progress;
use crate::git::remote::RemoteRun;
use crate::state::AppState;

/// Minimum spacing between progress events (spec §5.4: ~10 Hz).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

/// Progress of a running remote operation.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct GitProgress {
    pub repo_id: String,
    pub op_id: String,
    pub progress: Progress,
}

/// Everything one git invocation of the operation needs: credentials, cancellation, and
/// throttled `GitProgress` events tagged with `repo_id` (empty for a clone) and `op_id`.
pub(crate) fn remote_run(
    app: &AppHandle,
    auth: &RemoteAuth,
    cancel: &CancellationToken,
    repo_id: &str,
    op_id: &str,
) -> RemoteRun {
    RemoteRun {
        env: auth.env.clone(),
        config: auth.config.clone(),
        cancel: cancel.clone(),
        on_progress: progress_emitter(app.clone(), repo_id.to_owned(), op_id.to_owned()),
    }
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

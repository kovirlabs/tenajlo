//! Tauri commands. Thin: validate input, call core modules, map errors to [`crate::error::AppError`].

use std::path::PathBuf;

use tauri::AppHandle;
use tauri_plugin_dialog::{DialogExt, FilePath};
use uuid::Uuid;

use crate::auth::accounts::AccountError;
use crate::error::AppError;
use crate::forgejo::ForgejoError;
use crate::state::AppState;
use crate::store::accounts::AccountEntry;

pub mod accounts;
pub mod app;
pub mod auth;
pub mod branches;
pub mod changes;
pub mod clone;
pub mod merge;
pub mod remote_op;
pub mod repo;
pub mod repos;
pub mod settings;
pub mod sync;

/// Parses a repository id from the frontend.
pub(crate) fn parse_id(id: &str) -> Result<Uuid, AppError> {
    id.parse()
        .map_err(|_| AppError::invalid_input("That repository id isn't valid."))
}

/// Checks an operation id from the frontend (a UUID naming progress events and cancellation).
pub(crate) fn check_op_id(op_id: &str) -> Result<(), AppError> {
    Uuid::parse_str(op_id)
        .map(|_| ())
        .map_err(|_| AppError::invalid_input("Invalid operation id."))
}

/// The stored account with the frontend's `account_id`.
pub(crate) fn account(state: &AppState, account_id: &str) -> Result<AccountEntry, AppError> {
    account_id
        .parse()
        .ok()
        .and_then(|id| state.accounts.entry(id))
        .ok_or_else(|| AccountError::UnknownAccount.into())
}

/// Maps a failed Forgejo API call made with `account`'s token. A 401 means the token stopped
/// working, so the account is marked "needs sign-in".
pub(crate) fn account_api_error(
    state: &AppState,
    account: &AccountEntry,
    err: ForgejoError,
) -> AppError {
    match err {
        ForgejoError::Unauthorized => state.accounts.token_rejected(account).into(),
        other => other.into(),
    }
}

/// What a native picker chooses.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Pick {
    Folder,
    File,
}

/// Shows a native folder or file picker. `Ok(None)` if the user cancelled; an error if the
/// choice isn't a local path.
pub(crate) async fn pick(
    app: &AppHandle,
    title: impl Into<String>,
    kind: Pick,
) -> Result<Option<PathBuf>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    let send = move |picked: Option<FilePath>| {
        let _ = tx.send(picked);
    };
    let dialog = app.dialog().file().set_title(title);
    match kind {
        Pick::Folder => dialog.pick_folder(send),
        Pick::File => dialog.pick_file(send),
    }
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    picked
        .into_path()
        .map(Some)
        .map_err(|_| AppError::invalid_input("That location can't be used."))
}

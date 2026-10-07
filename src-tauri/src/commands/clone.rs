//! Clone dialog commands: the user's Forgejo repositories, the destination, and the clone.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};
use tauri_plugin_dialog::DialogExt;

use super::sync::{explain_failure, prepare_error, progress_emitter};
use crate::auth::remote_auth;
use crate::error::AppError;
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::repos::{self, RemoteRepository};
use crate::forgejo::ForgejoError;
use crate::git::clone::{self, CloneError};
use crate::git::remote::RemoteRun;
use crate::repo_manager::Repository;
use crate::state::AppState;

/// Folder under Documents where clones go by default (spec §8.1).
const DEFAULT_PARENT: &str = "Tenajlo";

/// Repositories the account can clone: its own and its organizations'.
#[tauri::command]
#[specta::specta]
pub async fn list_forgejo_repositories(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Vec<RemoteRepository>, AppError> {
    let account = account_id
        .parse()
        .ok()
        .and_then(|id| state.accounts.entry(id))
        .ok_or_else(|| AppError::from(crate::auth::accounts::AccountError::UnknownAccount))?;
    if account.needs_sign_in {
        return Err(AppError::sign_in_required(&account, None));
    }
    let token = state
        .accounts
        .token(&account)
        .await
        .map_err(|e| AppError::from(crate::auth::accounts::AccountError::Secret(e)))?;
    let Some(token) = token else {
        let _ = state.accounts.mark_needs_sign_in(account.id);
        return Err(AppError::sign_in_required(&account, None));
    };
    let base = normalize_base_url(&account.base_url)?;
    match repos::list_repositories(&state.forgejo, &base, &token).await {
        Err(ForgejoError::Unauthorized) => {
            let _ = state.accounts.mark_needs_sign_in(account.id);
            Err(AppError::sign_in_required(&account, None))
        }
        other => Ok(other?),
    }
}

/// Default destination for `url`: `<default clone folder>/<name>` (Settings, else
/// `Documents/Tenajlo`), made unique if taken.
#[tauri::command]
#[specta::specta]
pub fn suggest_clone_path(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<String, AppError> {
    let parent = match state.settings.get().default_clone_folder {
        Some(folder) => PathBuf::from(folder),
        None => app
            .path()
            .document_dir()
            .or_else(|_| app.path().home_dir())
            .map_err(|_| AppError::invalid_input("Tenajlo couldn't find your Documents folder."))?
            .join(DEFAULT_PARENT),
    };
    Ok(display(&unique_child(&parent, &name_for(&url))))
}

/// Lets the user pick a parent folder; returns `<picked>/<name>`, or `None` if cancelled.
#[tauri::command]
#[specta::specta]
pub async fn choose_clone_folder(app: AppHandle, url: String) -> Result<Option<String>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose where to put the repository")
        .pick_folder(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let parent = picked
        .into_path()
        .map_err(|_| AppError::invalid_input("That folder can't be used."))?;
    Ok(Some(display(&unique_child(&parent, &name_for(&url)))))
}

/// Clones `url` into `path`, then adds and selects the new repository. Progress arrives as
/// `GitProgress` events with an empty `repoId`; `cancel_operation(op_id)` stops it.
#[tauri::command]
#[specta::specta]
pub async fn clone_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    op_id: String,
    url: String,
    path: String,
) -> Result<Repository, AppError> {
    if uuid::Uuid::parse_str(&op_id).is_err() {
        return Err(AppError::invalid_input("Invalid operation id."));
    }
    let url = url.trim().to_owned();
    clone::validate_url(&url)?;
    let dest = PathBuf::from(path.trim());
    let git = state.git()?;
    let (cancel, _op) = state.operations.start(&op_id);

    let auth = remote_auth::prepare(
        &state.accounts,
        state.trampoline.as_ref(),
        state.askpass.as_deref(),
        "",
        &op_id,
        cancel.clone(),
        Some(&url),
    )
    .await
    .map_err(prepare_error)?;
    let run = RemoteRun {
        env: auth.env.clone(),
        config: auth.config.clone(),
        cancel,
        on_progress: progress_emitter(app, String::new(), op_id.clone()),
    };
    match clone::clone(&git, &url, &dest, run).await {
        Ok(()) => {}
        Err(CloneError::Git(e)) => return Err(explain_failure(&state, &auth, e).await),
        Err(e) => return Err(e.into()),
    }
    Ok(state.repos.add(&git, &dest).await?)
}

fn name_for(url: &str) -> String {
    clone::folder_name(url).unwrap_or_else(|| "repository".to_owned())
}

/// `parent/name`, or `parent/name-2`, `-3`… if that already exists.
fn unique_child(parent: &Path, name: &str) -> PathBuf {
    let mut candidate = parent.join(name);
    let mut n = 2;
    while candidate.symlink_metadata().is_ok() && n < 1000 {
        candidate = parent.join(format!("{name}-{n}"));
        n += 1;
    }
    candidate
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_child_skips_taken_names() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_child(dir.path(), "plc"), dir.path().join("plc"));
        std::fs::create_dir(dir.path().join("plc")).unwrap();
        std::fs::write(dir.path().join("plc-2"), "").unwrap();
        assert_eq!(unique_child(dir.path(), "plc"), dir.path().join("plc-3"));
        assert_eq!(name_for("https://h/"), "repository");
    }
}

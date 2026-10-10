//! Clone dialog commands: the user's Forgejo repositories, the destination, and the clone.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager, State};

use super::remote_op::{explain_failure, remote_run};
use super::{check_op_id, pick, Pick};
use crate::auth::remote_auth::AuthRequest;
use crate::error::AppError;
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::repos::{self, RemoteRepository};
use crate::git::clone::{self, CloneError};
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
    let account = super::account(&state, &account_id)?;
    let token = state.accounts.usable_token(&account).await?;
    let base = normalize_base_url(&account.base_url)?;
    repos::list_repositories(&state.forgejo, &base, &token)
        .await
        .map_err(|e| super::account_api_error(&state, &account, e))
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
    let parent = default_parent(&app, &state)?;
    Ok(display(&unique_child(&parent, &name_for(&url))))
}

/// Where new repositories go: the Settings folder, else `Documents/Tenajlo`.
pub(crate) fn default_parent(app: &AppHandle, state: &AppState) -> Result<PathBuf, AppError> {
    match state.settings.get().default_clone_folder {
        Some(folder) => Ok(PathBuf::from(folder)),
        None => Ok(app
            .path()
            .document_dir()
            .or_else(|_| app.path().home_dir())
            .map_err(|_| AppError::invalid_input("Tenajlo couldn't find your Documents folder."))?
            .join(DEFAULT_PARENT)),
    }
}

/// Lets the user pick a parent folder; returns `<picked>/<name>`, or `None` if cancelled.
#[tauri::command]
#[specta::specta]
pub async fn choose_clone_folder(app: AppHandle, url: String) -> Result<Option<String>, AppError> {
    let Some(parent) = pick(&app, "Choose where to put the repository", Pick::Folder).await? else {
        return Ok(None);
    };
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
    check_op_id(&op_id)?;
    let url = url.trim().to_owned();
    clone::validate_url(&url)?;
    let dest = PathBuf::from(path.trim());
    let git = state.git()?;
    let (cancel, _op) = state.operations.start(&op_id);

    let auth = state
        .prepare_auth(AuthRequest {
            repo_id: "",
            op_id: &op_id,
            cancel: cancel.clone(),
            remote_url: Some(&url),
            interactive: true,
        })
        .await?;
    let run = remote_run(&app, &auth, &cancel, "", &op_id);
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

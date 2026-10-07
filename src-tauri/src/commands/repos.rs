//! Repository list commands.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;
use uuid::Uuid;

use crate::error::AppError;
use crate::git::init;
use crate::repo_manager::{Repository, RepositoryList};
use crate::state::AppState;

/// Parses a repository id from the frontend.
pub(crate) fn parse_id(id: &str) -> Result<Uuid, AppError> {
    id.parse()
        .map_err(|_| AppError::invalid_input("That repository id isn't valid."))
}

/// Lists added repositories, most recently opened first.
#[tauri::command]
#[specta::specta]
pub fn list_repositories(state: State<'_, AppState>) -> RepositoryList {
    state.repos.list()
}

/// Opens a folder picker and adds the chosen repository. `None` if the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn add_local_repository(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Option<Repository>, AppError> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Add a local repository")
        .pick_folder(move |picked| {
            let _ = tx.send(picked);
        });
    let Some(picked) = rx.await.ok().flatten() else {
        return Ok(None);
    };
    let dir = picked
        .into_path()
        .map_err(|_| AppError::invalid_input("That folder can't be opened."))?;
    let git = state.git()?;
    Ok(Some(state.repos.add(&git, &dir).await?))
}

/// Removes a repository from the list. Files on disk are not touched.
#[tauri::command]
#[specta::specta]
pub fn remove_repository(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    Ok(state.repos.remove(parse_id(&id)?)?)
}

/// Selects a repository and marks it recently opened.
#[tauri::command]
#[specta::specta]
pub fn select_repository(state: State<'_, AppState>, id: String) -> Result<Repository, AppError> {
    Ok(state.repos.select(parse_id(&id)?)?)
}

/// Folder that new repositories go in by default (shown in "New repository").
#[tauri::command]
#[specta::specta]
pub fn default_repository_folder(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    Ok(super::clone::default_parent(&app, &state)?
        .to_string_lossy()
        .into_owned())
}

/// Creates `parent/name` as a new repository on `main` (optionally with a README), then adds
/// and selects it.
#[tauri::command]
#[specta::specta]
pub async fn create_repository(
    state: State<'_, AppState>,
    parent: String,
    name: String,
    readme: bool,
) -> Result<Repository, AppError> {
    let git = state.git()?;
    let dir = init::init(&git, std::path::Path::new(parent.trim()), &name, readme).await?;
    Ok(state.repos.add(&git, &dir).await?)
}

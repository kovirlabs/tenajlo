//! Repository list commands.

use tauri::{AppHandle, State};

use super::{parse_id, pick, Pick};
use crate::error::AppError;
use crate::git::init;
use crate::repo_manager::{Repository, RepositoryList};
use crate::state::AppState;

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
    let Some(dir) = pick(&app, "Add a local repository", Pick::Folder).await? else {
        return Ok(None);
    };
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

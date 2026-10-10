//! Conflict banner commands: merge/rebase state, mark resolved, abort, open a file.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::changes::select;
use super::parse_id;
use crate::editor;
use crate::error::AppError;
use crate::git::merge::{self, OperationState};
use crate::git::parse::status::FileStatusKind;
use crate::git::status;
use crate::state::AppState;
use crate::store::settings::Editor;

/// The merge (or rebase…) in progress and its conflicted files.
#[tauri::command]
#[specta::specta]
pub async fn get_operation_state(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<OperationState, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    Ok(merge::operation_state(&git, &root, &current).await?)
}

/// Marks conflicted files as resolved. Paths that aren't conflicted are ignored.
#[tauri::command]
#[specta::specta]
pub async fn mark_resolved(
    state: State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let conflicted: Vec<String> = select(&current.files, &paths)
        .into_iter()
        .filter(|f| f.kind == FileStatusKind::Conflicted)
        .map(|f| f.path.clone())
        .collect();
    Ok(merge::mark_resolved(&git, &root, &conflicted).await?)
}

/// Aborts the merge, rebase, cherry-pick or revert in progress.
#[tauri::command]
#[specta::specta]
pub async fn abort_operation(state: State<'_, AppState>, repo_id: String) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let Some(kind) = merge::operation_state(&git, &root, &current)
        .await?
        .operation
    else {
        return Ok(());
    };
    Ok(merge::abort(&git, &root, kind).await?)
}

/// Opens a repository file in the editor chosen in Settings (default: its default app).
#[tauri::command]
#[specta::specta]
pub fn open_repo_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let file = repo_file(&state, &repo_id, &path)?;
    match state.settings.get().editor {
        Editor::SystemDefault => app
            .opener()
            .open_path(file.to_string_lossy(), None::<&str>)
            .map_err(|e| open_failed(e.to_string())),
        editor => Ok(editor::open(&editor, &file)?),
    }
}

/// Shows a repository file in Explorer / Finder / the file manager.
#[tauri::command]
#[specta::specta]
pub fn reveal_repo_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let file = repo_file(&state, &repo_id, &path)?;
    // A deleted file can't be selected; show its folder instead.
    let target = if file.exists() {
        file
    } else {
        file.parent().map(Path::to_path_buf).unwrap_or(file)
    };
    app.opener()
        .reveal_item_in_dir(&target)
        .map_err(|e| open_failed(e.to_string()))
}

fn repo_file(state: &AppState, repo_id: &str, path: &str) -> Result<PathBuf, AppError> {
    Ok(state.repos.file(parse_id(repo_id)?, path)?)
}

fn open_failed(details: String) -> AppError {
    AppError::internal("Tenajlo couldn't open that file.", details)
}

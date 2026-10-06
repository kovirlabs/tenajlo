//! Read-only commands on the selected repository.

use tauri::State;

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::diff;
use crate::git::parse::diff::FileDiff;
use crate::git::parse::status::WorkingDirectoryStatus;
use crate::git::status;
use crate::state::AppState;

/// Working directory status for a repository.
#[tauri::command]
#[specta::specta]
pub async fn get_status(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<WorkingDirectoryStatus, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(status::status(&state.git()?, &root).await?)
}

/// Diff of a changed working-directory file. Unchanged if the file no longer has changes.
#[tauri::command]
#[specta::specta]
pub async fn get_working_diff(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<FileDiff, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let git = state.git()?;
    // Re-read status so Rust, not the UI, decides how to diff, and the path is a real change.
    let current = status::status(&git, &root).await?;
    let Some(file) = current.files.iter().find(|f| f.path == path) else {
        return Ok(FileDiff::Unchanged);
    };
    Ok(diff::working_dir_diff(&git, &root, file, current.branch.tip.is_some()).await?)
}

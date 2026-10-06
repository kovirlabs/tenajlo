//! Read-only commands on the selected repository.

use tauri::State;

use super::repos::parse_id;
use crate::error::AppError;
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

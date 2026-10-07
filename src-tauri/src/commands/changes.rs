//! Mutating commands on the working directory. Each holds the repository's mutation lock.

use std::collections::HashSet;

use tauri::State;

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::parse::status::FileChange;
use crate::git::{stage, status};
use crate::state::AppState;

/// Picks the current status entries for `paths`. Paths that no longer have changes are ignored.
pub(crate) fn select<'a>(files: &'a [FileChange], paths: &[String]) -> Vec<&'a FileChange> {
    let wanted: HashSet<&str> = paths.iter().map(String::as_str).collect();
    files
        .iter()
        .filter(|f| wanted.contains(f.path.as_str()))
        .collect()
}

/// Stages (`staged = true`) or unstages the given changed files.
#[tauri::command]
#[specta::specta]
pub async fn set_staged(
    state: State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
    staged: bool,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let files = select(&current.files, &paths);
    if staged {
        stage::stage(&git, &root, &files).await?;
    } else {
        stage::unstage(&git, &root, &files, current.branch.tip.is_some()).await?;
    }
    Ok(())
}

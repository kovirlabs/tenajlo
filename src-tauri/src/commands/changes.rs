//! Mutating commands on the working directory. Each holds the repository's mutation lock.

use std::collections::HashSet;

use tauri::State;

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::identity::{self, Identity};
use crate::git::parse::status::{FileChange, StagedState};
use crate::git::{commit, stage, status};
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

/// Commits the staged changes. Returns the new commit's SHA.
#[tauri::command]
#[specta::specta]
pub async fn commit_changes(
    state: State<'_, AppState>,
    repo_id: String,
    summary: String,
    description: String,
) -> Result<String, AppError> {
    if summary.trim().is_empty() {
        return Err(AppError::invalid_input(
            "Add a summary describing your changes.",
        ));
    }
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    if current.has_conflicts {
        return Err(AppError::invalid_input(
            "Resolve the conflicted files before committing.",
        ));
    }
    if current.files.iter().all(|f| f.staged == StagedState::None) {
        return Err(AppError::invalid_input(
            "Select at least one file to include in the commit.",
        ));
    }
    let message = commit::build_message(&summary, &description);
    Ok(commit::commit(&git, &root, &message).await?)
}

/// The name and email git will record on commits in this repository.
#[tauri::command]
#[specta::specta]
pub async fn get_identity(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Identity, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(identity::identity(&state.git()?, &root).await?)
}

/// Saves name and email to the global git config. The UI calls this only after the user
/// explicitly confirms (CLAUDE.md rule 5).
#[tauri::command]
#[specta::specta]
pub async fn set_global_identity(
    state: State<'_, AppState>,
    name: String,
    email: String,
) -> Result<(), AppError> {
    if !identity::valid_field(&name) || !identity::valid_field(&email) {
        return Err(AppError::invalid_input(
            "Enter a name and an email address, each on one line.",
        ));
    }
    Ok(identity::set_global(&state.git()?, &name, &email).await?)
}

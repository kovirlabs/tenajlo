//! Read-only commands on the selected repository.

use tauri::{AppHandle, State};

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::branches;
use crate::git::diff;
use crate::git::log;
use crate::git::parse::branches::BranchList;
use crate::git::parse::diff::FileDiff;
use crate::git::parse::log::{Commit, CommitFile};
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

/// Largest page the History list may request.
const MAX_HISTORY_PAGE: u32 = 500;

/// A page of commits reachable from HEAD, newest first.
#[tauri::command]
#[specta::specta]
pub async fn get_history(
    state: State<'_, AppState>,
    repo_id: String,
    skip: u32,
    limit: u32,
) -> Result<Vec<Commit>, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(log::history(&state.git()?, &root, skip, limit.min(MAX_HISTORY_PAGE)).await?)
}

fn check_sha(sha: &str) -> Result<(), AppError> {
    if log::is_commit_hash(sha) {
        Ok(())
    } else {
        Err(AppError::invalid_input("That commit id isn't valid."))
    }
}

/// Files changed by a commit.
#[tauri::command]
#[specta::specta]
pub async fn get_commit_files(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<Vec<CommitFile>, AppError> {
    check_sha(&sha)?;
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(log::commit_files(&state.git()?, &root, &sha).await?)
}

/// Diff of one file within a commit.
#[tauri::command]
#[specta::specta]
pub async fn get_commit_diff(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
    path: String,
    old_path: Option<String>,
) -> Result<FileDiff, AppError> {
    check_sha(&sha)?;
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(diff::commit_file_diff(&state.git()?, &root, &sha, &path, old_path.as_deref()).await?)
}

/// Local and remote branches.
#[tauri::command]
#[specta::specta]
pub async fn get_branches(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<BranchList, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    Ok(branches::branches(&state.git()?, &root).await?)
}

/// Starts watching a repository for changes (emits `RepoChanged`). Replaces any previous watch.
/// Watching is best-effort: on failure the UI still refreshes on window focus.
#[tauri::command]
#[specta::specta]
pub fn watch_repository(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    if let Err(e) = state.watcher.watch(&app, repo_id, root) {
        tracing::warn!(error = %e, "could not watch repository");
    }
    Ok(())
}

//! Branch commands: create, switch, delete, and restoring changes saved on switch.

use tauri::State;

use super::repos::parse_id;
use crate::error::AppError;
use crate::git::stash::{self, SavedChanges};
use crate::git::switch::{self, LocalChanges};
use crate::git::{branch_name, branches, status};
use crate::state::AppState;

/// How a typed branch name will be created (`"Fix pump"` → `"Fix-pump"`). Pure; no git call.
#[tauri::command]
#[specta::specta]
pub fn preview_branch_name(name: String) -> String {
    branch_name::sanitize(&name)
}

/// Creates a branch from the current commit and switches to it. Local changes come along.
#[tauri::command]
#[specta::specta]
pub async fn create_branch(
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let name = branch_name::sanitize(&name);
    branch_name::validate(&git, &root, &name).await?;
    Ok(switch::create(&git, &root, &name).await?)
}

/// Switches to a local branch, or creates a tracking branch for a remote-only one.
#[tauri::command]
#[specta::specta]
pub async fn switch_branch(
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    local_changes: LocalChanges,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let list = branches::branches(&git, &root).await?;
    let target = switch::resolve_target(&list, &name).ok_or_else(|| {
        AppError::invalid_input("That branch no longer exists. Try fetching first.")
    })?;
    if let switch::Target::Track { local, .. } = &target {
        branch_name::validate(&git, &root, local).await?;
    }
    let current = status::status(&git, &root).await?;
    if current.has_conflicts {
        return Err(AppError::invalid_input(
            "Resolve the conflicted files before switching branches.",
        ));
    }
    let has_changes = !current.files.is_empty();
    Ok(switch::switch(
        &git,
        &root,
        &target,
        current.branch.name.as_deref(),
        has_changes,
        local_changes,
    )
    .await?)
}

/// Deletes a local branch. Without `force`, fails with `BranchNotMerged` if it has unique commits.
#[tauri::command]
#[specta::specta]
pub async fn delete_branch(
    state: State<'_, AppState>,
    repo_id: String,
    name: String,
    force: bool,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let list = branches::branches(&git, &root).await?;
    let Some(branch) = list.local.iter().find(|b| b.name == name) else {
        return Err(AppError::invalid_input("That branch no longer exists."));
    };
    if branch.is_current {
        return Err(AppError::invalid_input(
            "Switch to another branch before deleting this one.",
        ));
    }
    Ok(switch::delete(&git, &root, &name, force).await?)
}

/// Changes Anvil saved when the user last left the current branch, if any.
#[tauri::command]
#[specta::specta]
pub async fn get_saved_changes(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<Option<SavedChanges>, AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let Some(branch) = current.branch.name else {
        return Ok(None);
    };
    Ok(stash::find(&git, &root, &branch).await?)
}

/// Re-applies the changes saved for the current branch.
#[tauri::command]
#[specta::specta]
pub async fn restore_saved_changes(
    state: State<'_, AppState>,
    repo_id: String,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let saved = match current.branch.name {
        Some(branch) => stash::find(&git, &root, &branch).await?,
        None => None,
    };
    let saved = saved
        .ok_or_else(|| AppError::invalid_input("There are no saved changes for this branch."))?;
    Ok(stash::restore(&git, &root, &saved).await?)
}

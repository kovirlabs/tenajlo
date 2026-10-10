//! Mutating commands on the working directory. Each holds the repository's mutation lock.

use std::collections::HashSet;

use tauri::State;

use super::parse_id;
use crate::error::AppError;
use crate::git::discard::{self, DiscardError};
use crate::git::identity::{self, Identity};
use crate::git::line_staging::{self, LineRef, LineStageError};
use crate::git::parse::status::{FileChange, FileStatusKind};
use crate::git::undo::{self, UndoError, UndoneCommit};
use crate::git::{commit, ignore, stage, status};
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

/// Stages or unstages individual change lines of one file. `token` comes from the
/// `get_working_diff` result the lines refer to.
#[tauri::command]
#[specta::specta]
pub async fn set_lines_staged(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    token: String,
    lines: Vec<LineRef>,
    staged: bool,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let changed = || {
        AppError::invalid_input(
            "This file changed since it was shown. Check the updated changes and try again.",
        )
    };
    let file = current
        .files
        .iter()
        .find(|f| f.path == path)
        .ok_or_else(changed)?;
    let has_head = current.branch.tip.is_some();
    match line_staging::set_lines_staged(&git, &root, file, has_head, &token, &lines, staged).await
    {
        Ok(()) => Ok(()),
        Err(LineStageError::Git(e)) => Err(e.into()),
        Err(LineStageError::Changed | LineStageError::BadLine) => Err(changed()),
        Err(LineStageError::Unsupported) => Err(AppError::invalid_input(
            "This file can only be committed as a whole. Use its checkbox in the list of changes.",
        )),
    }
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
    if let Some(blocker) = commit::check_ready(&git, &root, &current).await? {
        return Err(blocker.into());
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

/// Discards all changes to the given files. Current content goes to the OS trash first.
#[tauri::command]
#[specta::specta]
pub async fn discard_changes(
    state: State<'_, AppState>,
    repo_id: String,
    paths: Vec<String>,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let git = state.git()?;
    let current = status::status(&git, &root).await?;
    let files = select(&current.files, &paths);
    let has_head = current.branch.tip.is_some();
    match discard::discard(
        &git,
        &root,
        &files,
        has_head,
        &crate::os_trash::move_to_trash,
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(DiscardError::Git(e)) => Err(e.into()),
        Err(DiscardError::Unsupported(_)) => Err(AppError::invalid_input(
            "Files with conflicts and submodules can't be discarded here. Nothing was changed.",
        )),
        Err(DiscardError::TrashFailed(failed)) => {
            let details = failed
                .iter()
                .map(|(p, e)| format!("{p}: {e}"))
                .collect::<Vec<_>>()
                .join("\n");
            Err(AppError::internal(
                "Some files couldn't be moved to the Trash, so their changes were kept. They may be open in another program.",
                details,
            ))
        }
    }
}

/// The extension "Ignore all .ext files" would ignore for `path`, if any. Pure; no git call.
#[tauri::command]
#[specta::specta]
pub fn ignore_extension(path: String) -> Option<String> {
    ignore::extension(&path).map(str::to_owned)
}

/// Adds an untracked file (or all files with its extension) to the root `.gitignore`.
#[tauri::command]
#[specta::specta]
pub async fn ignore_file(
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
    by_extension: bool,
) -> Result<(), AppError> {
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    let current = status::status(&state.git()?, &root).await?;
    // Only untracked files: ignoring a tracked file has no effect until it's removed from git.
    if !current
        .files
        .iter()
        .any(|f| f.path == path && f.kind == FileStatusKind::Untracked)
    {
        return Err(AppError::invalid_input(
            "Only new files that aren't in Git yet can be ignored.",
        ));
    }
    let pattern = if by_extension {
        ignore::extension_pattern(&path)
            .ok_or_else(|| AppError::invalid_input("That file has no extension to ignore."))?
    } else {
        ignore::exact_pattern(&path)
    };
    ignore::append(&root, &pattern).map_err(|e| {
        AppError::internal(
            "Tenajlo couldn't update the .gitignore file.",
            e.to_string(),
        )
    })
}

/// Undoes the most recent commit if it is still `sha`, keeping its changes staged.
/// Returns the commit's message so the UI can restore it.
#[tauri::command]
#[specta::specta]
pub async fn undo_commit(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<UndoneCommit, AppError> {
    if !crate::git::log::is_commit_hash(&sha) {
        return Err(AppError::invalid_input("That commit id isn't valid."));
    }
    let (root, _guard) = state.repos.lock_repo(parse_id(&repo_id)?).await?;
    match undo::undo_last_commit(&state.git()?, &root, &sha).await {
        Ok(undone) => Ok(undone),
        Err(UndoError::Git(e)) => Err(e.into()),
        Err(UndoError::HeadMoved) => {
            Err(AppError::invalid_input("This isn't the latest commit anymore, so it can't be undone."))
        }
        Err(UndoError::MergeCommit) => Err(AppError::invalid_input("Merge commits can't be undone here.")),
        Err(UndoError::AlreadyPushed) => Err(AppError::invalid_input(
            "This commit is already on the server, so undoing it here would cause problems for others.",
        )),
    }
}

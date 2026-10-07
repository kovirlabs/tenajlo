//! Conflict banner commands: merge/rebase state, mark resolved, abort, open a file.

use std::path::{Component, Path, PathBuf};

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::changes::select;
use super::repos::parse_id;
use crate::error::{AppError, AppErrorKind};
use crate::git::merge::{self, OperationState};
use crate::git::parse::status::FileStatusKind;
use crate::git::status;
use crate::state::AppState;

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

/// Opens a file from the repository in its default app (e.g. to resolve conflicts).
#[tauri::command]
#[specta::specta]
pub fn open_repo_file(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    path: String,
) -> Result<(), AppError> {
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let file = inside_repo(&root, &path)
        .ok_or_else(|| AppError::invalid_input("That file isn't in this repository."))?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| {
            AppError::with_details(
                AppErrorKind::Internal,
                "Tenajlo couldn't open that file.",
                e.to_string(),
            )
        })
}

/// `root/rel` if `rel` is a plain relative path that stays inside `root` (spec §10.9).
fn inside_repo(root: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    let plain = rel
        .components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir));
    if !plain || rel.as_os_str().is_empty() {
        return None;
    }
    let full = root.join(rel);
    // Symlinks could still point outside; compare canonical paths when the file exists.
    match (dunce::canonicalize(&full), dunce::canonicalize(root)) {
        (Ok(f), Ok(r)) if !f.starts_with(&r) => None,
        _ => Some(full),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_paths_inside_the_repo() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("repo");
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/a.txt"), "").unwrap();
        assert_eq!(
            inside_repo(&root, "sub/a.txt"),
            Some(root.join("sub/a.txt"))
        );
        for bad in ["../x", "/etc/passwd", "sub/../../x", ""] {
            assert_eq!(inside_repo(&root, bad), None, "{bad}");
        }
        #[cfg(unix)]
        {
            std::fs::write(dir.path().join("secret"), "").unwrap();
            std::os::unix::fs::symlink(dir.path().join("secret"), root.join("link")).unwrap();
            assert_eq!(inside_repo(&root, "link"), None);
        }
    }
}

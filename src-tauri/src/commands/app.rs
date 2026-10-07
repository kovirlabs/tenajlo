//! App-level commands: startup checks.

use tauri::State;

use crate::error::AppError;
use crate::git::{binary, version};
use crate::state::AppState;

/// Locates git and reports its version and whether it meets the minimum.
#[tauri::command]
#[specta::specta]
pub async fn check_git(state: State<'_, AppState>) -> Result<version::GitInfo, AppError> {
    let override_path = state.settings.get().git_path.map(std::path::PathBuf::from);
    let git = binary::resolve(override_path.as_deref(), state.bundled_git_dir.as_deref())?;
    let info = version::detect(&git).await?;
    tracing::info!(version = %info.version, supported = info.supported, lfs = ?info.lfs_version, "git detected");
    if info.supported {
        state.set_git(git.with_lfs(info.lfs_version.is_some()));
    }
    Ok(info)
}

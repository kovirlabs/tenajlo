//! App-level commands: startup checks.

use tauri::State;

use crate::error::AppError;
use crate::git::{binary, version};
use crate::state::AppState;

/// Locates git and reports its version and whether it meets the minimum.
#[tauri::command]
#[specta::specta]
pub async fn check_git(state: State<'_, AppState>) -> Result<version::GitInfo, AppError> {
    // TODO(M6): pass the Settings git-binary override here.
    let git = binary::resolve(None, state.bundled_git_dir.as_deref())?;
    let info = version::detect(&git).await?;
    tracing::info!(version = %info.version, supported = info.supported, "git detected");
    if info.supported {
        state.set_git(git);
    }
    Ok(info)
}

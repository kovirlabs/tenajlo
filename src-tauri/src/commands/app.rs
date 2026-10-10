//! App-level commands: startup checks, version and log files.

use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;

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

/// Shown in Settings → About.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    /// Folder with the daily log files.
    pub logs_dir: String,
}

/// Tenajlo's version and where its logs are.
#[tauri::command]
#[specta::specta]
pub fn get_app_info(app: AppHandle) -> Result<AppInfo, AppError> {
    Ok(AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        logs_dir: logs_dir(&app)?.to_string_lossy().into_owned(),
    })
}

/// Opens the log folder in Explorer / Finder (to attach logs to a bug report).
#[tauri::command]
#[specta::specta]
pub fn open_logs_folder(app: AppHandle) -> Result<(), AppError> {
    let dir = logs_dir(&app)?;
    let _ = std::fs::create_dir_all(&dir);
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::internal("Tenajlo couldn't open the log folder.", e.to_string()))
}

fn logs_dir(app: &AppHandle) -> Result<std::path::PathBuf, AppError> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("logs"))
        .map_err(|e| AppError::internal("Tenajlo couldn't find its data folder.", e.to_string()))
}

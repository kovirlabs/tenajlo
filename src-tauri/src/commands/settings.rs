//! Settings screen commands.

use tauri::{AppHandle, State};
use tauri_plugin_dialog::DialogExt;

use crate::error::AppError;
use crate::git::identity::{self, Identity};
use crate::state::AppState;
use crate::store::settings::Settings;

/// Current settings.
#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings.get()
}

/// Validates and saves settings; returns what was stored. After changing `gitPath`, the UI
/// re-runs `check_git`.
#[tauri::command]
#[specta::specta]
pub fn save_settings(state: State<'_, AppState>, settings: Settings) -> Result<Settings, AppError> {
    Ok(state.settings.save(settings)?)
}

/// Folder picker for settings. `None` if cancelled.
#[tauri::command]
#[specta::specta]
pub async fn choose_folder(app: AppHandle, title: String) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).pick_folder(move |p| {
        let _ = tx.send(p);
    });
    let picked = rx.await.ok().flatten()?;
    picked
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// File picker for settings (git or editor program). `None` if cancelled.
#[tauri::command]
#[specta::specta]
pub async fn choose_file(app: AppHandle, title: String) -> Option<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog().file().set_title(title).pick_file(move |p| {
        let _ = tx.send(p);
    });
    let picked = rx.await.ok().flatten()?;
    picked
        .into_path()
        .ok()
        .map(|p| p.to_string_lossy().into_owned())
}

/// Name and email from the user's global git config.
#[tauri::command]
#[specta::specta]
pub async fn get_global_identity(state: State<'_, AppState>) -> Result<Identity, AppError> {
    Ok(identity::global(&state.git()?).await?)
}

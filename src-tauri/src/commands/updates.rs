//! Tenajlo's own updates (spec §10 item 8). The WebView has no updater permissions; it can
//! only ask Rust to check, install, or open the release page.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;
use tauri_specta::Event;

use crate::error::AppError;
use crate::state::AppState;
use crate::updates::{self, AvailableUpdate, RELEASES_URL};

/// Download progress while an update installs.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    /// 0–100, or `None` when the server didn't say how big the download is.
    pub percent: Option<u8>,
}

/// Whether a newer Tenajlo is available. `None` means this is the newest.
#[tauri::command]
#[specta::specta]
pub async fn check_for_update(app: AppHandle) -> Result<Option<AvailableUpdate>, AppError> {
    Ok(updates::check(&app).await?)
}

/// Downloads and installs the newest version, then restarts Tenajlo. Refused while a fetch,
/// pull, push or clone is running.
#[tauri::command]
#[specta::specta]
pub async fn install_update(app: AppHandle, state: State<'_, AppState>) -> Result<(), AppError> {
    if state.operations.any_running() {
        return Err(AppError::invalid_input(
            "Wait for the current fetch, pull, push or clone to finish, then install the update.",
        ));
    }
    // Emit only when the percentage changes (once, if the size is unknown).
    let mut last: Option<Option<u8>> = None;
    let handle = app.clone();
    let installed = updates::install(&app, move |done, total| {
        let percent = total.filter(|t| *t > 0).map(|t| {
            u8::try_from(done.saturating_mul(100) / t)
                .unwrap_or(100)
                .min(100)
        });
        if last != Some(percent) {
            last = Some(percent);
            let _ = UpdateProgress { percent }.emit(&handle);
        }
    })
    .await?;
    if !installed {
        return Err(AppError::invalid_input(
            "You already have the newest Tenajlo.",
        ));
    }
    tracing::info!("update installed; restarting");
    app.restart()
}

/// Opens the latest release on GitHub in the browser (for installs that can't update
/// themselves).
#[tauri::command]
#[specta::specta]
pub fn open_release_page(app: AppHandle) -> Result<(), AppError> {
    app.opener()
        .open_url(RELEASES_URL, None::<&str>)
        .map_err(|e| AppError::internal("Tenajlo couldn't open your web browser.", e.to_string()))
}

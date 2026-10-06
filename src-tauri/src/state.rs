//! Application-wide state managed by Tauri.

use std::path::PathBuf;

/// Shared state injected into commands via `tauri::State`.
#[derive(Debug, Default)]
pub struct AppState {
    /// Root of the bundled MinGit (Windows release builds), if present.
    pub bundled_git_dir: Option<PathBuf>,
}

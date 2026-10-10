//! Tenajlo's own updates (spec §10 item 8): checked against the public GitHub Releases feed and
//! installed only after the Tauri updater verifies the release's signature against the public
//! key built into the app.
//!
//! This and the Forgejo client are the only network calls Tenajlo makes (CLAUDE.md). The
//! check can be turned off in Settings. Linux `.deb` installs can't replace themselves, so
//! there the user is pointed to the release page instead.

use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

/// Where users download releases by hand.
pub const RELEASES_URL: &str = "https://github.com/kovirlabs/tenajlo/releases/latest";
/// How long a check may take before it's reported as failed.
const CHECK_TIMEOUT: Duration = Duration::from_secs(20);

/// A newer Tenajlo than the one running.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub version: String,
    /// Release notes from the release, if any.
    pub notes: Option<String>,
    pub release_url: String,
}

/// True if this copy can check for and install updates itself.
pub fn enabled<R: Runtime>(app: &AppHandle<R>) -> bool {
    configured(app.config()) && can_self_update()
}

/// Why checking or installing failed.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    /// This build has no update signing key (a local or development build).
    #[error("this build can't check for updates")]
    NotConfigured,
    #[error("the update check timed out")]
    TimedOut,
    #[error("update failed: {0}")]
    Updater(#[from] tauri_plugin_updater::Error),
}

/// True on platforms whose installer the updater can run in place.
pub fn can_self_update() -> bool {
    cfg!(any(windows, target_os = "macos"))
}

/// True if the app was built with the update signing key's public half. Without it the
/// updater can't verify anything, so updates stay off.
pub fn configured(config: &tauri::Config) -> bool {
    config
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

/// Asks the release feed whether a newer version exists.
pub async fn check<R: Runtime>(app: &AppHandle<R>) -> Result<Option<AvailableUpdate>, UpdateError> {
    // The feed only lists installers the updater can run, so a Linux package has nothing to
    // compare against; the About tab links to the release page instead.
    if !enabled(app) {
        return Err(UpdateError::NotConfigured);
    }
    let updater = app.updater_builder().timeout(CHECK_TIMEOUT).build()?;
    let found = tokio::time::timeout(CHECK_TIMEOUT, updater.check())
        .await
        .map_err(|_| UpdateError::TimedOut)??;
    if let Some(u) = &found {
        tracing::info!(current = %u.current_version, available = %u.version, "update available");
    }
    Ok(found.map(|u| AvailableUpdate {
        version: u.version.clone(),
        notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        release_url: RELEASES_URL.to_owned(),
    }))
}

/// Downloads, verifies and installs the newest version, reporting `(downloaded, total)`
/// bytes. Returns `Ok(false)` if there's no longer anything newer. The caller restarts.
pub async fn install<R: Runtime>(
    app: &AppHandle<R>,
    mut progress: impl FnMut(u64, Option<u64>) + Send,
) -> Result<bool, UpdateError> {
    if !enabled(app) {
        return Err(UpdateError::NotConfigured);
    }
    let Some(update) = app.updater()?.check().await? else {
        return Ok(false);
    };
    tracing::info!(version = %update.version, "installing update");
    let mut downloaded = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                downloaded += chunk as u64;
                progress(downloaded, total);
            },
            || {},
        )
        .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_with_pubkey(pubkey: &str) -> tauri::Config {
        let raw = include_str!("../tauri.conf.json");
        let mut json: serde_json::Value = serde_json::from_str(raw).unwrap();
        json["plugins"]["updater"]["pubkey"] = pubkey.into();
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn updates_need_a_public_key() {
        assert!(!configured(&config_with_pubkey("")));
        assert!(!configured(&config_with_pubkey("  ")));
        assert!(configured(&config_with_pubkey(
            "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWdu"
        )));
    }

    #[test]
    fn feed_is_the_public_release_on_github() {
        let raw = include_str!("../tauri.conf.json");
        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        assert_eq!(
            json["plugins"]["updater"]["endpoints"],
            serde_json::json!([
                "https://github.com/kovirlabs/tenajlo/releases/latest/download/latest.json"
            ])
        );
    }
}

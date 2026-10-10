//! Loads, validates and saves user settings.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::store::settings::{Editor, Settings, FILE_NAME};
use crate::store::{self, StoreError};

/// Upper bound for the background fetch interval (one day).
const MAX_FETCH_MINUTES: u32 = 24 * 60;

/// Why settings couldn't be saved.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("invalid setting: {0}")]
    Invalid(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Owns `settings.json`.
pub struct SettingsManager {
    path: PathBuf,
    current: Mutex<Settings>,
    read_only: bool,
}

impl SettingsManager {
    /// Loads settings from `data_dir`. Never fails: unreadable files give defaults (and,
    /// if written by a newer Tenajlo, are never overwritten).
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(FILE_NAME);
        let (current, read_only) = match store::load::<Settings>(&path) {
            Ok(s) => (s, false),
            Err(e) => {
                tracing::error!(error = %e, "could not load settings; using defaults");
                (Settings::default(), true)
            }
        };
        Self {
            path,
            current: Mutex::new(current),
            read_only,
        }
    }

    /// Current settings.
    pub fn get(&self) -> Settings {
        self.lock().clone()
    }

    /// Validates and saves `next`, returning what was stored.
    pub fn save(&self, next: Settings) -> Result<Settings, SettingsError> {
        let next = validate(next)?;
        // Hold the lock across the write so two saves can't interleave on disk.
        let mut current = self.lock();
        if self.read_only {
            tracing::warn!("settings were written by a newer Tenajlo; not saving");
        } else {
            store::save(&self.path, &next)?;
        }
        *current = next.clone();
        Ok(next)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Settings> {
        self.current.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Trims paths (empty → unset) and checks they're usable.
fn validate(mut s: Settings) -> Result<Settings, SettingsError> {
    let clean = |p: Option<String>| p.map(|p| p.trim().to_owned()).filter(|p| !p.is_empty());
    s.default_clone_folder = clean(s.default_clone_folder);
    s.git_path = clean(s.git_path);
    if s.background_fetch_minutes > MAX_FETCH_MINUTES {
        return Err(SettingsError::Invalid(
            "Background fetch can be at most once a day (1440 minutes).".into(),
        ));
    }
    if let Some(dir) = &s.default_clone_folder {
        if !Path::new(dir).is_absolute() {
            return Err(SettingsError::Invalid(
                "Choose a full folder path for new clones.".into(),
            ));
        }
    }
    if let Some(git) = &s.git_path {
        if !Path::new(git).is_file() {
            return Err(SettingsError::Invalid(format!(
                "There's no program at {git}."
            )));
        }
    }
    if let Editor::Custom { path } = &mut s.editor {
        *path = path.trim().to_owned();
        if !Path::new(path.as_str()).is_file() {
            return Err(SettingsError::Invalid(format!(
                "There's no editor program at {path}."
            )));
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saves_and_reloads_with_validation() {
        let dir = tempfile::tempdir().unwrap();
        let m = SettingsManager::load(dir.path());
        assert_eq!(m.get(), Settings::default());

        let folder = dir.path().to_string_lossy().into_owned();
        let saved = m
            .save(Settings {
                default_clone_folder: Some(format!("  {folder} ")),
                git_path: Some("   ".into()),
                ..Settings::default()
            })
            .unwrap();
        assert_eq!(saved.default_clone_folder.as_deref(), Some(folder.as_str()));
        assert_eq!(saved.git_path, None);
        assert_eq!(SettingsManager::load(dir.path()).get(), saved);

        for bad in [
            Settings {
                background_fetch_minutes: 5000,
                ..Settings::default()
            },
            Settings {
                default_clone_folder: Some("relative/dir".into()),
                ..Settings::default()
            },
            Settings {
                git_path: Some(dir.path().join("nope").to_string_lossy().into_owned()),
                ..Settings::default()
            },
            Settings {
                editor: Editor::Custom {
                    path: "/no/such/editor".into(),
                },
                ..Settings::default()
            },
        ] {
            assert!(matches!(m.save(bad), Err(SettingsError::Invalid(_))));
        }
        assert_eq!(m.get(), saved, "rejected saves change nothing");
    }
}

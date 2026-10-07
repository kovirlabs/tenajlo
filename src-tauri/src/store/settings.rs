//! `settings.json`: user preferences (spec §8.1 Settings, §9). Never holds secrets.

use serde::{Deserialize, Serialize};

use super::Versioned;

/// File name inside the app data dir.
pub const FILE_NAME: &str = "settings.json";

/// Spec §8.1: background fetch every 5 minutes by default; 0 turns it off.
pub const DEFAULT_FETCH_MINUTES: u32 = 5;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

/// What Pull does (spec §13.5: fast-forward only unless the user chooses otherwise).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub enum PullStrategy {
    /// Fast-forward only; on divergence, offer a merge.
    #[default]
    FastForwardOnly,
    Merge,
    /// No guided conflict UI: a conflicted rebase can only be aborted.
    Rebase,
}

/// Program used for "Open in editor".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "kind")]
pub enum Editor {
    /// The file's default app.
    #[default]
    SystemDefault,
    VsCode,
    NotepadPlusPlus,
    Custom {
        path: String,
    },
}

/// Contents of `settings.json`. Missing fields take their defaults (see `normalize`), so
/// files written before a setting existed keep loading.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    /// Where clones go by default; `None` = `Documents/Tenajlo`.
    pub default_clone_folder: Option<String>,
    pub pull_strategy: PullStrategy,
    /// 0 = off.
    pub background_fetch_minutes: u32,
    /// Git executable to use instead of the bundled/system one.
    pub git_path: Option<String>,
    pub editor: Editor,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            default_clone_folder: None,
            pull_strategy: PullStrategy::FastForwardOnly,
            background_fetch_minutes: DEFAULT_FETCH_MINUTES,
            git_path: None,
            editor: Editor::SystemDefault,
        }
    }
}

impl Versioned for Settings {
    const VERSION: u64 = 1;

    fn normalize(mut doc: serde_json::Value) -> serde_json::Value {
        if let (Some(map), Ok(serde_json::Value::Object(defaults))) = (
            doc.as_object_mut(),
            serde_json::to_value(Settings::default()),
        ) {
            for (key, value) in defaults {
                map.entry(key).or_insert(value);
            }
        }
        doc
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_fields_take_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        std::fs::write(&path, r#"{"schemaVersion":1,"theme":"Dark"}"#).unwrap();
        let s: Settings = crate::store::load(&path).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.background_fetch_minutes, DEFAULT_FETCH_MINUTES);
        assert_eq!(s.editor, Editor::SystemDefault);
    }

    #[test]
    fn round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE_NAME);
        let s = Settings {
            editor: Editor::Custom {
                path: "C:\\Tools\\ed.exe".into(),
            },
            pull_strategy: PullStrategy::Rebase,
            ..Settings::default()
        };
        crate::store::save(&path, &s).unwrap();
        assert_eq!(crate::store::load::<Settings>(&path).unwrap(), s);
    }
}

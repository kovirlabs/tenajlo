//! JSON persistence with `schemaVersion` and migrations (spec §9).
//!
//! Each persisted file implements [`Versioned`]. Loading migrates older
//! versions forward, refuses files written by a newer Tenajlo, and moves corrupt
//! files aside instead of silently discarding them. Writes are atomic.

pub mod accounts;
pub mod repositories;
pub mod settings;

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

/// Errors loading or saving a persisted file.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("i/o error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(
        "{path} was written by a newer version of Tenajlo (schema {found}, supported {supported})"
    )]
    TooNew {
        path: PathBuf,
        found: u64,
        supported: u64,
    },
    #[error("could not serialize {0}")]
    Serialize(#[from] serde_json::Error),
}

/// A persisted document with a schema version and forward migrations.
pub trait Versioned: Serialize + DeserializeOwned + Default {
    /// Current schema version written by this build.
    const VERSION: u64;

    /// Migrates `doc` from `from` to `from + 1`. Called repeatedly until current.
    /// The default has no migrations, which is correct while `VERSION == 1`.
    fn migrate(doc: Value, from: u64) -> Value {
        let _ = from;
        doc
    }

    /// Adjusts the current-version document before it's parsed, e.g. filling in fields
    /// added since it was written. The default leaves it unchanged.
    fn normalize(doc: Value) -> Value {
        doc
    }
}

/// Loads `path`, migrating as needed. Missing → default. Corrupt → moved aside, default.
pub fn load<T: Versioned>(path: &Path) -> Result<T, StoreError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(T::default()),
        Err(source) => {
            return Err(StoreError::Io {
                path: path.into(),
                source,
            })
        }
    };

    let mut doc: Value = match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(e) => return quarantine(path, &e.to_string()),
    };
    let mut version = doc
        .get("schemaVersion")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    if version > T::VERSION {
        return Err(StoreError::TooNew {
            path: path.into(),
            found: version,
            supported: T::VERSION,
        });
    }
    while version < T::VERSION {
        doc = T::migrate(doc, version);
        version += 1;
    }
    match serde_json::from_value(T::normalize(doc)) {
        Ok(v) => Ok(v),
        Err(e) => quarantine(path, &e.to_string()),
    }
}

/// Atomically writes `value` to `path` (temp file + fsync + rename).
pub fn save<T: Versioned>(path: &Path, value: &T) -> Result<(), StoreError> {
    let io = |source| StoreError::Io {
        path: path.into(),
        source,
    };
    let mut doc = serde_json::to_value(value)?;
    if let Value::Object(map) = &mut doc {
        map.insert("schemaVersion".into(), T::VERSION.into());
    }
    let bytes = serde_json::to_vec_pretty(&doc)?;

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(io)?;
    }
    let tmp = path.with_extension("json.tmp");
    {
        let mut f = fs::File::create(&tmp).map_err(io)?;
        f.write_all(&bytes).map_err(io)?;
        f.sync_all().map_err(io)?;
    }
    fs::rename(&tmp, path).map_err(io)
}

/// Moves an unreadable file to `<name>.corrupt` and returns the default.
fn quarantine<T: Versioned>(path: &Path, reason: &str) -> Result<T, StoreError> {
    let backup = path.with_extension("json.corrupt");
    tracing::warn!(path = %path.display(), backup = %backup.display(), %reason, "corrupt store file moved aside");
    fs::rename(path, &backup).map_err(|source| StoreError::Io {
        path: path.into(),
        source,
    })?;
    Ok(T::default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Doc {
        items: Vec<String>,
    }

    impl Versioned for Doc {
        const VERSION: u64 = 2;
        fn migrate(mut doc: Value, from: u64) -> Value {
            // v1 called the field "names".
            if from == 1 {
                if let Some(names) = doc.as_object_mut().and_then(|m| m.remove("names")) {
                    doc["items"] = names;
                }
            }
            doc
        }
    }

    #[test]
    fn missing_file_is_default() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            load::<Doc>(&dir.path().join("x.json")).unwrap(),
            Doc::default()
        );
    }

    #[test]
    fn round_trips_with_version() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub/x.json");
        let doc = Doc {
            items: vec!["ü".into()],
        };
        save(&path, &doc).unwrap();
        let raw: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(raw["schemaVersion"], 2);
        assert_eq!(load::<Doc>(&path).unwrap(), doc);
        assert!(!path.with_extension("json.tmp").exists());
    }

    #[test]
    fn migrates_old_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        fs::write(&path, r#"{"schemaVersion":1,"names":["a"]}"#).unwrap();
        assert_eq!(load::<Doc>(&path).unwrap().items, vec!["a"]);
    }

    #[test]
    fn refuses_newer_versions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        fs::write(&path, r#"{"schemaVersion":99,"items":[]}"#).unwrap();
        assert!(matches!(
            load::<Doc>(&path),
            Err(StoreError::TooNew { found: 99, .. })
        ));
        assert!(path.exists(), "newer file must be left untouched");
    }

    #[test]
    fn quarantines_corrupt_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.json");
        fs::write(&path, "{ not json").unwrap();
        assert_eq!(load::<Doc>(&path).unwrap(), Doc::default());
        assert!(path.with_extension("json.corrupt").exists());
        assert!(!path.exists());
    }
}

//! "Open in editor" (spec §8.1): VS Code, Notepad++, a custom program, or the default app.
//!
//! Editors are started with an argument array (never a shell) and the absolute file path,
//! which can't be mistaken for an option. Tenajlo doesn't wait for them to exit.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Serialize;

use crate::store::settings::Editor;

/// An editor choice for the Settings list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorOption {
    pub editor: Editor,
    pub label: String,
    /// Found on this computer (always true for the system default and custom programs).
    pub available: bool,
}

/// Why an editor couldn't be started.
#[derive(Debug, thiserror::Error)]
pub enum EditorError {
    #[error("{0} isn't installed")]
    NotFound(&'static str),
    #[error("could not start {program}: {source}")]
    Spawn {
        program: String,
        #[source]
        source: std::io::Error,
    },
}

/// The editors Settings offers, with whether each was found.
pub fn options() -> Vec<EditorOption> {
    let mut out = vec![
        EditorOption {
            editor: Editor::SystemDefault,
            label: "The file's default app".into(),
            available: true,
        },
        EditorOption {
            editor: Editor::VsCode,
            label: "Visual Studio Code".into(),
            available: vs_code().is_some(),
        },
    ];
    if cfg!(windows) {
        out.push(EditorOption {
            editor: Editor::NotepadPlusPlus,
            label: "Notepad++".into(),
            available: notepad_plus_plus().is_some(),
        });
    }
    out
}

/// Opens `file` in `editor`. `SystemDefault` is handled by the caller (Tauri opener).
pub fn open(editor: &Editor, file: &Path) -> Result<(), EditorError> {
    let program = match editor {
        Editor::SystemDefault => return Ok(()),
        Editor::VsCode => vs_code().ok_or(EditorError::NotFound("Visual Studio Code"))?,
        Editor::NotepadPlusPlus => notepad_plus_plus().ok_or(EditorError::NotFound("Notepad++"))?,
        Editor::Custom { path } => PathBuf::from(path),
    };
    let mut cmd = if cfg!(target_os = "macos") && program.extension().is_some_and(|e| e == "app") {
        // App bundles are launched through LaunchServices.
        let mut c = Command::new("open");
        c.arg("-a").arg(&program);
        c
    } else {
        Command::new(&program)
    };
    cmd.arg(file)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    cmd.spawn().map(drop).map_err(|source| EditorError::Spawn {
        program: program.display().to_string(),
        source,
    })
}

/// VS Code: the app bundle on macOS, `Code.exe` on Windows, `code` on PATH on Linux.
fn vs_code() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        let home_apps = std::env::var_os("HOME").map(|h| Path::new(&h).join("Applications"));
        [Some(PathBuf::from("/Applications")), home_apps]
            .into_iter()
            .flatten()
            .map(|dir| dir.join("Visual Studio Code.app"))
            .find(|p| p.is_dir())
    } else if cfg!(windows) {
        let candidates = [
            ("LOCALAPPDATA", "Programs\\Microsoft VS Code\\Code.exe"),
            ("ProgramFiles", "Microsoft VS Code\\Code.exe"),
        ];
        first_existing(&candidates)
    } else {
        on_path("code")
    }
}

fn notepad_plus_plus() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    first_existing(&[
        ("ProgramFiles", "Notepad++\\notepad++.exe"),
        ("ProgramFiles(x86)", "Notepad++\\notepad++.exe"),
    ])
}

/// The first `%VAR%\rel` that exists.
fn first_existing(candidates: &[(&str, &str)]) -> Option<PathBuf> {
    candidates.iter().find_map(|(var, rel)| {
        let path = Path::new(&std::env::var_os(var)?).join(rel);
        path.is_file().then_some(path)
    })
}

fn on_path(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH")?)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_default_is_always_offered_first() {
        let opts = options();
        assert_eq!(opts[0].editor, Editor::SystemDefault);
        assert!(opts[0].available);
        assert!(opts.iter().any(|o| o.editor == Editor::VsCode));
    }

    #[test]
    fn custom_editor_gets_the_file_as_its_argument() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("args.txt");
        #[cfg(unix)]
        let program = {
            use std::os::unix::fs::PermissionsExt;
            let script = dir.path().join("fake editor.sh");
            std::fs::write(
                &script,
                format!("#!/bin/sh\nprintf '%s' \"$1\" > '{}'\n", log.display()),
            )
            .unwrap();
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
            script
        };
        #[cfg(windows)]
        let program = PathBuf::from(std::env::var("ComSpec").unwrap());
        let file = dir.path().join("ünï file.txt");
        open(
            &Editor::Custom {
                path: program.to_string_lossy().into_owned(),
            },
            &file,
        )
        .unwrap();
        #[cfg(unix)]
        {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !log.exists() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            assert_eq!(
                std::fs::read_to_string(&log).unwrap(),
                file.to_string_lossy()
            );
        }
    }

    #[test]
    fn missing_custom_editor_is_reported() {
        let err = open(
            &Editor::Custom {
                path: "/definitely/not/an/editor".into(),
            },
            Path::new("/tmp/x"),
        )
        .unwrap_err();
        assert!(matches!(err, EditorError::Spawn { .. }));
    }
}

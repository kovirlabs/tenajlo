//! Working-tree watcher: emits [`RepoChanged`] when a repository's files or git state change.

use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use notify_debouncer_full::notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_full::{new_debouncer, DebounceEventResult, Debouncer, RecommendedCache};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_specta::Event;

/// Quiet period before a burst of file events becomes one refresh.
const DEBOUNCE: Duration = Duration::from_millis(400);

/// Emitted (debounced) when files or git state in a watched repository change.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct RepoChanged {
    pub repo_id: String,
}

/// Watches at most one repository: the one the user has open.
#[derive(Default)]
pub struct RepoWatcher {
    current: Mutex<Option<(String, Debouncer<RecommendedWatcher, RecommendedCache>)>>,
}

impl RepoWatcher {
    /// Starts watching `root`, replacing any previous watch. No-op if already watching `repo_id`.
    pub fn watch(&self, app: &AppHandle, repo_id: String, root: PathBuf) -> Result<(), String> {
        let mut current = self.current.lock().unwrap_or_else(|p| p.into_inner());
        if current.as_ref().is_some_and(|(id, _)| *id == repo_id) {
            return Ok(());
        }
        // Drop the old watcher first so we never watch two repositories.
        *current = None;

        let app = app.clone();
        let id = repo_id.clone();
        let event_root = root.clone();
        let mut debouncer =
            new_debouncer(DEBOUNCE, None, move |res: DebounceEventResult| match res {
                Ok(events) => {
                    let relevant = events
                        .iter()
                        .flat_map(|e| e.paths.iter())
                        .any(|p| is_relevant(&event_root, p));
                    if relevant {
                        if let Err(e) = (RepoChanged {
                            repo_id: id.clone(),
                        })
                        .emit(&app)
                        {
                            tracing::warn!(error = %e, "failed to emit repo-changed");
                        }
                    }
                }
                Err(errors) => tracing::warn!(count = errors.len(), "file watcher errors"),
            })
            .map_err(|e| e.to_string())?;
        debouncer
            .watch(&root, RecursiveMode::Recursive)
            .map_err(|e| e.to_string())?;
        *current = Some((repo_id, debouncer));
        Ok(())
    }
}

/// Whether a changed path should trigger a refresh.
///
/// Working-tree files always count. Inside `.git`, only state that affects what we show:
/// the index, HEAD, refs, and merge/rebase markers — not objects, logs, or lock files.
pub fn is_relevant(root: &Path, path: &Path) -> bool {
    let Ok(rel) = path.strip_prefix(root) else {
        // Events outside the root (e.g. the root itself being renamed) — refresh to be safe.
        return true;
    };
    let mut parts = rel.components().filter_map(|c| match c {
        Component::Normal(s) => s.to_str(),
        _ => None,
    });
    if parts.next() != Some(".git") {
        return true;
    }
    if rel.extension().is_some_and(|e| e == "lock") {
        return false;
    }
    matches!(
        parts.next(),
        Some(
            "index"
                | "HEAD"
                | "refs"
                | "packed-refs"
                | "MERGE_HEAD"
                | "CHERRY_PICK_HEAD"
                | "REVERT_HEAD"
                | "rebase-merge"
                | "rebase-apply"
        )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_git_internals() {
        let root = Path::new("/r");
        let yes = [
            "/r/src/main.py",
            "/r/ü dir/x",
            "/r/.gitignore",
            "/r/.git/index",
            "/r/.git/HEAD",
            "/r/.git/refs/heads/main",
            "/r/.git/packed-refs",
            "/r/.git/MERGE_HEAD",
            "/elsewhere",
        ];
        let no = [
            "/r/.git/objects/ab/cdef",
            "/r/.git/logs/HEAD",
            "/r/.git/index.lock",
            "/r/.git/refs/heads/main.lock",
            "/r/.git/FETCH_HEAD",
            "/r/.git/config",
        ];
        for p in yes {
            assert!(is_relevant(root, Path::new(p)), "{p} should be relevant");
        }
        for p in no {
            assert!(!is_relevant(root, Path::new(p)), "{p} should be ignored");
        }
    }
}

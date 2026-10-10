//! Desktop notifications after a background fetch (spec §8.1, v1.1): "the server has new
//! commits on your branch". Sent from Rust; the WebView has no notification permissions.
//!
//! Only when the number of commits the branch is behind went *up* (so the same commits are
//! announced once), only while Tenajlo isn't the focused window (the toolbar already shows
//! it), and only if Settings → Repositories allows it.

use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_notification::NotificationExt;

/// What to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notice {
    pub title: String,
    pub body: String,
}

/// The notice for a background fetch that moved the branch from `before` to `after` commits
/// behind its upstream on `remote`, or `None` if nothing new arrived.
pub fn new_commits(
    repo_name: &str,
    branch: Option<&str>,
    remote: &str,
    before: u32,
    after: u32,
) -> Option<Notice> {
    let branch = branch?;
    if after <= before {
        return None;
    }
    let new = after - before;
    let commits = if new == 1 {
        "1 new commit".to_owned()
    } else {
        format!("{new} new commits")
    };
    Some(Notice {
        title: format!("New changes in {repo_name}"),
        body: format!("{remote} has {commits} on {branch}. Pull to get them."),
    })
}

/// Shows `notice` unless Tenajlo's window has focus. Failures (no notification service, or
/// permission denied) are logged and otherwise ignored.
pub fn show<R: Runtime>(app: &AppHandle<R>, notice: &Notice) {
    let focused = app
        .webview_windows()
        .values()
        .any(|w| w.is_focused().unwrap_or(false));
    if focused {
        return;
    }
    let shown = app
        .notification()
        .builder()
        .title(&notice.title)
        .body(&notice.body)
        .show();
    if let Err(e) = shown {
        tracing::warn!(error = %e, "could not show a notification");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn announces_only_new_commits() {
        let n = new_commits("pump-station", Some("main"), "origin", 0, 3).unwrap();
        assert_eq!(n.title, "New changes in pump-station");
        assert_eq!(
            n.body,
            "origin has 3 new commits on main. Pull to get them."
        );
        // Already behind by 2, now 3: one new.
        assert_eq!(
            new_commits("r", Some("main"), "origin", 2, 3).unwrap().body,
            "origin has 1 new commit on main. Pull to get them."
        );
        assert_eq!(new_commits("r", Some("main"), "origin", 3, 3), None);
        assert_eq!(new_commits("r", Some("main"), "origin", 3, 0), None);
        assert_eq!(
            new_commits("r", None, "origin", 0, 2),
            None,
            "detached HEAD"
        );
    }
}

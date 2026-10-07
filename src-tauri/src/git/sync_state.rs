//! What the toolbar sync button should do (spec §8.1). Pure: decided from status + remotes.

use serde::Serialize;

use super::parse::branches::remote_name;
use super::parse::status::WorkingDirectoryStatus;

/// The sync button's action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum SyncAction {
    /// The repository has no remotes.
    NoRemote,
    /// The branch isn't on the server yet (no upstream, or it was deleted there).
    Publish {
        remote: String,
    },
    /// The server has commits we don't (shown even if we also have new commits).
    Pull {
        remote: String,
    },
    Push {
        remote: String,
    },
    Fetch {
        remote: String,
    },
}

/// Sync button state for the current branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncState {
    pub action: SyncAction,
    pub branch: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// Unix seconds of the last fetch (FETCH_HEAD mtime), if any.
    pub last_fetched: Option<u32>,
}

/// `origin` if present, else the first remote.
pub fn default_remote(remotes: &[String]) -> Option<&str> {
    remotes
        .iter()
        .find(|r| *r == "origin")
        .or(remotes.first())
        .map(String::as_str)
}

/// Splits an upstream like `team/origin/feature/x` into (`team/origin`, `feature/x`).
pub fn split_upstream(upstream: &str, remotes: &[String]) -> Option<(String, String)> {
    let remote = remote_name(upstream, remotes)?;
    let branch = upstream.strip_prefix(remote.as_str())?.strip_prefix('/')?;
    Some((remote, branch.to_owned()))
}

/// Decides the sync action.
pub fn sync_state(
    status: &WorkingDirectoryStatus,
    remotes: &[String],
    last_fetched: Option<u32>,
) -> SyncState {
    let b = &status.branch;
    let base = |action| SyncState {
        action,
        branch: b.name.clone(),
        ahead: b.ahead,
        behind: b.behind,
        last_fetched,
    };
    let Some(default) = default_remote(remotes) else {
        return base(SyncAction::NoRemote);
    };
    let upstream = b
        .upstream
        .as_deref()
        .filter(|_| !b.upstream_gone)
        .and_then(|u| split_upstream(u, remotes));
    let remote = upstream
        .as_ref()
        .map_or(default, |(r, _)| r.as_str())
        .to_owned();

    // Detached HEAD or no commits yet: nothing to publish or push.
    if b.name.is_none() || b.tip.is_none() {
        return base(SyncAction::Fetch { remote });
    }
    let action = match upstream {
        None => SyncAction::Publish { remote },
        Some(_) if b.behind > 0 => SyncAction::Pull { remote },
        Some(_) if b.ahead > 0 => SyncAction::Push { remote },
        Some(_) => SyncAction::Fetch { remote },
    };
    base(action)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::status::BranchState;

    fn status(
        upstream: Option<&str>,
        ahead: u32,
        behind: u32,
        gone: bool,
    ) -> WorkingDirectoryStatus {
        WorkingDirectoryStatus {
            branch: BranchState {
                name: Some("main".into()),
                tip: Some("abc".into()),
                upstream: upstream.map(str::to_owned),
                ahead,
                behind,
                upstream_gone: gone,
            },
            ..Default::default()
        }
    }

    fn action(s: &WorkingDirectoryStatus, remotes: &[&str]) -> SyncAction {
        let remotes: Vec<String> = remotes.iter().map(|r| (*r).to_owned()).collect();
        sync_state(s, &remotes, None).action
    }

    #[test]
    fn picks_the_right_action() {
        let origin = || "origin".to_owned();
        assert_eq!(
            action(&status(None, 0, 0, false), &[]),
            SyncAction::NoRemote
        );
        assert_eq!(
            action(&status(None, 0, 0, false), &["upstream", "origin"]),
            SyncAction::Publish { remote: origin() }
        );
        assert_eq!(
            action(&status(Some("origin/main"), 0, 0, true), &["origin"]),
            SyncAction::Publish { remote: origin() }
        );
        assert_eq!(
            action(&status(Some("origin/main"), 2, 3, false), &["origin"]),
            SyncAction::Pull { remote: origin() }
        );
        assert_eq!(
            action(&status(Some("origin/main"), 2, 0, false), &["origin"]),
            SyncAction::Push { remote: origin() }
        );
        assert_eq!(
            action(&status(Some("origin/main"), 0, 0, false), &["origin"]),
            SyncAction::Fetch { remote: origin() }
        );
        assert_eq!(
            action(
                &status(Some("team/origin/main"), 0, 0, false),
                &["origin", "team/origin"]
            ),
            SyncAction::Fetch {
                remote: "team/origin".into()
            }
        );
        let mut detached = status(None, 0, 0, false);
        detached.branch.name = None;
        assert_eq!(
            action(&detached, &["origin"]),
            SyncAction::Fetch { remote: origin() }
        );
    }

    #[test]
    fn splits_upstreams() {
        let remotes = vec!["origin".to_owned(), "team/origin".to_owned()];
        assert_eq!(
            split_upstream("team/origin/feature/x", &remotes),
            Some(("team/origin".into(), "feature/x".into()))
        );
        assert_eq!(
            split_upstream("origin/main", &remotes),
            Some(("origin".into(), "main".into()))
        );
    }
}

//! Which remote operation a sync button click runs. Pure: decided from status + remotes, so
//! the UI only says what was clicked and Rust picks the remote and branches.

use serde::Deserialize;

use super::parse::status::WorkingDirectoryStatus;
use super::sync_state::{self, split_upstream, SyncAction};

/// What the user clicked. Rust re-derives the details (remote, branch) itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub enum SyncRequest {
    Fetch,
    Pull,
    /// "Merge the server's changes" after a fast-forward pull found diverged branches.
    PullMerge,
    Push,
    Publish,
}

/// The operation to run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Plan {
    Fetch {
        remote: String,
    },
    Pull {
        remote: String,
        /// Merge even if the user's pull strategy is fast-forward only.
        merge: bool,
    },
    Push {
        remote: String,
        branch: String,
        remote_branch: String,
    },
    /// Push a branch that has no upstream yet and set one.
    Publish {
        remote: String,
        branch: String,
    },
}

/// Why the click can't run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    #[error("the repository has no remotes")]
    NoRemote,
    #[error("the branch has no upstream")]
    NotPublished,
    #[error("HEAD is detached")]
    Detached,
}

impl Plan {
    /// The remote the operation talks to.
    pub fn remote(&self) -> &str {
        match self {
            Plan::Fetch { remote }
            | Plan::Pull { remote, .. }
            | Plan::Push { remote, .. }
            | Plan::Publish { remote, .. } => remote,
        }
    }

    /// True for pushes, which may use the remote's push URL.
    pub fn is_push(&self) -> bool {
        matches!(self, Plan::Push { .. } | Plan::Publish { .. })
    }
}

/// Decides what `request` runs for the current branch.
pub fn plan(
    request: SyncRequest,
    current: &WorkingDirectoryStatus,
    remotes: &[String],
) -> Result<Plan, PlanError> {
    let branch = current.branch.name.clone();
    let upstream = || {
        current
            .branch
            .upstream
            .as_deref()
            .filter(|_| !current.branch.upstream_gone)
            .and_then(|u| split_upstream(u, remotes))
            .ok_or(PlanError::NotPublished)
    };
    Ok(match request {
        SyncRequest::Fetch => match sync_state::sync_state(current, remotes, None).action {
            SyncAction::NoRemote => return Err(PlanError::NoRemote),
            SyncAction::Fetch { remote }
            | SyncAction::Pull { remote }
            | SyncAction::Push { remote }
            | SyncAction::Publish { remote } => Plan::Fetch { remote },
        },
        SyncRequest::Pull | SyncRequest::PullMerge => Plan::Pull {
            remote: upstream()?.0,
            merge: request == SyncRequest::PullMerge,
        },
        SyncRequest::Push => {
            let branch = branch.ok_or(PlanError::Detached)?;
            let (remote, remote_branch) = upstream()?;
            Plan::Push {
                remote,
                branch,
                remote_branch,
            }
        }
        SyncRequest::Publish => Plan::Publish {
            branch: branch.ok_or(PlanError::Detached)?,
            remote: sync_state::default_remote(remotes)
                .ok_or(PlanError::NoRemote)?
                .to_owned(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::status::BranchState;

    fn status(name: Option<&str>, upstream: Option<&str>, gone: bool) -> WorkingDirectoryStatus {
        WorkingDirectoryStatus {
            branch: BranchState {
                name: name.map(str::to_owned),
                tip: Some("abc".into()),
                upstream: upstream.map(str::to_owned),
                ahead: 0,
                behind: 0,
                upstream_gone: gone,
            },
            ..Default::default()
        }
    }

    fn run(
        request: SyncRequest,
        s: &WorkingDirectoryStatus,
        remotes: &[&str],
    ) -> Result<Plan, PlanError> {
        let remotes: Vec<String> = remotes.iter().map(|r| (*r).to_owned()).collect();
        plan(request, s, &remotes)
    }

    #[test]
    fn push_targets_the_upstream_branch() {
        let s = status(Some("feature"), Some("team/origin/feature/x"), false);
        let p = run(SyncRequest::Push, &s, &["origin", "team/origin"]).unwrap();
        assert_eq!(
            p,
            Plan::Push {
                remote: "team/origin".into(),
                branch: "feature".into(),
                remote_branch: "feature/x".into(),
            }
        );
        assert!(p.is_push());
        assert_eq!(p.remote(), "team/origin");
    }

    #[test]
    fn pull_needs_a_live_upstream() {
        let tracked = status(Some("main"), Some("origin/main"), false);
        assert_eq!(
            run(SyncRequest::PullMerge, &tracked, &["origin"]),
            Ok(Plan::Pull {
                remote: "origin".into(),
                merge: true
            })
        );
        let gone = status(Some("main"), Some("origin/main"), true);
        for request in [SyncRequest::Pull, SyncRequest::Push] {
            assert_eq!(
                run(request, &gone, &["origin"]),
                Err(PlanError::NotPublished)
            );
        }
    }

    #[test]
    fn publish_uses_the_default_remote() {
        let s = status(Some("new"), None, false);
        let p = run(SyncRequest::Publish, &s, &["upstream", "origin"]).unwrap();
        assert_eq!(
            p,
            Plan::Publish {
                remote: "origin".into(),
                branch: "new".into()
            }
        );
        assert_eq!(run(SyncRequest::Publish, &s, &[]), Err(PlanError::NoRemote));
    }

    #[test]
    fn detached_head_can_only_fetch_and_pull() {
        let s = status(None, None, false);
        assert_eq!(
            run(SyncRequest::Publish, &s, &["origin"]),
            Err(PlanError::Detached)
        );
        assert_eq!(
            run(SyncRequest::Fetch, &s, &["origin"]),
            Ok(Plan::Fetch {
                remote: "origin".into()
            })
        );
        assert_eq!(run(SyncRequest::Fetch, &s, &[]), Err(PlanError::NoRemote));
    }
}

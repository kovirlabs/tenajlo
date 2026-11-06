//! The remote Tenajlo treats as "the server" for a repository.

use std::path::Path;

use super::error::GitError;
use super::exec::GitBinary;
use super::sync_plan::{plan, SyncRequest};
use super::{remote, status};

/// The remote the sync button fetches from (the upstream's, else the default remote) and its
/// URL, or `None` without one. Used for LFS locks and Forgejo links.
pub async fn primary_remote(
    git: &GitBinary,
    root: &Path,
) -> Result<Option<(String, String)>, GitError> {
    let current = status::status(git, root).await?;
    let remotes = remote::list_remotes(git, root).await?;
    let Ok(plan) = plan(SyncRequest::Fetch, &current, &remotes) else {
        return Ok(None);
    };
    let name = plan.remote().to_owned();
    Ok(remote::remote_url(git, root, &name, false)
        .await?
        .map(|url| (name, url)))
}

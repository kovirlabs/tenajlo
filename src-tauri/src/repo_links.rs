//! Resolves the open repository to its Forgejo repository (spec §7, v1.4), cached briefly so
//! pull request and check lookups don't repeat `GET /repos/{owner}/{name}`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::Url;
use uuid::Uuid;

use crate::auth::accounts::{AccountError, AccountManager};
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::links::{self, RepoInfo};
use crate::forgejo::remote_repo::RepoRef;
use crate::forgejo::{ForgejoClient, ForgejoError};
use crate::git::error::GitError;
use crate::git::exec::GitBinary;
use crate::git::primary_remote::primary_remote;
use crate::store::accounts::AccountEntry;

/// How long a resolved (or unresolvable) repository is remembered.
const TTL: Duration = Duration::from_secs(5 * 60);

/// The open repository on its Forgejo server.
#[derive(Debug, Clone)]
pub struct Linked {
    pub account: AccountEntry,
    pub base: Url,
    pub repo: RepoRef,
    pub info: RepoInfo,
}

/// Why a lookup failed.
#[derive(Debug, thiserror::Error)]
pub enum LinkError {
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Account(#[from] AccountError),
    /// A Forgejo call made with `account`'s token failed.
    #[error("{error}")]
    Api {
        account: Box<AccountEntry>,
        error: ForgejoError,
    },
}

type Entry = (Instant, Option<Arc<Linked>>);

/// Per-repository cache of [`Linked`] lookups.
#[derive(Default)]
pub struct RepoLinks(Mutex<HashMap<Uuid, Entry>>);

impl RepoLinks {
    /// The Forgejo repository behind `root`'s primary remote, or `None` when the remote isn't
    /// on a signed-in account's server (or the server doesn't know it).
    pub async fn resolve(
        &self,
        accounts: &AccountManager,
        client: &ForgejoClient,
        git: &GitBinary,
        id: Uuid,
        root: &Path,
    ) -> Result<Option<Arc<Linked>>, LinkError> {
        if let Some((at, linked)) = self.lock().get(&id).cloned() {
            if at.elapsed() < TTL {
                return Ok(linked);
            }
        }
        let Some(found) = lookup(accounts, client, git, root).await? else {
            // No account for the remote: cheap to check again (no network call), and the user
            // may sign in at any moment.
            return Ok(None);
        };
        let linked = found.map(Arc::new);
        self.lock().insert(id, (Instant::now(), linked.clone()));
        Ok(linked)
    }

    /// Forgets `id`'s lookup (its remote may have changed).
    pub fn forget(&self, id: Uuid) {
        self.lock().remove(&id);
    }

    /// Forgets every lookup (an account was added, replaced or removed).
    pub fn clear(&self) {
        self.lock().clear();
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<Uuid, Entry>> {
        self.0.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// `None`: no remote, or no account for it (not cached). `Some(None)`: the account's server
/// doesn't know the repository (cached).
async fn lookup(
    accounts: &AccountManager,
    client: &ForgejoClient,
    git: &GitBinary,
    root: &Path,
) -> Result<Option<Option<Linked>>, LinkError> {
    let Some((_, url)) = primary_remote(git, root).await? else {
        return Ok(None);
    };
    let Some((account, repo)) = accounts.repo_for_remote(&url) else {
        return Ok(None);
    };
    let token = accounts.usable_token(&account).await?;
    let api = |error| LinkError::Api {
        account: Box::new(account.clone()),
        error,
    };
    let base = normalize_base_url(&account.base_url).map_err(api)?;
    match links::repo_info(client, &base, &token, &repo).await {
        Ok(info) => Ok(Some(Some(Linked {
            account,
            base,
            repo,
            info,
        }))),
        // Renamed, deleted, or not visible to this account: no links.
        Err(ForgejoError::Http { status: 404, .. }) => Ok(Some(None)),
        Err(e) => Err(api(e)),
    }
}

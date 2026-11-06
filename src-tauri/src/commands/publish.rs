//! "Publish repository" (spec §8.1, v1.2): create the repository on Forgejo and connect the
//! local one to it as `origin`. The UI then runs the normal "Publish branch" sync.

use std::path::Path;

use serde::Serialize;
use tauri::State;

use super::parse_id;
use crate::error::AppError;
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::publish::{self, CreateRepoError, NewRepository, Owner, PUBLISH_SCOPES};
use crate::forgejo::repos::RemoteRepository;
use crate::git::exec::GitBinary;
use crate::git::{remote, status};
use crate::state::AppState;

/// The remote name a published repository gets.
const ORIGIN: &str = "origin";

/// Where an account may publish to.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PublishOptions {
    pub account_id: String,
    /// The signed-in user, the default owner.
    pub login: String,
    /// Whether the token may create repositories for the user.
    pub user_allowed: bool,
    /// Organizations the user may create repositories in, sorted. Empty when the token can't.
    pub organizations: Vec<String>,
    /// The token can't list or create in organizations; a new token would allow it.
    pub organizations_need_token: bool,
    /// Permissions for a replacement token.
    pub required_scopes: Vec<String>,
}

/// What `account_id`'s token allows for publishing. Creates nothing.
#[tauri::command]
#[specta::specta]
pub async fn get_publish_options(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<PublishOptions, AppError> {
    let account = super::account(&state, &account_id)?;
    let token = state.accounts.usable_token(&account).await?;
    let base = normalize_base_url(&account.base_url)?;
    let api_error = |e| super::account_api_error(&state, &account, e);
    let client = &state.forgejo;

    let (user_allowed, orgs) = tokio::join!(
        publish::can_create_for_user(client, &base, &token),
        publish::creatable_orgs(client, &base, &token, &account.login),
    );
    let user_allowed = user_allowed.map_err(api_error)?;
    // Listing needs read:organization, creating needs write:organization: check the latter.
    let organizations = match orgs.map_err(api_error)? {
        Some(orgs) => match orgs.first() {
            Some(first) => publish::can_create_in_orgs(client, &base, &token, first)
                .await
                .map_err(api_error)?
                .then_some(orgs),
            None => Some(orgs),
        },
        None => None,
    };
    Ok(PublishOptions {
        account_id,
        login: account.login.clone(),
        user_allowed,
        organizations_need_token: organizations.is_none(),
        organizations: organizations.unwrap_or_default(),
        required_scopes: PUBLISH_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
    })
}

/// Creates `name` on `account_id`'s server, under `organization` or the user, and adds it as
/// `origin`. With `ssh`, the SSH address is used if the server has SSH turned on.
#[tauri::command]
#[specta::specta]
#[allow(clippy::too_many_arguments)] // one per dialog field
pub async fn publish_repository(
    state: State<'_, AppState>,
    repo_id: String,
    account_id: String,
    organization: Option<String>,
    name: String,
    description: String,
    is_private: bool,
    ssh: bool,
) -> Result<RemoteRepository, AppError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(AppError::invalid_input("Give the repository a name first."));
    }
    let id = parse_id(&repo_id)?;
    let account = super::account(&state, &account_id)?;
    let (root, _guard) = state.repos.lock_repo(id).await?;
    let git = state.git()?;

    // Check everything local first, so nothing is created on the server for a repository
    // that can't be pushed.
    if !remote::list_remotes(&git, &root).await?.is_empty() {
        return Err(AppError::invalid_input(
            "This repository is already connected to a server.",
        ));
    }
    let current = status::status(&git, &root).await?;
    if current.branch.tip.is_none() {
        return Err(AppError::invalid_input(
            "Make your first commit, then publish.",
        ));
    }
    if current.branch.name.is_none() {
        return Err(AppError::invalid_input("Switch to a branch first."));
    }

    let token = state.accounts.usable_token(&account).await?;
    let base = normalize_base_url(&account.base_url)?;
    let owner = match organization.as_deref() {
        Some(org) => Owner::Org(org),
        None => Owner::User,
    };
    let new = NewRepository {
        name,
        description: description.trim(),
        private: is_private,
    };
    let created = publish::create_repository(&state.forgejo, &base, &token, owner, &new)
        .await
        .map_err(|e| create_error(&state, &account, e, owner, name))?;
    tracing::info!(server = %crate::redact::redact(&account.base_url), "created a repository to publish to");

    // The repository now has a remote: look its Forgejo links up again.
    state.repo_links.forget(id);
    connect_origin(&git, &root, &created, ssh).await?;
    Ok(created)
}

/// Adds the just-created repository as `origin`, using its SSH address with `ssh` when the
/// server has one. On failure the message says the repository was already created.
async fn connect_origin(
    git: &GitBinary,
    root: &Path,
    created: &RemoteRepository,
    ssh: bool,
) -> Result<(), AppError> {
    let url = if ssh && !created.ssh_url.is_empty() {
        &created.ssh_url
    } else {
        &created.clone_url
    };
    remote::add_remote(git, root, ORIGIN, url)
        .await
        .map_err(|e| {
            AppError::internal(
                format!(
                    "Tenajlo created {} on the server but couldn't connect this repository to it.",
                    created.full_name
                ),
                e.to_string(),
            )
        })
}

fn create_error(
    state: &AppState,
    account: &crate::store::accounts::AccountEntry,
    err: CreateRepoError,
    owner: Owner<'_>,
    name: &str,
) -> AppError {
    let place = match owner {
        Owner::User => "your account".to_owned(),
        Owner::Org(org) => org.to_owned(),
    };
    match err {
        CreateRepoError::NotAllowed => AppError::invalid_input(format!(
            "Your saved token can't create repositories in {place}. Create a new token with these permissions: {}.",
            PUBLISH_SCOPES.join(", ")
        )),
        CreateRepoError::NoPermission => AppError::invalid_input(format!(
            "You don't have permission to create repositories in {place}. Ask an owner of {place}, or publish to your own account."
        )),
        CreateRepoError::NameTaken => AppError::invalid_input(format!(
            "There's already a repository named {name} in {place}. Choose another name."
        )),
        CreateRepoError::InvalidName => AppError::invalid_input(
            "That name can't be used. Use letters, numbers, dashes (-), underscores (_) and dots (.), up to 100 characters.",
        ),
        CreateRepoError::Api(e) => super::account_api_error(state, account, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::AppErrorKind;
    use crate::git::test_support::{git, git_in, init_repo};

    fn created(ssh_url: &str) -> RemoteRepository {
        RemoteRepository {
            full_name: "alice/plc".to_owned(),
            owner: "alice".to_owned(),
            name: "plc".to_owned(),
            description: String::new(),
            private: true,
            archived: false,
            clone_url: "https://git.example.com/alice/plc.git".to_owned(),
            ssh_url: ssh_url.to_owned(),
        }
    }

    async fn origin_url(repo: &Path) -> String {
        git_in(repo, &["remote", "get-url", "origin"])
            .await
            .trim()
            .to_owned()
    }

    #[tokio::test]
    async fn connects_origin_preferring_ssh_when_offered() {
        let (_tmp, repo) = init_repo().await;
        let ssh = "ssh://git@git.example.com:2222/alice/plc.git";
        connect_origin(&git(), &repo, &created(ssh), true)
            .await
            .unwrap();
        assert_eq!(origin_url(&repo).await, ssh);

        // No SSH on the server: HTTPS even when SSH was asked for.
        let (_tmp, repo) = init_repo().await;
        connect_origin(&git(), &repo, &created(""), true)
            .await
            .unwrap();
        assert_eq!(
            origin_url(&repo).await,
            "https://git.example.com/alice/plc.git"
        );
    }

    #[tokio::test]
    async fn failing_to_add_origin_says_the_repository_was_created() {
        let (_tmp, repo) = init_repo().await;
        // Something added `origin` after the up-front check: `git remote add` fails.
        git_in(
            &repo,
            &[
                "remote",
                "add",
                "origin",
                "https://elsewhere.example.com/x.git",
            ],
        )
        .await;

        let err = connect_origin(&git(), &repo, &created(""), false)
            .await
            .unwrap_err();
        assert_eq!(err.kind, AppErrorKind::Internal);
        assert_eq!(
            err.message,
            "Tenajlo created alice/plc on the server but couldn't connect this repository to it."
        );
        assert!(err.details.is_some_and(|d| !d.is_empty()));
        // The existing remote is left alone.
        assert_eq!(
            origin_url(&repo).await,
            "https://elsewhere.example.com/x.git"
        );
    }
}

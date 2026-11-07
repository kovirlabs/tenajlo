//! Read-only Forgejo links for the open repository (spec §8.1, v1.4): the branch's pull
//! request, a commit's checks, and opening those pages in the browser.

use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use super::{account_api_error, parse_id};
use crate::error::AppError;
use crate::forgejo::links::{self, CommitChecks, ForgejoPage, PullRequest};
use crate::git::{branch_name, branches, log};
use crate::repo_links::{LinkError, Linked};
use crate::state::AppState;

/// The current branch on Forgejo.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchLink {
    /// `owner/name` on the server.
    pub full_name: String,
    pub default_branch: String,
    /// The branch's name on the server (its upstream's), which can differ from the local
    /// name; the local name when it has no upstream yet.
    pub server_branch: String,
    /// The open pull request from this branch, if any.
    pub pull: Option<PullRequest>,
}

async fn linked(state: &AppState, repo_id: &str) -> Result<Option<Arc<Linked>>, AppError> {
    let id = parse_id(repo_id)?;
    let root = state.repos.root(id)?;
    let git = state.git()?;
    state
        .repo_links
        .resolve(&state.accounts, &state.forgejo, &git, id, &root)
        .await
        .map_err(|e| link_error(state, e))
}

fn link_error(state: &AppState, e: LinkError) -> AppError {
    match e {
        LinkError::Git(e) => e.into(),
        LinkError::Account(e) => e.into(),
        LinkError::Api { account, error } => account_api_error(state, &account, error),
    }
}

/// The branch's repository on Forgejo and its open pull request. `None` when the remote isn't
/// on a signed-in account's server.
#[tauri::command]
#[specta::specta]
pub async fn get_branch_link(
    state: State<'_, AppState>,
    repo_id: String,
    branch: String,
) -> Result<Option<BranchLink>, AppError> {
    let Some(l) = linked(&state, &repo_id).await? else {
        return Ok(None);
    };
    let root = state.repos.root(parse_id(&repo_id)?)?;
    let git = state.git()?;
    branch_name::validate(&git, &root, &branch).await?;
    let server_branch = branches::server_branch(&git, &root, &branch)
        .await?
        .unwrap_or(branch);
    let pull = links::pull_for_branch(
        &state.forgejo,
        &l.base,
        &token(&state, &l).await?,
        &l.repo,
        &server_branch,
    )
    .await
    .map_err(|e| account_api_error(&state, &l.account, e))?;
    Ok(Some(BranchLink {
        full_name: l.info.full_name.clone(),
        default_branch: l.info.default_branch.clone(),
        server_branch,
        pull,
    }))
}

/// A commit's checks (Forgejo Actions and other CI). `None` when the repository isn't linked.
#[tauri::command]
#[specta::specta]
pub async fn get_commit_checks(
    state: State<'_, AppState>,
    repo_id: String,
    sha: String,
) -> Result<Option<CommitChecks>, AppError> {
    check_sha(&sha)?;
    let Some(l) = linked(&state, &repo_id).await? else {
        return Ok(None);
    };
    let checks = links::commit_checks(
        &state.forgejo,
        &l.base,
        &token(&state, &l).await?,
        &l.repo,
        &sha,
    )
    .await
    .map_err(|e| account_api_error(&state, &l.account, e))?;
    Ok(Some(checks))
}

/// Opens a page of the repository's website. URLs are built in Rust from the `html_url` the
/// server returned; the WebView never supplies one.
#[tauri::command]
#[specta::specta]
pub async fn open_forgejo_page(
    app: AppHandle,
    state: State<'_, AppState>,
    repo_id: String,
    page: ForgejoPage,
) -> Result<(), AppError> {
    match &page {
        ForgejoPage::Commit { sha } => check_sha(sha)?,
        ForgejoPage::NewPullRequest { branch } => {
            let root = state.repos.root(parse_id(&repo_id)?)?;
            branch_name::validate(&state.git()?, &root, branch).await?;
        }
        ForgejoPage::Repository | ForgejoPage::PullRequest { .. } => {}
    }
    let Some(l) = linked(&state, &repo_id).await? else {
        return Err(AppError::invalid_input(
            "This repository isn't on a Forgejo server you're signed in to.",
        ));
    };
    let url = links::page_url(&l.info, &page)?;
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| AppError::internal("Tenajlo couldn't open your web browser.", e.to_string()))
}

async fn token(state: &AppState, l: &Linked) -> Result<crate::auth::secrets::Secret, AppError> {
    Ok(state.accounts.usable_token(&l.account).await?)
}

fn check_sha(sha: &str) -> Result<(), AppError> {
    if log::is_commit_hash(sha) {
        Ok(())
    } else {
        Err(AppError::invalid_input("That commit id isn't valid."))
    }
}

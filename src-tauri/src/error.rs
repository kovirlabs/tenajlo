//! Serializable error returned by every Tauri command.

use serde::Serialize;

use crate::auth::accounts::AccountError;
use crate::forgejo::ForgejoError;
use crate::git::clone::CloneError;
use crate::git::error::{GitError, GitErrorKind};
use crate::repo_manager::RepoError;
use crate::store::accounts::AccountEntry;

/// Broad category the UI uses to pick a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum AppErrorKind {
    GitNotFound,
    GitUnsupported,
    GitTimedOut,
    GitCancelled,
    Git,
    UnknownRepository,
    /// A Forgejo server request failed (sign-in, repository list).
    Server,
    UnknownAccount,
    /// The account's token no longer works; `account_id` says which. UI offers "Sign in again".
    SignInRequired,
    Storage,
    InvalidInput,
    Internal,
}

/// Error shape sent to the frontend. Never contains secrets: `details` is redacted.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub kind: AppErrorKind,
    /// Set when `kind == Git`.
    pub git_kind: Option<GitErrorKind>,
    /// Plain-language message for non-Git-experts.
    pub message: String,
    /// Technical details shown behind "Details".
    pub details: Option<String>,
    /// The account this error is about (`SignInRequired`, account permission problems).
    pub account_id: Option<String>,
}

impl AppError {
    /// An error with a plain message and technical details (redacted here).
    pub fn with_details(
        kind: AppErrorKind,
        message: impl Into<String>,
        details: impl AsRef<str>,
    ) -> Self {
        Self::new(kind, message, Some(crate::redact::redact(details.as_ref())))
    }

    /// Rejected command input (bad id, bad path).
    pub fn invalid_input(message: impl Into<String>) -> Self {
        Self::new(AppErrorKind::InvalidInput, message, None)
    }

    /// The account's token was rejected (or is gone); the user must paste a new one.
    pub fn sign_in_required(account: &AccountEntry, details: Option<String>) -> Self {
        Self {
            account_id: Some(account.id.to_string()),
            ..Self::new(
                AppErrorKind::SignInRequired,
                format!(
                    "Your sign-in for {} has stopped working. The access token may have expired or been deleted. Sign in again with a new token.",
                    account.base_url
                ),
                details,
            )
        }
    }

    /// git's auth failed although the account's token is valid: a permission problem.
    pub fn account_lacks_access(account: &AccountEntry, git_error: AppError) -> Self {
        Self {
            account_id: Some(account.id.to_string()),
            message: format!(
                "{} didn't allow this as {}. You may not have access to this repository, or your access token may be missing the write:repository permission needed to push.",
                account.base_url, account.login
            ),
            ..git_error
        }
    }

    fn new(kind: AppErrorKind, message: impl Into<String>, details: Option<String>) -> Self {
        Self {
            kind,
            git_kind: None,
            message: message.into(),
            details,
            account_id: None,
        }
    }
}

impl From<GitError> for AppError {
    fn from(err: GitError) -> Self {
        match err {
            GitError::NotFound { searched } => AppError::new(
                AppErrorKind::GitNotFound,
                "Tenajlo couldn't find Git on this computer. Install Git 2.40 or newer, then reopen Tenajlo.",
                Some(
                    searched
                        .iter()
                        .map(|p| format!("Looked in: {}", p.display()))
                        .collect::<Vec<_>>()
                        .join("\n"),
                ),
            ),
            GitError::Unsupported { found, minimum } => AppError::new(
                AppErrorKind::GitUnsupported,
                format!(
                    "Tenajlo needs Git {minimum} or newer. This computer has Git {found}. Update Git, then reopen Tenajlo."
                ),
                None,
            ),
            GitError::TimedOut => AppError::new(
                AppErrorKind::GitTimedOut,
                "Git took too long to respond and was stopped.",
                None,
            ),
            GitError::Cancelled => AppError::new(AppErrorKind::GitCancelled, "The operation was cancelled.", None),
            GitError::Failed { kind, stderr, .. } => AppError {
                kind: AppErrorKind::Git,
                git_kind: Some(kind),
                message: git_message(kind).into(),
                details: Some(stderr),
                account_id: None,
            },
            GitError::Spawn(e) => {
                AppError::new(AppErrorKind::Internal, "Tenajlo couldn't start Git.", Some(e.to_string()))
            }
            GitError::InvalidRefName(name) => AppError::new(
                AppErrorKind::InvalidInput,
                format!("“{name}” can't be used as a branch name."),
                None,
            ),
            GitError::Parse(s) => AppError::new(
                AppErrorKind::Internal,
                "Git returned something Tenajlo didn't understand.",
                Some(crate::redact::redact(&s)),
            ),
        }
    }
}

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::UnknownRepository => AppError::new(
                AppErrorKind::UnknownRepository,
                "That repository is no longer in your list.",
                None,
            ),
            RepoError::Git(e) => e.into(),
            RepoError::Store(e) => AppError::new(
                AppErrorKind::Storage,
                "Tenajlo couldn't save your repository list.",
                Some(crate::redact::redact(&e.to_string())),
            ),
        }
    }
}

impl From<ForgejoError> for AppError {
    fn from(err: ForgejoError) -> Self {
        let details = err.to_string();
        let (kind, message) = match &err {
            ForgejoError::InvalidUrl(reason) => (
                AppErrorKind::InvalidInput,
                format!("That server address can't be used: {reason}."),
            ),
            ForgejoError::TlsUntrusted(_) => (
                AppErrorKind::Server,
                git_message(GitErrorKind::TlsUntrusted).to_owned(),
            ),
            ForgejoError::Unreachable(_) => (
                AppErrorKind::Server,
                "Couldn't reach the server. Check the address and your network or VPN connection.".to_owned(),
            ),
            ForgejoError::TimedOut => (
                AppErrorKind::Server,
                "The server took too long to respond. Try again in a moment.".to_owned(),
            ),
            ForgejoError::NotForgejo(_) => (
                AppErrorKind::Server,
                "That address doesn't look like a Forgejo server. Check it and try again.".to_owned(),
            ),
            ForgejoError::Unauthorized => (
                AppErrorKind::Server,
                "The server didn't accept that token. Check that you copied all of it and that it hasn't expired or been deleted.".to_owned(),
            ),
            ForgejoError::MissingScope(_) => (
                AppErrorKind::Server,
                format!(
                    "That token is missing a permission Tenajlo needs. Create a new token with these permissions: {}.",
                    crate::forgejo::client::REQUIRED_SCOPES.join(", ")
                ),
            ),
            ForgejoError::Http { status, .. } => (
                AppErrorKind::Server,
                format!("The server reported a problem (error {status}). Try again later."),
            ),
            ForgejoError::Other(_) => (AppErrorKind::Server, "Couldn't talk to the server.".to_owned()),
        };
        AppError::with_details(kind, message, details)
    }
}

impl From<CloneError> for AppError {
    fn from(err: CloneError) -> Self {
        match err {
            CloneError::Git(e) => e.into(),
            other => AppError::with_details(
                AppErrorKind::InvalidInput,
                clone_message(&other),
                other.to_string(),
            ),
        }
    }
}

fn clone_message(err: &CloneError) -> String {
    match err {
        CloneError::InvalidUrl(_) => "Tenajlo can only clone https:// addresses for now (SSH comes in a later version). The address must not include a password.".to_owned(),
        CloneError::DestinationExists(path) => format!(
            "A folder already exists at {}. Choose another name or location.",
            path.display()
        ),
        CloneError::InvalidDestination(_) => "Choose a folder for the repository.".to_owned(),
        CloneError::Io { .. } => "Tenajlo couldn't create the folder for the repository.".to_owned(),
        CloneError::Git(_) => git_message(GitErrorKind::Unknown).to_owned(),
    }
}

impl From<AccountError> for AppError {
    fn from(err: AccountError) -> Self {
        match err {
            AccountError::UnknownAccount => AppError::new(
                AppErrorKind::UnknownAccount,
                "That account is no longer signed in.",
                None,
            ),
            AccountError::ReadOnly => AppError::new(
                AppErrorKind::Storage,
                "Your accounts were saved by a newer version of Tenajlo, so they can't be changed here. Update Tenajlo.",
                None,
            ),
            AccountError::Secret(e) => AppError::with_details(
                AppErrorKind::Storage,
                "Tenajlo couldn't use this computer's password store (keychain).",
                e.to_string(),
            ),
            AccountError::Store(e) => AppError::with_details(
                AppErrorKind::Storage,
                "Tenajlo couldn't save your accounts.",
                e.to_string(),
            ),
        }
    }
}

/// Plain-language message for a classified git failure (spec §5.5).
fn git_message(kind: GitErrorKind) -> &'static str {
    match kind {
        GitErrorKind::AuthFailed => "The server didn't accept your sign-in. Check your account or token.",
        GitErrorKind::HostKeyUnknown => "The server's identity couldn't be confirmed.",
        GitErrorKind::SshKeyRejected => "The server didn't accept your SSH key.",
        GitErrorKind::PushRejected => "The server has changes you don't have yet. Pull first, then push.",
        GitErrorKind::PullDiverged => "Your branch and the server's branch have both changed.",
        GitErrorKind::LocalChangesBlock => "You have changes that would be overwritten.",
        GitErrorKind::TlsUntrusted => {
            "This computer doesn't trust the server's certificate. Your IT team may need to install the company certificate."
        }
        GitErrorKind::HostUnreachable => "Couldn't reach the server. Check your network or VPN connection.",
        GitErrorKind::MergeConflict => "Some files have conflicts that need to be resolved.",
        GitErrorKind::NotARepository => "This folder isn't a Git repository.",
        GitErrorKind::DubiousOwnership => {
            "Git won't open this folder because it belongs to a different user account. Ask IT to fix the folder's owner, or mark it as safe with `git config --global --add safe.directory <path>`."
        }
        GitErrorKind::IdentityMissing => "Git needs your name and email before you can commit.",
        GitErrorKind::BranchExists => "A branch with that name already exists.",
        GitErrorKind::BranchNotMerged => "This branch has commits that aren't on any other branch.",
        GitErrorKind::RepositoryNotFound => {
            "The server couldn't find that repository. Check the address, and that your account has access to it."
        }
        GitErrorKind::Unknown => "Git reported a problem.",
    }
}

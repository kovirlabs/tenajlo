//! Serializable error returned by every Tauri command.

use serde::Serialize;

use crate::auth::accounts::AccountError;
use crate::auth::remote_auth::PrepareError;
use crate::auth::saved_secrets::SavedSecretError;
use crate::auth::ssh_keys::SshKeyError;
use crate::editor::EditorError;
use crate::forgejo::ForgejoError;
use crate::git::clone::CloneError;
use crate::git::commit::CommitBlocker;
use crate::git::error::{GitError, GitErrorKind};
use crate::git::init::InitError;
use crate::git::sync_plan::PlanError;
use crate::repo_manager::RepoError;
use crate::settings::SettingsError;
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

    /// Something on this computer failed (opening a folder, starting a program); `details`
    /// says what.
    pub fn internal(message: impl Into<String>, details: impl AsRef<str>) -> Self {
        Self::with_details(AppErrorKind::Internal, message, details)
    }

    /// The repository stores files with Git LFS but git-lfs isn't installed here.
    pub fn lfs_missing() -> Self {
        Self::invalid_input(
            "This repository stores large files with Git LFS, which isn't installed on this computer. Install Git LFS (git-lfs.com), then restart Tenajlo.",
        )
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
                message: kind.user_message().into(),
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

impl From<PrepareError> for AppError {
    fn from(err: PrepareError) -> Self {
        match err {
            PrepareError::SignInRequired(account) => AppError::sign_in_required(&account, None),
            PrepareError::Trampoline(e) => {
                AppError::internal("Couldn't prepare sign-in prompts.", e.to_string())
            }
        }
    }
}

impl From<CommitBlocker> for AppError {
    fn from(blocker: CommitBlocker) -> Self {
        AppError::invalid_input(match blocker {
            CommitBlocker::Conflicts => "Resolve the conflicted files before committing.",
            CommitBlocker::LfsMissing => return AppError::lfs_missing(),
            CommitBlocker::OperationInProgress => {
                "Finish or abort the operation in progress before committing."
            }
            CommitBlocker::NothingStaged => "Select at least one file to include in the commit.",
        })
    }
}

impl From<PlanError> for AppError {
    fn from(err: PlanError) -> Self {
        AppError::invalid_input(match err {
            PlanError::NoRemote => "This repository isn't connected to a server.",
            PlanError::NotPublished => "This branch isn't on the server yet. Publish it first.",
            PlanError::Detached => "Switch to a branch first.",
        })
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
            RepoError::OutsideRepository => {
                AppError::invalid_input("That file isn't in this repository.")
            }
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
                GitErrorKind::TlsUntrusted.user_message().to_owned(),
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
        CloneError::InvalidUrl(_) => "Enter an https:// or SSH address, like https://server/team/project.git or git@server:team/project.git. The address must not include a password.".to_owned(),
        CloneError::DestinationExists(path) => format!(
            "A folder already exists at {}. Choose another name or location.",
            path.display()
        ),
        CloneError::InvalidDestination(_) => "Choose a folder for the repository.".to_owned(),
        CloneError::Io { .. } => "Tenajlo couldn't create the folder for the repository.".to_owned(),
        CloneError::Git(_) => GitErrorKind::Unknown.user_message().to_owned(),
    }
}

impl From<SettingsError> for AppError {
    fn from(err: SettingsError) -> Self {
        match err {
            SettingsError::Invalid(message) => AppError::invalid_input(message),
            SettingsError::Store(e) => AppError::with_details(
                AppErrorKind::Storage,
                "Tenajlo couldn't save your settings.",
                e.to_string(),
            ),
        }
    }
}

impl From<InitError> for AppError {
    fn from(err: InitError) -> Self {
        match err {
            InitError::Git(e) => e.into(),
            other => AppError::with_details(
                AppErrorKind::InvalidInput,
                init_message(&other),
                other.to_string(),
            ),
        }
    }
}

fn init_message(err: &InitError) -> String {
    match err {
        InitError::InvalidName => "That name can't be used for a folder. Avoid characters like / \\ : * ? \" < > | and a dot or space at the end.".to_owned(),
        InitError::InvalidLocation(_) => "Choose a folder for the new repository.".to_owned(),
        InitError::NotEmpty(path) => format!(
            "{} already has files in it. To use them, choose Add local… instead, or pick another name.",
            path.display()
        ),
        InitError::Io { .. } => "Tenajlo couldn't create the folder for the repository.".to_owned(),
        InitError::Git(_) => GitErrorKind::Unknown.user_message().to_owned(),
    }
}

impl From<EditorError> for AppError {
    fn from(err: EditorError) -> Self {
        let message = match &err {
            EditorError::NotFound(name) => {
                format!("{name} isn't installed. Choose another editor in Settings → Repositories.")
            }
            EditorError::Spawn { .. } => {
                "Tenajlo couldn't start your editor. Check the editor in Settings → Repositories."
                    .to_owned()
            }
        };
        AppError::with_details(AppErrorKind::InvalidInput, message, err.to_string())
    }
}

impl From<SshKeyError> for AppError {
    fn from(err: SshKeyError) -> Self {
        let (kind, message) = match &err {
            SshKeyError::Exists(_) => (
                AppErrorKind::InvalidInput,
                "You already have an SSH key named id_ed25519. Use that key instead of creating a new one.",
            ),
            SshKeyError::NotFound(_) => (
                AppErrorKind::InvalidInput,
                "That SSH key isn't in your .ssh folder anymore.",
            ),
            SshKeyError::Invalid(_) => (
                AppErrorKind::InvalidInput,
                "That file isn't an SSH public key Tenajlo can read.",
            ),
            SshKeyError::Generate(_) | SshKeyError::Io { .. } => (
                AppErrorKind::Internal,
                "Tenajlo couldn't save the SSH key in your .ssh folder.",
            ),
        };
        AppError::with_details(kind, message, err.to_string())
    }
}

impl From<SavedSecretError> for AppError {
    fn from(err: SavedSecretError) -> Self {
        let (kind, message) = match &err {
            SavedSecretError::Unknown => (
                AppErrorKind::InvalidInput,
                "That password was already forgotten.",
            ),
            SavedSecretError::ReadOnly => (
                AppErrorKind::Storage,
                "Your saved passwords were changed by a newer version of Tenajlo, so they can't be changed here. Update Tenajlo.",
            ),
            SavedSecretError::Secret(_) => (
                AppErrorKind::Storage,
                "Tenajlo couldn't reach this computer's password store.",
            ),
            SavedSecretError::Store(_) => (
                AppErrorKind::Storage,
                "Tenajlo couldn't save the list of remembered passwords.",
            ),
        };
        AppError::with_details(kind, message, err.to_string())
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
            AccountError::SignInRequired(account) => AppError::sign_in_required(&account, None),
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

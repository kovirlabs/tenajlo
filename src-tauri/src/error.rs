//! Serializable error returned by every Tauri command.

use serde::Serialize;

use crate::auth::accounts::AccountError;
use crate::auth::remote_auth::PrepareError;
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
        CloneError::InvalidUrl(_) => "Enter an https:// or SSH address, like https://server/team/project.git or git@server:team/project.git. The address must not include a password.".to_owned(),
        CloneError::DestinationExists(path) => format!(
            "A folder already exists at {}. Choose another name or location.",
            path.display()
        ),
        CloneError::InvalidDestination(_) => "Choose a folder for the repository.".to_owned(),
        CloneError::Io { .. } => "Tenajlo couldn't create the folder for the repository.".to_owned(),
        CloneError::Git(_) => git_message(GitErrorKind::Unknown).to_owned(),
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
        InitError::Git(_) => git_message(GitErrorKind::Unknown).to_owned(),
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

/// Plain-language message for a classified git failure (spec §5.5).
fn git_message(kind: GitErrorKind) -> &'static str {
    match kind {
        GitErrorKind::AuthFailed => "The server didn't accept your sign-in. Check your account or token.",
        GitErrorKind::HostKeyUnknown => {
            "The server's identity wasn't confirmed, so Tenajlo didn't connect. Try again and check the fingerprint with your IT team."
        }
        GitErrorKind::HostKeyChanged => {
            "The server's identity has changed since you last connected. This can mean someone is intercepting the connection. Don't continue: contact your IT team."
        }
        GitErrorKind::SshKeyRejected => {
            "The server didn't accept your SSH key. Add your public key (for example id_ed25519.pub in your .ssh folder) to your Forgejo account under Settings → SSH / GPG Keys, or clone with HTTPS instead."
        }
        GitErrorKind::PushRejected => "The server has changes you don't have yet. Pull first, then push.",
        GitErrorKind::PullDiverged => {
            "Your branch and the server's branch both have new commits. Merge the server's changes into your branch to continue."
        }
        GitErrorKind::LocalChangesBlock => "You have changes that would be overwritten.",
        GitErrorKind::TlsUntrusted => {
            "This computer doesn't trust the server's certificate. Your IT team may need to install the company certificate."
        }
        GitErrorKind::HostUnreachable => "Couldn't reach the server. Check your network or VPN connection.",
        GitErrorKind::MergeConflict => {
            "Some files were changed both on the server and by you. Resolve the conflicts, then commit to finish."
        }
        GitErrorKind::NotARepository => "This folder isn't a Git repository.",
        GitErrorKind::DubiousOwnership => {
            "Git won't open this folder because it belongs to a different user account. Ask IT to fix the folder's owner, or mark it as safe with `git config --global --add safe.directory <path>`."
        }
        GitErrorKind::IdentityMissing => "Git needs your name and email before you can commit.",
        GitErrorKind::BranchExists => "A branch with that name already exists.",
        GitErrorKind::BranchNotMerged => "This branch has commits that aren't on any other branch.",
        GitErrorKind::ProtectedBranch => {
            "The server doesn't allow pushing directly to this branch. Create a new branch for your changes, push that, and open a pull request on Forgejo."
        }
        GitErrorKind::PushDeclined => "The server refused this push. Its explanation is in the details.",
        GitErrorKind::PushTooLarge => {
            "The server refused the upload because it's too large. Large files like CAD models belong in Git LFS; ask the repository owner to set it up for those file types."
        }
        GitErrorKind::RemoteBranchMissing => {
            "This branch no longer exists on the server. Someone may have deleted it after it was merged."
        }
        GitErrorKind::RepositoryLocked => {
            "Another program seems to be using this repository. Close other Git tools and try again. If none are open, Git may have crashed earlier: delete the .lock file named in the details."
        }
        GitErrorKind::PathTooLong => {
            "A file path is too long for Windows. Clone the repository to a shorter folder (for example C:\\src), or ask IT to enable long path support."
        }
        GitErrorKind::DiskFull => "Your disk is full. Free up some space and try again.",
        GitErrorKind::TlsRevocationCheck => {
            "Windows couldn't check whether the server's certificate has been revoked, usually because the company's certificate list can't be reached from here. Connect to the company network or VPN and try again, or ask your IT team."
        }
        GitErrorKind::LfsFailed => {
            "Git LFS couldn't transfer some large files. Check your connection and try again; the details say which files."
        }
        GitErrorKind::RepositoryNotFound => {
            "The server couldn't find that repository. Check the address, and that your account has access to it."
        }
        GitErrorKind::Unknown => "Git reported a problem.",
    }
}

//! Serializable error returned by every Tauri command.

use serde::Serialize;

use crate::git::error::{GitError, GitErrorKind};
use crate::repo_manager::RepoError;

/// Broad category the UI uses to pick a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum AppErrorKind {
    GitNotFound,
    GitUnsupported,
    GitTimedOut,
    GitCancelled,
    Git,
    UnknownRepository,
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

    fn new(kind: AppErrorKind, message: impl Into<String>, details: Option<String>) -> Self {
        Self {
            kind,
            git_kind: None,
            message: message.into(),
            details,
        }
    }
}

impl From<GitError> for AppError {
    fn from(err: GitError) -> Self {
        match err {
            GitError::NotFound { searched } => AppError::new(
                AppErrorKind::GitNotFound,
                "Anvil couldn't find Git on this computer. Install Git 2.40 or newer, then reopen Anvil.",
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
                    "Anvil needs Git {minimum} or newer. This computer has Git {found}. Update Git, then reopen Anvil."
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
            },
            GitError::Spawn(e) => {
                AppError::new(AppErrorKind::Internal, "Anvil couldn't start Git.", Some(e.to_string()))
            }
            GitError::Parse(s) => AppError::new(
                AppErrorKind::Internal,
                "Git returned something Anvil didn't understand.",
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
                "Anvil couldn't save your repository list.",
                Some(crate::redact::redact(&e.to_string())),
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
        GitErrorKind::Unknown => "Git reported a problem.",
    }
}

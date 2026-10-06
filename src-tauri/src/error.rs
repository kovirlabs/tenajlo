//! Serializable error returned by every Tauri command.

use serde::Serialize;

use crate::git::error::{GitError, GitErrorKind};

/// Broad category the UI uses to pick a dialog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum AppErrorKind {
    GitNotFound,
    GitUnsupported,
    GitTimedOut,
    GitCancelled,
    Git,
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
                message: "Git reported a problem.".into(),
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

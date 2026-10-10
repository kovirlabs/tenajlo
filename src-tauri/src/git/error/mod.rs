//! Git error types (spec §5.5). The stderr → [`GitErrorKind`] table is in [`patterns`]; the
//! plain-language message for each kind is in [`messages`].

mod messages;
mod patterns;

pub use patterns::classify;

use std::path::PathBuf;

use serde::Serialize;

use super::parse::version::GitVersion;

/// Typed classification of a failed git invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum GitErrorKind {
    AuthFailed,
    HostKeyUnknown,
    /// The server's SSH host key differs from `known_hosts`: possibly an attack.
    HostKeyChanged,
    SshKeyRejected,
    PushRejected,
    PullDiverged,
    LocalChangesBlock,
    TlsUntrusted,
    HostUnreachable,
    MergeConflict,
    NotARepository,
    /// Folder owned by another user; git's `safe.directory` check refused it.
    DubiousOwnership,
    /// `user.name` / `user.email` not configured.
    IdentityMissing,
    BranchExists,
    BranchNotMerged,
    /// The server has no such repository, or hides it from this account.
    RepositoryNotFound,
    /// Forgejo branch protection refused a direct push.
    ProtectedBranch,
    /// A server-side hook refused the push for another reason (its message is in details).
    PushDeclined,
    /// HTTP 413: the upload exceeded the server's size limit.
    PushTooLarge,
    /// The branch being pulled no longer exists on the server.
    RemoteBranchMissing,
    /// `.git/index.lock` (or another lock) exists: another git process, or a crashed one.
    RepositoryLocked,
    /// A path exceeds Windows' length limit.
    PathTooLong,
    DiskFull,
    /// Windows couldn't check the certificate's revocation status (internal CA, offline CRL).
    TlsRevocationCheck,
    /// Git LFS couldn't transfer large files.
    LfsFailed,
    Unknown,
}

/// Errors from locating, spawning, or running git.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    #[error("git executable not found")]
    NotFound { searched: Vec<PathBuf> },
    #[error("failed to start git: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("git timed out")]
    TimedOut,
    #[error("git was cancelled")]
    Cancelled,
    /// Git exited non-zero. `stderr` is already redacted.
    #[error("git exited with code {exit_code:?}: {kind:?}")]
    Failed {
        kind: GitErrorKind,
        exit_code: Option<i32>,
        stderr: String,
    },
    #[error("invalid branch name: {0}")]
    InvalidRefName(String),
    #[error("unrecognized git output: {0}")]
    Parse(String),
    #[error("git {found} is older than the minimum {minimum}")]
    Unsupported {
        found: GitVersion,
        minimum: GitVersion,
    },
}

impl GitError {
    /// A failed invocation: redacts `stderr` and classifies it. Some failures are only
    /// explained on stdout (merge: "CONFLICT (content): …"), so that's the fallback.
    pub fn failed(stderr: &str, stdout: &[u8], exit_code: Option<i32>) -> Self {
        let stderr = crate::redact::redact(stderr);
        let kind = match classify(&stderr) {
            GitErrorKind::Unknown => classify(&String::from_utf8_lossy(stdout)),
            kind => kind,
        };
        GitError::Failed {
            kind,
            exit_code,
            stderr,
        }
    }
}

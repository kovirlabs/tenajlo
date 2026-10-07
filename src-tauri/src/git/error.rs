//! Git error types and the stderr → [`GitErrorKind`] pattern table (spec §5.5).

use std::path::PathBuf;

use serde::Serialize;

use super::parse::version::GitVersion;

/// Typed classification of a failed git invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum GitErrorKind {
    AuthFailed,
    HostKeyUnknown,
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
    Unknown,
}

/// Ordered pattern table. First match wins, so more specific patterns go first.
const PATTERNS: &[(&str, GitErrorKind)] = &[
    ("Authentication failed", GitErrorKind::AuthFailed),
    (
        "The requested URL returned error: 401",
        GitErrorKind::AuthFailed,
    ),
    (
        "The requested URL returned error: 403",
        GitErrorKind::AuthFailed,
    ),
    ("could not read Username", GitErrorKind::AuthFailed),
    // Forgejo: "remote: Repository not found"; git: "fatal: repository '<url>' not found";
    // local paths: "fatal: '<path>' does not appear to be a git repository".
    ("Repository not found", GitErrorKind::RepositoryNotFound),
    ("' not found", GitErrorKind::RepositoryNotFound),
    (
        "does not appear to be a git repository",
        GitErrorKind::RepositoryNotFound,
    ),
    ("Host key verification failed", GitErrorKind::HostKeyUnknown),
    ("Permission denied (publickey", GitErrorKind::SshKeyRejected),
    ("Not possible to fast-forward", GitErrorKind::PullDiverged),
    ("non-fast-forward", GitErrorKind::PushRejected),
    ("[rejected]", GitErrorKind::PushRejected),
    (
        "would be overwritten by checkout",
        GitErrorKind::LocalChangesBlock,
    ),
    (
        "would be overwritten by merge",
        GitErrorKind::LocalChangesBlock,
    ),
    ("SSL certificate problem", GitErrorKind::TlsUntrusted),
    ("schannel: SEC_E_UNTRUSTED_ROOT", GitErrorKind::TlsUntrusted),
    (
        "server certificate verification failed",
        GitErrorKind::TlsUntrusted,
    ),
    ("Could not resolve host", GitErrorKind::HostUnreachable),
    ("Could not resolve hostname", GitErrorKind::HostUnreachable),
    ("CONFLICT", GitErrorKind::MergeConflict),
    ("detected dubious ownership", GitErrorKind::DubiousOwnership),
    ("is not fully merged", GitErrorKind::BranchNotMerged),
    ("a branch named", GitErrorKind::BranchExists),
    ("Please tell me who you are", GitErrorKind::IdentityMissing),
    ("Author identity unknown", GitErrorKind::IdentityMissing),
    ("empty ident name", GitErrorKind::IdentityMissing),
    ("no email was given", GitErrorKind::IdentityMissing),
    ("not a git repository", GitErrorKind::NotARepository),
    ("must be run in a work tree", GitErrorKind::NotARepository),
];

/// Classifies git stderr into a [`GitErrorKind`].
pub fn classify(stderr: &str) -> GitErrorKind {
    PATTERNS
        .iter()
        .find(|(needle, _)| stderr.contains(needle))
        .map_or(GitErrorKind::Unknown, |(_, kind)| *kind)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_known_patterns() {
        let cases = [
            (
                "remote: Invalid username or password.\nfatal: Authentication failed for 'https://***@h/r.git/'",
                GitErrorKind::AuthFailed,
            ),
            (
                "fatal: unable to access 'https://h/r/': The requested URL returned error: 403",
                GitErrorKind::AuthFailed,
            ),
            (
                "Host key verification failed.\nfatal: Could not read from remote repository.",
                GitErrorKind::HostKeyUnknown,
            ),
            ("git@h: Permission denied (publickey).", GitErrorKind::SshKeyRejected),
            (
                "remote: Repository not found.\nfatal: repository 'https://h/team/x.git/' not found",
                GitErrorKind::RepositoryNotFound,
            ),
            (
                "fatal: '/tmp/nope.git' does not appear to be a git repository",
                GitErrorKind::RepositoryNotFound,
            ),
            (
                " ! [rejected]        main -> main (non-fast-forward)",
                GitErrorKind::PushRejected,
            ),
            (
                "fatal: Not possible to fast-forward, aborting.",
                GitErrorKind::PullDiverged,
            ),
            (
                "error: Your local changes to the following files would be overwritten by checkout:",
                GitErrorKind::LocalChangesBlock,
            ),
            (
                "fatal: unable to access 'https://h/': SSL certificate problem: unable to get local issuer certificate",
                GitErrorKind::TlsUntrusted,
            ),
            (
                "fatal: unable to access 'https://h/': schannel: SEC_E_UNTRUSTED_ROOT (0x80090325)",
                GitErrorKind::TlsUntrusted,
            ),
            (
                "fatal: unable to access 'https://h/': Could not resolve host: TMC-GIT01.tmus.local",
                GitErrorKind::HostUnreachable,
            ),
            (
                "CONFLICT (content): Merge conflict in a.txt",
                GitErrorKind::MergeConflict,
            ),
            (
                "fatal: not a git repository (or any of the parent directories): .git",
                GitErrorKind::NotARepository,
            ),
            ("fatal: detected dubious ownership in repository at 'C:/Shared/repo'", GitErrorKind::DubiousOwnership),
            ("error: the branch 'x' is not fully merged", GitErrorKind::BranchNotMerged),
            ("fatal: a branch named 'x' already exists", GitErrorKind::BranchExists),
            ("fatal: destination path 'r' already exists and is not an empty directory.", GitErrorKind::Unknown),
            ("", GitErrorKind::Unknown),
            ("something else entirely", GitErrorKind::Unknown),
        ];
        for (stderr, expected) in cases {
            assert_eq!(classify(stderr), expected, "stderr: {stderr}");
        }
    }
}

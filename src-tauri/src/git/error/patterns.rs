//! The ordered stderr → [`GitErrorKind`] pattern table.

use super::GitErrorKind;

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
    (
        "batch response: Authentication required",
        GitErrorKind::AuthFailed,
    ),
    // Forgejo: "remote: Forgejo: Not allowed to push to protected branch main".
    ("push to protected branch", GitErrorKind::ProtectedBranch),
    ("pre-receive hook declined", GitErrorKind::PushDeclined),
    ("HTTP 413", GitErrorKind::PushTooLarge),
    ("413 Request Entity Too Large", GitErrorKind::PushTooLarge),
    (
        "couldn't find remote ref",
        GitErrorKind::RemoteBranchMissing,
    ),
    (".lock': File exists", GitErrorKind::RepositoryLocked),
    ("Filename too long", GitErrorKind::PathTooLong),
    ("No space left on device", GitErrorKind::DiskFull),
    (
        "There is not enough space on the disk",
        GitErrorKind::DiskFull,
    ),
    // schannel 0x80092012 / 0x80092013: CRL or OCSP unreachable (before the TLS patterns).
    (
        "unable to check revocation",
        GitErrorKind::TlsRevocationCheck,
    ),
    (
        "revocation server was offline",
        GitErrorKind::TlsRevocationCheck,
    ),
    ("batch response:", GitErrorKind::LfsFailed),
    ("Smudge error", GitErrorKind::LfsFailed),
    ("smudge filter lfs failed", GitErrorKind::LfsFailed),
    // Forgejo: "remote: Repository not found"; git: "fatal: repository '<url>' not found";
    // local paths: "fatal: '<path>' does not appear to be a git repository".
    ("Repository not found", GitErrorKind::RepositoryNotFound),
    ("' not found", GitErrorKind::RepositoryNotFound),
    (
        "does not appear to be a git repository",
        GitErrorKind::RepositoryNotFound,
    ),
    // OpenSSH prints this banner before "Host key verification failed", so it goes first.
    (
        "REMOTE HOST IDENTIFICATION HAS CHANGED",
        GitErrorKind::HostKeyChanged,
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
    // curl: "Failed to connect to h port 443 after 0 ms: Couldn't connect to server";
    // OpenSSH: "ssh: connect to host h port 2222: Connection refused".
    ("Couldn't connect to server", GitErrorKind::HostUnreachable),
    ("Failed to connect to", GitErrorKind::HostUnreachable),
    ("Connection refused", GitErrorKind::HostUnreachable),
    ("Connection timed out", GitErrorKind::HostUnreachable),
    ("Operation timed out", GitErrorKind::HostUnreachable),
    ("Network is unreachable", GitErrorKind::HostUnreachable),
    ("No route to host", GitErrorKind::HostUnreachable),
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
            // Captured from Forgejo 11 with branch protection on main.
            (
                "remote: \nremote: Forgejo: Not allowed to push to protected branch main        \nTo http://localhost:3000/t/p.git\n ! [remote rejected] main -> main (pre-receive hook declined)\nerror: failed to push some refs to 'http://localhost:3000/t/p.git'",
                GitErrorKind::ProtectedBranch,
            ),
            (
                " ! [remote rejected] main -> main (pre-receive hook declined)",
                GitErrorKind::PushDeclined,
            ),
            (
                "fatal: unable to access 'http://localhost:3999/x.git/': Failed to connect to localhost port 3999 after 0 ms: Couldn't connect to server",
                GitErrorKind::HostUnreachable,
            ),
            (
                "ssh: connect to host localhost port 2999: Connection refused\nfatal: Could not read from remote repository.",
                GitErrorKind::HostUnreachable,
            ),
            (
                "fatal: unable to access 'https://h/x.git/': schannel: next InitializeSecurityContext failed: Unknown error (0x80092012) - The revocation function was unable to check revocation for the certificate.",
                GitErrorKind::TlsRevocationCheck,
            ),
            (
                "error: RPC failed; HTTP 413 curl 22 The requested URL returned error: 413",
                GitErrorKind::PushTooLarge,
            ),
            (
                "fatal: Unable to create 'C:/src/plc/.git/index.lock': File exists.",
                GitErrorKind::RepositoryLocked,
            ),
            (
                "error: unable to create file very/long/path.txt: Filename too long",
                GitErrorKind::PathTooLong,
            ),
            ("fatal: couldn't find remote ref feature/x", GitErrorKind::RemoteBranchMissing),
            (
                "Error downloading object: big.bin (abc): Smudge error: Error downloading big.bin",
                GitErrorKind::LfsFailed,
            ),
            (
                "@@@@@\n@    WARNING: REMOTE HOST IDENTIFICATION HAS CHANGED!     @\n@@@@@\nHost key verification failed.",
                GitErrorKind::HostKeyChanged,
            ),
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
                "fatal: unable to access 'https://h/': Could not resolve host: GIT.EXAMPLE.COM",
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

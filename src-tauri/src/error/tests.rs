use std::path::PathBuf;

use uuid::Uuid;

use super::*;
use crate::git::parse::version::GitVersion;
use crate::store::accounts::AccountKind;

fn failed(kind: GitErrorKind) -> GitError {
    GitError::Failed {
        kind,
        exit_code: Some(128),
        stderr: "fatal: something".to_owned(),
    }
}

fn account() -> AccountEntry {
    AccountEntry {
        id: Uuid::nil(),
        kind: AccountKind::Forgejo,
        base_url: "https://git.example.com".to_owned(),
        login: "alice".to_owned(),
        display_name: "Alice".to_owned(),
        avatar_url: None,
        token_scopes: None,
        ssh_host: None,
        needs_sign_in: true,
    }
}

#[test]
fn maps_errors_to_kind_and_plain_message() {
    let cases: Vec<(AppError, AppErrorKind, Option<GitErrorKind>, &str)> = vec![
        (
            GitError::Cancelled.into(),
            AppErrorKind::GitCancelled,
            None,
            "The operation was cancelled.",
        ),
        (
            GitError::TimedOut.into(),
            AppErrorKind::GitTimedOut,
            None,
            "Git took too long to respond and was stopped.",
        ),
        (
            GitError::NotFound { searched: vec![] }.into(),
            AppErrorKind::GitNotFound,
            None,
            "Tenajlo couldn't find Git on this computer. Install Git 2.40 or newer, then reopen Tenajlo.",
        ),
        (
            GitError::Unsupported {
                found: GitVersion { major: 2, minor: 30, patch: 1 },
                minimum: GitVersion::MINIMUM,
            }
            .into(),
            AppErrorKind::GitUnsupported,
            None,
            "Tenajlo needs Git 2.40.0 or newer. This computer has Git 2.30.1. Update Git, then reopen Tenajlo.",
        ),
        (
            failed(GitErrorKind::AuthFailed).into(),
            AppErrorKind::Git,
            Some(GitErrorKind::AuthFailed),
            "The server didn't accept your sign-in. Check your account or token.",
        ),
        (
            failed(GitErrorKind::MergeConflict).into(),
            AppErrorKind::Git,
            Some(GitErrorKind::MergeConflict),
            "Some files were changed both on the server and by you. Resolve the conflicts, then commit to finish.",
        ),
        (
            failed(GitErrorKind::TlsUntrusted).into(),
            AppErrorKind::Git,
            Some(GitErrorKind::TlsUntrusted),
            "This computer doesn't trust the server's certificate. Your IT team may need to install the company certificate.",
        ),
        (
            failed(GitErrorKind::HostUnreachable).into(),
            AppErrorKind::Git,
            Some(GitErrorKind::HostUnreachable),
            "Couldn't reach the server. Check your network or VPN connection.",
        ),
        (
            GitError::InvalidRefName("bad..name".to_owned()).into(),
            AppErrorKind::InvalidInput,
            None,
            "“bad..name” can't be used as a branch name.",
        ),
        (
            RepoError::UnknownRepository.into(),
            AppErrorKind::UnknownRepository,
            None,
            "That repository is no longer in your list.",
        ),
        (
            RepoError::OutsideRepository.into(),
            AppErrorKind::InvalidInput,
            None,
            "That file isn't in this repository.",
        ),
        (
            RepoError::Git(GitError::Cancelled).into(),
            AppErrorKind::GitCancelled,
            None,
            "The operation was cancelled.",
        ),
        (
            ForgejoError::TlsUntrusted("unknown issuer".to_owned()).into(),
            AppErrorKind::Server,
            None,
            "This computer doesn't trust the server's certificate. Your IT team may need to install the company certificate.",
        ),
        (
            ForgejoError::Unreachable("dns".to_owned()).into(),
            AppErrorKind::Server,
            None,
            "Couldn't reach the server. Check the address and your network or VPN connection.",
        ),
        (
            ForgejoError::TimedOut.into(),
            AppErrorKind::Server,
            None,
            "The server took too long to respond. Try again in a moment.",
        ),
        (
            ForgejoError::Unauthorized.into(),
            AppErrorKind::Server,
            None,
            "The server didn't accept that token. Check that you copied all of it and that it hasn't expired or been deleted.",
        ),
        (
            ForgejoError::Http {
                status: 500,
                body: String::new(),
            }
            .into(),
            AppErrorKind::Server,
            None,
            "The server reported a problem (error 500). Try again later.",
        ),
        (
            ForgejoError::InvalidUrl("no host".to_owned()).into(),
            AppErrorKind::InvalidInput,
            None,
            "That server address can't be used: no host.",
        ),
        (
            AccountError::UnknownAccount.into(),
            AppErrorKind::UnknownAccount,
            None,
            "That account is no longer signed in.",
        ),
        (
            CommitBlocker::Conflicts.into(),
            AppErrorKind::InvalidInput,
            None,
            "Resolve the conflicted files before committing.",
        ),
        (
            CommitBlocker::LfsMissing.into(),
            AppErrorKind::InvalidInput,
            None,
            "This repository stores large files with Git LFS, which isn't installed on this computer. Install Git LFS (git-lfs.com), then restart Tenajlo.",
        ),
        (
            PlanError::NotPublished.into(),
            AppErrorKind::InvalidInput,
            None,
            "This branch isn't on the server yet. Publish it first.",
        ),
        (
            CloneError::DestinationExists(PathBuf::from("proj")).into(),
            AppErrorKind::InvalidInput,
            None,
            "A folder already exists at proj. Choose another name or location.",
        ),
        (
            SavedSecretError::Unknown.into(),
            AppErrorKind::InvalidInput,
            None,
            "That password was already forgotten.",
        ),
        (
            UpdateError::NotConfigured.into(),
            AppErrorKind::InvalidInput,
            None,
            "This copy of Tenajlo can't update itself. Download new versions from the release page.",
        ),
    ];
    for (i, (err, kind, git_kind, message)) in cases.into_iter().enumerate() {
        assert_eq!(err.kind, kind, "case {i}: {message}");
        assert_eq!(err.git_kind, git_kind, "case {i}: {message}");
        assert_eq!(err.message, message, "case {i}");
        assert_eq!(err.account_id, None, "case {i}: {message}");
    }
}

#[test]
fn git_failure_keeps_stderr_as_details() {
    let err: AppError = failed(GitErrorKind::AuthFailed).into();
    assert_eq!(err.details.as_deref(), Some("fatal: something"));
}

#[test]
fn sign_in_required_names_the_account() {
    let err: AppError = AccountError::SignInRequired(Box::new(account())).into();
    assert_eq!(err.kind, AppErrorKind::SignInRequired);
    assert_eq!(err.account_id, Some(Uuid::nil().to_string()));
    assert!(
        err.message.contains("https://git.example.com"),
        "{}",
        err.message
    );
}

#[test]
fn account_lacks_access_keeps_the_git_error() {
    let git: AppError = failed(GitErrorKind::AuthFailed).into();
    let err = AppError::account_lacks_access(&account(), git);
    assert_eq!(err.kind, AppErrorKind::Git);
    assert_eq!(err.git_kind, Some(GitErrorKind::AuthFailed));
    assert_eq!(err.account_id, Some(Uuid::nil().to_string()));
    assert!(err.message.contains("as alice"), "{}", err.message);
}

#[test]
fn server_error_details_are_redacted() {
    let err: AppError =
        ForgejoError::Unreachable("https://alice:s3cret@git.example.com/api".to_owned()).into();
    let details = err.details.unwrap_or_default();
    assert!(!details.contains("s3cret"), "{details}");
    assert!(details.contains("git.example.com"), "{details}");
}

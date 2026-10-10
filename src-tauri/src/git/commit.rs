//! Creating commits (spec §5.3). Callers hold the repository's mutation lock.

use std::path::Path;
use std::time::Duration;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::merge::{self, OperationKind};
use super::parse::status::{StagedState, WorkingDirectoryStatus};

/// Commit hooks (pre-commit, commit-msg) can be slow; allow them time.
const COMMIT_TIMEOUT: Duration = Duration::from_secs(10 * 60);

/// Builds the commit message from the UI's summary and optional description.
pub fn build_message(summary: &str, description: &str) -> String {
    let summary = summary.trim();
    let description = description.trim();
    if description.is_empty() {
        format!("{summary}\n")
    } else {
        format!("{summary}\n\n{description}\n")
    }
}

/// Why the repository can't be committed right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitBlocker {
    Conflicts,
    /// The repository uses Git LFS but git-lfs isn't installed: LFS files would be committed
    /// as plain blobs.
    LfsMissing,
    /// A rebase, cherry-pick or revert is in progress.
    OperationInProgress,
    NothingStaged,
}

/// Checks that `current` (fresh status) can be committed. `None` means ready.
pub async fn check_ready(
    git: &GitBinary,
    root: &Path,
    current: &WorkingDirectoryStatus,
) -> Result<Option<CommitBlocker>, GitError> {
    if current.has_conflicts {
        return Ok(Some(CommitBlocker::Conflicts));
    }
    if super::lfs::status(git, root).missing() {
        return Ok(Some(CommitBlocker::LfsMissing));
    }
    let operation = merge::operation_state(git, root, current).await?.operation;
    Ok(blocker_for(current, operation))
}

fn blocker_for(
    current: &WorkingDirectoryStatus,
    operation: Option<OperationKind>,
) -> Option<CommitBlocker> {
    match operation {
        // A merge can be committed with nothing staged (e.g. every conflict resolved as "ours").
        Some(OperationKind::Merge) => None,
        Some(_) => Some(CommitBlocker::OperationInProgress),
        None if current.files.iter().all(|f| f.staged == StagedState::None) => {
            Some(CommitBlocker::NothingStaged)
        }
        None => None,
    }
}

/// Commits the index with `message` (sent on stdin). Returns the new commit's SHA.
pub async fn commit(git: &GitBinary, root: &Path, message: &str) -> Result<String, GitError> {
    // `whitespace` keeps lines starting with '#' (e.g. "#123 fixed"), unlike the default `strip`.
    GitCommand::new(
        ["commit", "-F", "-", "--cleanup=whitespace"],
        Access::Mutating,
    )
    .stdin(message)
    .timeout(COMMIT_TIMEOUT)
    .cwd(root)
    .run(git)
    .await?;
    let out = GitCommand::new(["rev-parse", "HEAD"], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::error::GitErrorKind;
    use crate::git::test_support::{git, git_in, init_repo, write};

    #[test]
    fn builds_messages() {
        assert_eq!(build_message("  Fix pump  ", ""), "Fix pump\n");
        assert_eq!(
            build_message("Fix", " #12 details\n\n"),
            "Fix\n\n#12 details\n"
        );
    }

    #[test]
    fn merges_commit_with_nothing_staged() {
        let clean = WorkingDirectoryStatus::default();
        assert_eq!(blocker_for(&clean, Some(OperationKind::Merge)), None);
        assert_eq!(
            blocker_for(&clean, Some(OperationKind::Rebase)),
            Some(CommitBlocker::OperationInProgress)
        );
        assert_eq!(
            blocker_for(&clean, None),
            Some(CommitBlocker::NothingStaged)
        );
    }

    #[tokio::test]
    async fn conflicts_block_before_anything_else() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        let current = WorkingDirectoryStatus {
            has_conflicts: true,
            ..Default::default()
        };
        assert_eq!(
            check_ready(&git, &repo, &current).await.unwrap(),
            Some(CommitBlocker::Conflicts)
        );
    }

    #[tokio::test]
    async fn commits_index_with_unicode_and_hash_lines() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        git_in(&repo, &["config", "user.name", "Tëst"]).await;
        git_in(&repo, &["config", "user.email", "t@example.com"]).await;
        git_in(&repo, &["config", "commit.gpgsign", "false"]).await;
        write(&repo, "a.txt", "1\n");
        git_in(&repo, &["add", "a.txt"]).await;

        let sha = commit(
            &git,
            &repo,
            &build_message("Ünïcode summary", "#42 keep me"),
        )
        .await
        .unwrap();
        assert_eq!(sha.len(), 40);
        let msg = git_in(&repo, &["log", "-1", "--format=%B"]).await;
        assert_eq!(msg.trim_end(), "Ünïcode summary\n\n#42 keep me");
    }

    #[tokio::test]
    async fn missing_identity_is_classified() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        // Empty name in local config overrides any global identity on the test machine.
        git_in(&repo, &["config", "user.name", ""]).await;
        git_in(&repo, &["config", "user.email", ""]).await;
        git_in(&repo, &["config", "user.useConfigOnly", "true"]).await;
        write(&repo, "a.txt", "1\n");
        git_in(&repo, &["add", "a.txt"]).await;
        match commit(&git, &repo, "x\n").await {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::IdentityMissing),
            other => panic!("unexpected {other:?}"),
        }
    }
}

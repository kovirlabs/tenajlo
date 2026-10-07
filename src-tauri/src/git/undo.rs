//! Undoing the last commit (GitHub Desktop's "Undo"). Callers hold the mutation lock.

use std::path::Path;

use serde::Serialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

/// The message of an undone commit, so the UI can put it back in the commit box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct UndoneCommit {
    pub summary: String,
    pub description: String,
}

/// Why the last commit can't be undone.
#[derive(Debug, thiserror::Error)]
pub enum UndoError {
    #[error("HEAD is no longer the expected commit")]
    HeadMoved,
    #[error("merge commits can't be undone")]
    MergeCommit,
    #[error("the commit is already on the upstream branch")]
    AlreadyPushed,
    #[error(transparent)]
    Git(#[from] GitError),
}

async fn text(
    git: &GitBinary,
    root: &Path,
    args: &[&str],
    ok: &'static [i32],
) -> Result<(Option<i32>, String), GitError> {
    let out = GitCommand::new(args, Access::ReadOnly)
        .ok_exit_codes(ok)
        .cwd(root)
        .run(git)
        .await?;
    Ok((
        out.exit_code,
        String::from_utf8_lossy(&out.stdout).into_owned(),
    ))
}

/// Undoes `expected_sha` if it is still HEAD: keeps its changes staged and returns its message.
pub async fn undo_last_commit(
    git: &GitBinary,
    root: &Path,
    expected_sha: &str,
) -> Result<UndoneCommit, UndoError> {
    let (_, parents) = text(
        git,
        root,
        &["rev-list", "--parents", "-n", "1", "HEAD", "--"],
        &[0],
    )
    .await?;
    let mut ids = parents.split_whitespace();
    if ids.next() != Some(expected_sha) {
        return Err(UndoError::HeadMoved);
    }
    let parent_count = ids.count();
    if parent_count > 1 {
        return Err(UndoError::MergeCommit);
    }

    // Refuse if the commit already reached the upstream branch (exit 128: no upstream).
    let (code, _) = text(
        git,
        root,
        &["rev-parse", "--verify", "--quiet", "@{upstream}"],
        &[0, 1, 128],
    )
    .await?;
    if code == Some(0) {
        let (pushed, _) = text(
            git,
            root,
            &["merge-base", "--is-ancestor", "HEAD", "@{upstream}"],
            &[0, 1],
        )
        .await?;
        if pushed == Some(0) {
            return Err(UndoError::AlreadyPushed);
        }
    }

    let (_, message) = text(git, root, &["log", "-1", "--format=%B", "HEAD", "--"], &[0]).await?;
    let message = message.trim();
    let (summary, description) = message.split_once('\n').unwrap_or((message, ""));
    let undone = UndoneCommit {
        summary: summary.trim().to_owned(),
        description: description.trim().to_owned(),
    };

    let args: &[&str] = if parent_count == 0 {
        // First commit: make the branch unborn again; the index keeps the files staged.
        &["update-ref", "-d", "HEAD"]
    } else {
        &["reset", "--soft", "HEAD^"]
    };
    GitCommand::new(args, Access::Mutating)
        .cwd(root)
        .run(git)
        .await?;
    Ok(undone)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::parse::status::StagedState;
    use crate::git::status::status;
    use crate::git::test_support::{git_in, init_repo, write};

    async fn head(repo: &Path) -> String {
        git_in(repo, &["rev-parse", "HEAD"]).await.trim().to_owned()
    }

    #[tokio::test]
    async fn undoes_root_and_normal_commits() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        write(&repo, "a.txt", "1\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "First", "-m", "Body ü"]).await;
        let first = head(&repo).await;

        write(&repo, "a.txt", "2\n");
        git_in(&repo, &["commit", "-q", "-am", "Second"]).await;
        let second = head(&repo).await;

        assert!(matches!(
            undo_last_commit(&git, &repo, &first).await,
            Err(UndoError::HeadMoved)
        ));
        let undone = undo_last_commit(&git, &repo, &second).await.unwrap();
        assert_eq!(
            undone,
            UndoneCommit {
                summary: "Second".into(),
                description: String::new()
            }
        );
        assert_eq!(head(&repo).await, first);
        let s = status(&git, &repo).await.unwrap();
        assert_eq!(s.files[0].staged, StagedState::Full, "changes stay staged");

        git_in(&repo, &["reset", "-q", "--hard"]).await;
        let undone = undo_last_commit(&git, &repo, &first).await.unwrap();
        assert_eq!(undone.description, "Body ü");
        let s = status(&git, &repo).await.unwrap();
        assert_eq!(s.branch.tip, None, "branch is unborn again");
        assert_eq!(s.files[0].staged, StagedState::Full);
    }

    #[tokio::test]
    async fn refuses_pushed_and_merge_commits() {
        let (tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        let origin = tmp.path().join("o.git");
        git_in(
            tmp.path(),
            &["init", "-q", "--bare", origin.to_str().unwrap()],
        )
        .await;
        git_in(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        )
        .await;
        git_in(&repo, &["push", "-q", "-u", "origin", "main"]).await;
        let pushed = head(&repo).await;
        assert!(matches!(
            undo_last_commit(&git, &repo, &pushed).await,
            Err(UndoError::AlreadyPushed)
        ));

        git_in(&repo, &["switch", "-q", "-c", "side"]).await;
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "side"]).await;
        git_in(&repo, &["switch", "-q", "main"]).await;
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "main"]).await;
        git_in(&repo, &["merge", "-q", "--no-ff", "-m", "merge", "side"]).await;
        let merge = head(&repo).await;
        assert!(matches!(
            undo_last_commit(&git, &repo, &merge).await,
            Err(UndoError::MergeCommit)
        ));
    }
}

//! Branch listing (spec §5.3).

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::branches::{parse_branches, BranchList, BRANCH_FORMAT};

/// Lists local and remote branches.
pub async fn branches(git: &GitBinary, root: &Path) -> Result<BranchList, GitError> {
    let remotes = super::remote::list_remotes(git, root).await?;
    let out = GitCommand::new(
        ["for-each-ref", BRANCH_FORMAT, "refs/heads", "refs/remotes"],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(parse_branches(&out.stdout, &remotes))
}

/// The name local `branch` has on the server: its upstream's branch on the upstream's remote
/// (`fix` tracking `origin/evan/fix` is `evan/fix` there). `None` without an upstream. The
/// name must already be validated (it is passed as a ref, after `refs/heads/`).
pub async fn server_branch(
    git: &GitBinary,
    root: &Path,
    branch: &str,
) -> Result<Option<String>, GitError> {
    let refname = format!("refs/heads/{branch}");
    let out = GitCommand::new(
        [
            "for-each-ref",
            "--format=%(upstream:remotename)%00%(upstream)",
            refname.as_str(),
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    let line = String::from_utf8_lossy(&out.stdout);
    let Some((remote, upstream)) = line.trim_end().split_once('\0') else {
        return Ok(None);
    };
    Ok(upstream
        .strip_prefix(&format!("refs/remotes/{remote}/"))
        .filter(|b| !remote.is_empty() && !b.is_empty())
        .map(str::to_owned))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{git, git_in, init_repo, with_bare_origin};

    #[tokio::test]
    async fn server_branch_follows_the_upstream_name() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        with_bare_origin(&repo).await;
        git_in(&repo, &["push", "-q", "-u", "origin", "main"]).await;
        git_in(&repo, &["push", "-q", "origin", "main:evan/fix-ü"]).await;
        git_in(&repo, &["fetch", "-q", "origin"]).await;
        git_in(
            &repo,
            &["branch", "-q", "--track", "fix", "origin/evan/fix-ü"],
        )
        .await;
        git_in(&repo, &["branch", "-q", "local-only"]).await;

        let name = |b: &'static str| server_branch(&git, &repo, b);
        assert_eq!(name("fix").await.unwrap().as_deref(), Some("evan/fix-ü"));
        assert_eq!(name("main").await.unwrap().as_deref(), Some("main"));
        assert_eq!(name("local-only").await.unwrap(), None);
        assert_eq!(name("no-such-branch").await.unwrap(), None);
    }

    #[tokio::test]
    async fn lists_branches_with_bare_origin() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        assert_eq!(
            branches(&git, &repo).await.unwrap(),
            BranchList::default(),
            "unborn repo has no refs"
        );

        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        git_in(&repo, &["branch", "topic"]).await;
        with_bare_origin(&repo).await;
        git_in(&repo, &["push", "-q", "-u", "origin", "main"]).await;
        git_in(&repo, &["push", "-q", "origin", "topic:remote-only"]).await;
        git_in(&repo, &["fetch", "-q", "origin"]).await;

        let list = branches(&git, &repo).await.unwrap();
        assert_eq!(list.current.as_deref(), Some("main"));
        assert_eq!(
            list.local
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            vec!["main", "topic"]
        );
        assert_eq!(list.local[0].upstream.as_deref(), Some("origin/main"));
        assert_eq!(
            list.remote_only
                .iter()
                .map(|b| b.name.as_str())
                .collect::<Vec<_>>(),
            vec!["origin/remote-only"]
        );
    }
}

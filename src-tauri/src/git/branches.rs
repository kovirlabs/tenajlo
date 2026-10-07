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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::test_support::{git_in, init_repo};

    #[tokio::test]
    async fn lists_branches_with_bare_origin() {
        let (tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        assert_eq!(
            branches(&git, &repo).await.unwrap(),
            BranchList::default(),
            "unborn repo has no refs"
        );

        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        git_in(&repo, &["branch", "topic"]).await;
        let origin = tmp.path().join("origin.git");
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

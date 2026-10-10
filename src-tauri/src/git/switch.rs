//! Creating, switching and deleting branches (spec §5.3). Callers hold the mutation lock.

use std::path::Path;

use serde::Deserialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::branches::BranchList;
use super::stash;

/// What to do with uncommitted changes when switching (GitHub Desktop's two choices).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub enum LocalChanges {
    /// Carry them to the target branch (fails if they would be overwritten).
    Bring,
    /// Save them on the current branch and switch with a clean working directory.
    Leave,
}

/// Creates `name` from HEAD and switches to it. Local changes come along. `name` must be validated.
pub async fn create(git: &GitBinary, root: &Path, name: &str) -> Result<(), GitError> {
    GitCommand::new(["switch", "-c", name], Access::Mutating)
        .cwd(root)
        .run(git)
        .await?;
    Ok(())
}

/// How to reach a target branch, resolved from the current branch list.
#[derive(Debug, PartialEq, Eq)]
pub enum Target {
    Local(String),
    /// Create local `local` tracking remote branch `remote_branch` (e.g. `origin/foo`).
    Track {
        local: String,
        remote_branch: String,
    },
}

/// Resolves a name from the branch dropdown. `None` if it's not a known branch.
pub fn resolve_target(branches: &BranchList, name: &str) -> Option<Target> {
    if branches.local.iter().any(|b| b.name == name) {
        return Some(Target::Local(name.to_owned()));
    }
    let remote = branches.remote_only.iter().find(|b| b.name == name)?;
    let prefix = format!("{}/", remote.remote.as_deref()?);
    let local = remote.name.strip_prefix(&prefix)?.to_owned();
    Some(Target::Track {
        local,
        remote_branch: remote.name.clone(),
    })
}

/// Switches to `target`. With [`LocalChanges::Leave`], changes are saved on `current` first and
/// re-applied if the switch fails.
pub async fn switch(
    git: &GitBinary,
    root: &Path,
    target: &Target,
    current: Option<&str>,
    has_changes: bool,
    local_changes: LocalChanges,
) -> Result<(), GitError> {
    let stashed = match (has_changes, local_changes, current) {
        (true, LocalChanges::Leave, Some(branch)) => {
            stash::save(git, root, branch).await?;
            Some(branch)
        }
        _ => None,
    };

    let args: Vec<&str> = match target {
        // --no-guess: never silently create a tracking branch from a same-named remote branch.
        Target::Local(name) => vec!["switch", "--no-guess", name],
        Target::Track {
            local,
            remote_branch,
        } => vec!["switch", "-c", local, "--track", remote_branch],
    };
    let result = GitCommand::new(args, Access::Mutating)
        .cwd(root)
        .run(git)
        .await;

    if let (Err(_), Some(branch)) = (&result, stashed) {
        // Put the user's changes back where they were. If that fails too, report the switch
        // error, which explains what happened; the changes stay saved for this branch and
        // the saved-changes banner offers to restore them.
        if let Err(e) = restore_saved(git, root, branch).await {
            tracing::warn!(error = %e, "could not restore changes after a failed switch");
        }
    }
    result.map(|_| ())
}

async fn restore_saved(git: &GitBinary, root: &Path, branch: &str) -> Result<(), GitError> {
    if let Some(saved) = stash::find(git, root, branch).await? {
        stash::restore(git, root, &saved).await?;
    }
    Ok(())
}

/// Deletes local branch `name`. Without `force`, git refuses if it has unmerged commits
/// (`BranchNotMerged`).
pub async fn delete(git: &GitBinary, root: &Path, name: &str, force: bool) -> Result<(), GitError> {
    let flag = if force { "-D" } else { "-d" };
    GitCommand::new(["branch", flag, "--", name], Access::Mutating)
        .cwd(root)
        .run(git)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::branches::branches;
    use crate::git::error::GitErrorKind;
    use crate::git::status::status;
    use crate::git::test_support::{commit_all, git, git_in, init_repo, with_bare_origin, write};

    async fn head(repo: &Path) -> String {
        git_in(repo, &["branch", "--show-current"])
            .await
            .trim()
            .to_owned()
    }

    #[tokio::test]
    async fn create_switch_leave_and_bring() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        write(&repo, "a.txt", "1\n");
        commit_all(&repo, "init").await;

        create(&git, &repo, "feature/ü").await.unwrap();
        assert_eq!(head(&repo).await, "feature/ü");

        // Leave: changes stay with feature/ü, main is clean.
        write(&repo, "a.txt", "work in progress\n");
        let main = Target::Local("main".into());
        switch(
            &git,
            &repo,
            &main,
            Some("feature/ü"),
            true,
            LocalChanges::Leave,
        )
        .await
        .unwrap();
        assert_eq!(head(&repo).await, "main");
        assert!(status(&git, &repo).await.unwrap().files.is_empty());
        assert!(stash::find(&git, &repo, "feature/ü")
            .await
            .unwrap()
            .is_some());

        // Bring: changes travel along.
        write(&repo, "b.txt", "carry\n");
        let feature = Target::Local("feature/ü".into());
        switch(
            &git,
            &repo,
            &feature,
            Some("main"),
            true,
            LocalChanges::Bring,
        )
        .await
        .unwrap();
        assert!(repo.join("b.txt").exists());
    }

    #[tokio::test]
    async fn failed_switch_restores_left_changes() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        write(&repo, "a.txt", "wip\n");
        let missing = Target::Local("does-not-exist".into());
        let err = switch(
            &git,
            &repo,
            &missing,
            Some("main"),
            true,
            LocalChanges::Leave,
        )
        .await;
        assert!(err.is_err());
        assert!(
            repo.join("a.txt").exists(),
            "changes restored after failed switch"
        );
        assert!(stash::find(&git, &repo, "main").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn bring_blocked_by_conflicting_changes() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        write(&repo, "a.txt", "1\n");
        commit_all(&repo, "init").await;
        git_in(&repo, &["switch", "-q", "-c", "other"]).await;
        write(&repo, "a.txt", "other\n");
        git_in(&repo, &["commit", "-q", "-am", "other"]).await;
        git_in(&repo, &["switch", "-q", "main"]).await;
        write(&repo, "a.txt", "local edit\n");

        let other = Target::Local("other".into());
        match switch(&git, &repo, &other, Some("main"), true, LocalChanges::Bring).await {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::LocalChangesBlock),
            res => panic!("unexpected {res:?}"),
        }
    }

    #[tokio::test]
    async fn tracks_remote_only_and_deletes() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "init"]).await;
        with_bare_origin(&repo).await;
        git_in(&repo, &["push", "-q", "origin", "main:remote-feature"]).await;
        git_in(&repo, &["fetch", "-q", "origin"]).await;

        let list = branches(&git, &repo).await.unwrap();
        let target = resolve_target(&list, "origin/remote-feature").unwrap();
        assert_eq!(
            target,
            Target::Track {
                local: "remote-feature".into(),
                remote_branch: "origin/remote-feature".into()
            }
        );
        switch(
            &git,
            &repo,
            &target,
            Some("main"),
            false,
            LocalChanges::Bring,
        )
        .await
        .unwrap();
        let list = branches(&git, &repo).await.unwrap();
        assert_eq!(list.current.as_deref(), Some("remote-feature"));
        assert_eq!(
            list.local
                .iter()
                .find(|b| b.name == "remote-feature")
                .unwrap()
                .upstream
                .as_deref(),
            Some("origin/remote-feature")
        );
        assert_eq!(resolve_target(&list, "nope"), None);

        // Unmerged branch needs force.
        git_in(&repo, &["switch", "-q", "-c", "unmerged"]).await;
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "only here"]).await;
        git_in(&repo, &["switch", "-q", "main"]).await;
        match delete(&git, &repo, "unmerged", false).await {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::BranchNotMerged),
            res => panic!("unexpected {res:?}"),
        }
        delete(&git, &repo, "unmerged", true).await.unwrap();
        assert!(!git_in(&repo, &["branch"]).await.contains("unmerged"));
    }
}

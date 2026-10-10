//! Stash-on-switch (spec §8.1, GitHub Desktop behavior). Callers hold the mutation lock.
//!
//! Tenajlo stashes are tagged `!!Tenajlo<branch>` so we only ever restore our own, onto the
//! branch they came from.

use std::path::Path;

use serde::Serialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

fn marker(branch: &str) -> String {
    format!("!!Tenajlo<{branch}>")
}

/// Changes Tenajlo saved when the user left a branch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedChanges {
    pub branch: String,
    /// `stash@{n}`.
    #[serde(skip)]
    pub stash_ref: String,
}

/// Saves all working-directory changes (including untracked files) for `branch`.
pub async fn save(git: &GitBinary, root: &Path, branch: &str) -> Result<(), GitError> {
    let message = marker(branch);
    GitCommand::new(
        [
            "stash",
            "push",
            "--include-untracked",
            "-m",
            message.as_str(),
        ],
        Access::Mutating,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(())
}

/// The most recent Tenajlo stash for `branch`, if any.
pub async fn find(
    git: &GitBinary,
    root: &Path,
    branch: &str,
) -> Result<Option<SavedChanges>, GitError> {
    let out = GitCommand::new(["stash", "list", "--format=%gd%x00%gs"], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    let suffix = format!(": {}", marker(branch));
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|line| {
            let (stash_ref, subject) = line.split_once('\0')?;
            subject.ends_with(&suffix).then(|| SavedChanges {
                branch: branch.to_owned(),
                stash_ref: stash_ref.to_owned(),
            })
        }))
}

/// Re-applies and removes a stash found by [`find`].
pub async fn restore(git: &GitBinary, root: &Path, saved: &SavedChanges) -> Result<(), GitError> {
    if !is_stash_ref(&saved.stash_ref) {
        return Err(GitError::Parse(format!(
            "unexpected stash ref {}",
            saved.stash_ref
        )));
    }
    GitCommand::new(["stash", "pop", saved.stash_ref.as_str()], Access::Mutating)
        .cwd(root)
        .run(git)
        .await?;
    Ok(())
}

fn is_stash_ref(s: &str) -> bool {
    s.strip_prefix("stash@{")
        .and_then(|r| r.strip_suffix('}'))
        .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{commit_all, git, git_in, init_repo, write};

    #[tokio::test]
    async fn save_find_restore() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        write(&repo, "a.txt", "1\n");
        commit_all(&repo, "init").await;

        // A user's own stash must never be picked up.
        write(&repo, "a.txt", "mine\n");
        git_in(&repo, &["stash", "push", "-q", "-m", "manual"]).await;

        write(&repo, "a.txt", "2\n");
        write(&repo, "ü new.txt", "n\n");
        save(&git, &repo, "main").await.unwrap();
        assert!(
            !repo.join("ü new.txt").exists(),
            "untracked files are saved too"
        );
        assert_eq!(find(&git, &repo, "other").await.unwrap(), None);

        let saved = find(&git, &repo, "main").await.unwrap().unwrap();
        assert_eq!(saved.stash_ref, "stash@{0}");
        restore(&git, &repo, &saved).await.unwrap();
        assert_eq!(
            crate::git::test_support::read_text(&repo.join("a.txt")),
            "2\n"
        );
        assert!(repo.join("ü new.txt").exists());
        assert_eq!(find(&git, &repo, "main").await.unwrap(), None);
        let list = git_in(&repo, &["stash", "list"]).await;
        assert!(list.contains("manual"), "user's stash untouched: {list}");
    }

    #[test]
    fn stash_refs() {
        assert!(is_stash_ref("stash@{0}"));
        assert!(is_stash_ref("stash@{12}"));
        assert!(!is_stash_ref("stash@{-1}"));
        assert!(!is_stash_ref("--all"));
    }
}

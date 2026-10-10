//! Repository root detection.

use std::path::{Path, PathBuf};

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

/// Returns the canonical working-tree root containing `dir`.
///
/// Fails with `NotARepository` for plain folders and bare repositories.
pub async fn show_toplevel(git: &GitBinary, dir: &Path) -> Result<PathBuf, GitError> {
    let out = GitCommand::new(["rev-parse", "--show-toplevel"], Access::ReadOnly)
        .cwd(dir)
        .run(git)
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    let root = text.trim_end_matches(['\n', '\r']);
    if root.is_empty() {
        return Err(GitError::Parse("empty rev-parse output".into()));
    }
    // dunce avoids `\\?\` verbatim paths on Windows, which git mishandles.
    dunce::canonicalize(root)
        .map_err(|e| GitError::Parse(format!("cannot canonicalize repository root: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::error::GitErrorKind;
    use crate::git::test_support::{git, init_repo};

    #[tokio::test]
    async fn finds_root_from_subdirectory() {
        let (_tmp, repo) = init_repo().await;
        let sub = repo.join("a b/ünï");
        std::fs::create_dir_all(&sub).unwrap();
        let git = git();
        assert_eq!(
            show_toplevel(&git, &sub).await.unwrap(),
            dunce::canonicalize(&repo).unwrap()
        );
    }

    #[tokio::test]
    async fn plain_folder_is_not_a_repository() {
        let tmp = tempfile::tempdir().unwrap();
        let git = git();
        match show_toplevel(&git, tmp.path()).await {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::NotARepository),
            other => panic!("unexpected {other:?}"),
        }
    }
}

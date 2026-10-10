//! Working directory status (spec §5.3).

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::status::{parse_status, WorkingDirectoryStatus};

/// Reads the status of the repository at `root`.
pub async fn status(git: &GitBinary, root: &Path) -> Result<WorkingDirectoryStatus, GitError> {
    let out = GitCommand::new(
        [
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(parse_status(&out.stdout))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::status::{FileStatusKind, StagedState};
    use crate::git::test_support::{commit_all, git, git_in, init_repo, write};

    #[tokio::test]
    async fn reports_real_repository_changes() {
        let (_tmp, repo) = init_repo().await;
        let git = git();

        let empty = status(&git, &repo).await.unwrap();
        assert_eq!(empty.branch.name.as_deref(), Some("main"));
        assert_eq!(empty.branch.tip, None);

        write(&repo, "keep.txt", "one\n");
        write(&repo, "move me.txt", "move\ncontent\nhere\n");
        commit_all(&repo, "init").await;

        write(&repo, "keep.txt", "two\n");
        write(&repo, "ünï/new file.txt", "x\n");
        git_in(&repo, &["mv", "move me.txt", "moved.txt"]).await;

        let s = status(&git, &repo).await.unwrap();
        assert!(s.branch.tip.is_some());
        let find = |p: &str| {
            s.files
                .iter()
                .find(|f| f.path == p)
                .unwrap_or_else(|| panic!("{p} in {:?}", s.files))
        };
        assert_eq!(find("keep.txt").kind, FileStatusKind::Modified);
        assert_eq!(find("ünï/new file.txt").kind, FileStatusKind::Untracked);
        let moved = find("moved.txt");
        assert_eq!(
            (moved.kind, moved.staged),
            (FileStatusKind::Renamed, StagedState::Full)
        );
        assert_eq!(moved.old_path.as_deref(), Some("move me.txt"));
    }
}

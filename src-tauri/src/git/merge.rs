//! Merges in progress and conflict resolution (spec §8.1 conflict banner).
//!
//! Tenajlo starts merges only from "Merge the server's changes" (a pull that would otherwise
//! fail with `PullDiverged`), but also recognizes a rebase or cherry-pick started elsewhere,
//! so the user can at least abort it.

use std::io::Read;
use std::path::{Path, PathBuf};

use serde::Serialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::status::{FileStatusKind, WorkingDirectoryStatus};

/// Larger files aren't scanned for conflict markers (they're shown as "unknown").
const MAX_SCAN_BYTES: u64 = 10 * 1024 * 1024;

/// A multi-step git operation that's waiting for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum OperationKind {
    Merge,
    Rebase,
    CherryPick,
    Revert,
}

/// A conflicted file and how many conflict regions are still in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ConflictedFile {
    pub path: String,
    /// `<<<<<<<` markers left; `None` for binary or very large files.
    pub markers: Option<u32>,
}

/// What the conflict banner shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct OperationState {
    pub operation: Option<OperationKind>,
    pub conflicts: Vec<ConflictedFile>,
    /// First line of git's prepared merge message, to prefill the commit summary.
    pub merge_summary: Option<String>,
}

/// The operation in progress (if any) and the conflicted files from `status`.
pub async fn operation_state(
    git: &GitBinary,
    root: &Path,
    status: &WorkingDirectoryStatus,
) -> Result<OperationState, GitError> {
    const MARKERS: [(&str, OperationKind); 5] = [
        ("rebase-merge", OperationKind::Rebase),
        ("rebase-apply", OperationKind::Rebase),
        ("MERGE_HEAD", OperationKind::Merge),
        ("CHERRY_PICK_HEAD", OperationKind::CherryPick),
        ("REVERT_HEAD", OperationKind::Revert),
    ];
    let mut args = vec!["rev-parse"];
    for (name, _) in MARKERS {
        args.extend(["--git-path", name]);
    }
    args.extend(["--git-path", "MERGE_MSG"]);
    let out = GitCommand::new(args, Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    let paths: Vec<PathBuf> = text.lines().map(|l| root.join(l)).collect();
    let conflicted: Vec<String> = status
        .files
        .iter()
        .filter(|f| f.kind == FileStatusKind::Conflicted)
        .map(|f| f.path.clone())
        .collect();
    let root = root.to_path_buf();

    // Marker checks and the conflict scan (up to MAX_SCAN_BYTES per file) are blocking I/O.
    tokio::task::spawn_blocking(move || {
        let operation = MARKERS
            .iter()
            .zip(&paths)
            .find(|(_, p)| p.exists())
            .map(|((_, kind), _)| *kind);
        let merge_summary = (operation == Some(OperationKind::Merge))
            .then(|| paths.last().and_then(|p| std::fs::read_to_string(p).ok()))
            .flatten()
            .and_then(|msg| {
                msg.lines()
                    .find(|l| !l.trim().is_empty())
                    .map(str::to_owned)
            });
        let conflicts = conflicted
            .into_iter()
            .map(|path| ConflictedFile {
                markers: count_markers(&root.join(&path)),
                path,
            })
            .collect();
        OperationState {
            operation,
            conflicts,
            merge_summary,
        }
    })
    .await
    .map_err(|e| GitError::Spawn(std::io::Error::other(e)))
}

/// Counts `<<<<<<< ` lines. `None` for unreadable, binary or very large files.
fn count_markers(path: &Path) -> Option<u32> {
    let file = std::fs::File::open(path).ok()?;
    if file.metadata().ok()?.len() > MAX_SCAN_BYTES {
        return None;
    }
    let mut bytes = Vec::new();
    file.take(MAX_SCAN_BYTES).read_to_end(&mut bytes).ok()?;
    if bytes.contains(&0) {
        return None;
    }
    let count = bytes
        .split(|b| *b == b'\n')
        .filter(|line| line.starts_with(b"<<<<<<< ") || line == b"<<<<<<<")
        .count();
    u32::try_from(count).ok()
}

/// Marks conflicted files resolved (`git add`). Paths must be conflicted in `status`.
pub async fn mark_resolved(git: &GitBinary, root: &Path, paths: &[String]) -> Result<(), GitError> {
    if paths.is_empty() {
        return Ok(());
    }
    GitCommand::new(["add"], Access::Mutating)
        .cwd(root)
        .pathspecs_on_stdin(paths)
        .run(git)
        .await?;
    Ok(())
}

/// Abandons the operation and restores the branch to how it was before it started.
pub async fn abort(git: &GitBinary, root: &Path, kind: OperationKind) -> Result<(), GitError> {
    let command = match kind {
        OperationKind::Merge => "merge",
        OperationKind::Rebase => "rebase",
        OperationKind::CherryPick => "cherry-pick",
        OperationKind::Revert => "revert",
    };
    GitCommand::new([command, "--abort"], Access::Mutating)
        .cwd(root)
        .run(git)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::status::status;
    use crate::git::test_support::{commit_all, git, git_in, init_repo, write};

    /// `main` and `other` both change `a.txt` → merging `other` conflicts.
    async fn conflicted_merge() -> (tempfile::TempDir, std::path::PathBuf) {
        let (tmp, repo) = init_repo().await;
        write(&repo, "a.txt", "base\n");
        write(&repo, "ünï b.txt", "same\n");
        commit_all(&repo, "base").await;
        git_in(&repo, &["switch", "-q", "-c", "other"]).await;
        write(&repo, "a.txt", "theirs\n");
        git_in(&repo, &["commit", "-q", "-am", "theirs"]).await;
        git_in(&repo, &["switch", "-q", "main"]).await;
        write(&repo, "a.txt", "ours\n");
        git_in(&repo, &["commit", "-q", "-am", "ours"]).await;
        let git = git();
        // CI machines have no global identity; the merge needs one to start.
        let args = [
            "-c",
            "user.name=Test",
            "-c",
            "user.email=t@example.com",
            "merge",
            "--no-edit",
            "other",
        ];
        let res = GitCommand::new(args, Access::Mutating)
            .cwd(&repo)
            .run(&git)
            .await;
        assert!(
            matches!(
                res,
                Err(GitError::Failed {
                    kind: crate::git::error::GitErrorKind::MergeConflict,
                    ..
                })
            ),
            "merge should conflict: {res:?}"
        );
        (tmp, repo)
    }

    #[tokio::test]
    async fn detects_merge_conflicts_and_resolves_them() {
        let (_tmp, repo) = conflicted_merge().await;
        let git = git();
        let st = status(&git, &repo).await.unwrap();
        let state = operation_state(&git, &repo, &st).await.unwrap();
        assert_eq!(state.operation, Some(OperationKind::Merge));
        assert_eq!(
            state.conflicts,
            vec![ConflictedFile {
                path: "a.txt".into(),
                markers: Some(1)
            }]
        );
        assert_eq!(state.merge_summary.as_deref(), Some("Merge branch 'other'"));

        write(&repo, "a.txt", "resolved\n");
        mark_resolved(&git, &repo, &["a.txt".into()]).await.unwrap();
        let st = status(&git, &repo).await.unwrap();
        assert!(!st.has_conflicts);
        let state = operation_state(&git, &repo, &st).await.unwrap();
        assert_eq!(
            state.operation,
            Some(OperationKind::Merge),
            "still merging until committed"
        );
        assert!(state.conflicts.is_empty());
    }

    #[tokio::test]
    async fn abort_restores_the_branch() {
        let (_tmp, repo) = conflicted_merge().await;
        let git = git();
        abort(&git, &repo, OperationKind::Merge).await.unwrap();
        let st = status(&git, &repo).await.unwrap();
        assert!(st.files.is_empty());
        assert_eq!(
            crate::git::test_support::read_text(&repo.join("a.txt")),
            "ours\n"
        );
        let state = operation_state(&git, &repo, &st).await.unwrap();
        assert_eq!(state, OperationState::default());
    }

    #[test]
    fn marker_counting() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("f");
        std::fs::write(
            &p,
            "<<<<<<< HEAD\na\n=======\nb\n>>>>>>> x\n<<<<<<< HEAD\r\n",
        )
        .unwrap();
        assert_eq!(count_markers(&p), Some(2));
        std::fs::write(&p, "  <<<<<<< not at start\n").unwrap();
        assert_eq!(count_markers(&p), Some(0));
        std::fs::write(&p, b"bin\0<<<<<<< \n").unwrap();
        assert_eq!(count_markers(&p), None);
        assert_eq!(count_markers(&dir.path().join("missing")), None);
    }
}

//! Discarding working-directory changes (spec §5.3, §8.2). Callers hold the mutation lock.
//!
//! Every file whose current content would be lost is first moved to the OS trash from its
//! real location, then git resets the path. Nothing is ever hard-deleted.

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::status::{FileChange, FileStatusKind};

const FROM_STDIN: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];

/// Why a discard did not fully happen.
#[derive(Debug, thiserror::Error)]
pub enum DiscardError {
    #[error("conflicted files and submodules can't be discarded here: {0:?}")]
    Unsupported(Vec<String>),
    /// Some files couldn't be moved to the trash. Their changes are untouched; all other
    /// selected files were discarded.
    #[error("could not move to trash: {0:?}")]
    TrashFailed(Vec<(String, String)>),
    #[error(transparent)]
    Git(#[from] GitError),
}

/// Discards all changes (staged and unstaged) to `files`.
///
/// `trash` moves one file to the trash; tests inject a fake. `has_head` is false before
/// the first commit.
pub async fn discard(
    git: &GitBinary,
    root: &Path,
    files: &[&FileChange],
    has_head: bool,
    trash: &(dyn Fn(&Path) -> Result<(), String> + Sync),
) -> Result<(), DiscardError> {
    let unsupported: Vec<String> = files
        .iter()
        .filter(|f| f.kind == FileStatusKind::Conflicted || f.submodule)
        .map(|f| f.path.clone())
        .collect();
    if !unsupported.is_empty() {
        return Err(DiscardError::Unsupported(unsupported));
    }

    // 1. Move current content to the trash. Files that fail are left completely alone.
    let mut failed = Vec::new();
    let mut to_reset: Vec<&FileChange> = Vec::new();
    for f in files {
        let abs = root.join(&f.path);
        // symlink_metadata: a dangling symlink still exists and must be trashed, not followed.
        if abs.symlink_metadata().is_ok() {
            if let Err(e) = trash(&abs) {
                failed.push((f.path.clone(), e));
                continue;
            }
        }
        if f.kind != FileStatusKind::Untracked {
            to_reset.push(f);
        }
    }

    // 2. Reset index and working tree for tracked paths (trashed files are recreated from HEAD).
    let mut specs: Vec<&str> = Vec::new();
    for f in &to_reset {
        specs.push(&f.path);
        specs.extend(f.old_path.as_deref());
    }
    if !specs.is_empty() {
        // No-overlay restore also drops index entries absent from HEAD (added/copied/renamed-to).
        let mut args = if has_head {
            vec!["restore", "--source=HEAD", "--staged", "--worktree"]
        } else {
            vec!["rm", "--cached", "-r", "-q", "-f", "--ignore-unmatch"]
        };
        args.extend(FROM_STDIN);
        GitCommand::new(args, Access::Mutating)
            .pathspecs_on_stdin(&specs)
            .cwd(root)
            .run(git)
            .await?;
    }

    if failed.is_empty() {
        Ok(())
    } else {
        Err(DiscardError::TrashFailed(failed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::status::status;
    use crate::git::test_support::{git_in, init_repo, write};
    use std::path::PathBuf;
    use std::sync::Mutex;

    /// Fake trash: moves files into a directory and records what it received.
    struct FakeTrash {
        dir: tempfile::TempDir,
        got: Mutex<Vec<String>>,
    }

    impl FakeTrash {
        fn new() -> Self {
            Self {
                dir: tempfile::tempdir().unwrap(),
                got: Mutex::new(Vec::new()),
            }
        }
        fn trash(&self, p: &Path) -> Result<(), String> {
            let n = self.got.lock().unwrap().len();
            let dest: PathBuf = self.dir.path().join(n.to_string());
            std::fs::rename(p, dest).map_err(|e| e.to_string())?;
            self.got.lock().unwrap().push(
                std::fs::read_to_string(self.dir.path().join(n.to_string())).unwrap_or_default(),
            );
            Ok(())
        }
    }

    #[tokio::test]
    async fn discards_every_kind_and_keeps_content_in_trash() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        write(&repo, "mod.txt", "orig\n");
        write(&repo, "del.txt", "keep\n");
        write(&repo, "ren.txt", "a\nb\nc\nd\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "init"]).await;

        write(&repo, "mod.txt", "edited and staged\n");
        git_in(&repo, &["add", "mod.txt"]).await;
        std::fs::remove_file(repo.join("del.txt")).unwrap();
        git_in(&repo, &["mv", "ren.txt", "ren2.txt"]).await;
        write(&repo, "added[1].txt", "new staged\n");
        git_in(&repo, &["add", "--", "added[1].txt"]).await;
        write(&repo, "ü untracked.txt", "scratch\n");
        write(&repo, "keep me.txt", "not selected\n");

        let s = status(&git, &repo).await.unwrap();
        let chosen: Vec<&FileChange> = s.files.iter().filter(|f| f.path != "keep me.txt").collect();
        let fake = FakeTrash::new();
        discard(&git, &repo, &chosen, true, &|p| fake.trash(p))
            .await
            .unwrap();

        let after = status(&git, &repo).await.unwrap();
        let left: Vec<&str> = after.files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(left, vec!["keep me.txt"]);
        assert_eq!(
            crate::git::test_support::read_text(&repo.join("mod.txt")),
            "orig\n"
        );
        assert_eq!(
            crate::git::test_support::read_text(&repo.join("del.txt")),
            "keep\n"
        );
        assert!(repo.join("ren.txt").exists() && !repo.join("ren2.txt").exists());

        let mut trashed = fake.got.lock().unwrap().clone();
        trashed.sort();
        assert_eq!(
            trashed,
            vec![
                "a\nb\nc\nd\n",
                "edited and staged\n",
                "new staged\n",
                "scratch\n"
            ]
        );
    }

    #[tokio::test]
    async fn before_first_commit_and_trash_failures() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        write(&repo, "a.txt", "a\n");
        write(&repo, "b.txt", "b\n");
        git_in(&repo, &["add", "-A"]).await;
        let s = status(&git, &repo).await.unwrap();
        let all: Vec<&FileChange> = s.files.iter().collect();

        let fake = FakeTrash::new();
        let res = discard(&git, &repo, &all, false, &|p| {
            if p.ends_with("b.txt") {
                Err("locked".into())
            } else {
                fake.trash(p)
            }
        })
        .await;
        assert!(
            matches!(res, Err(DiscardError::TrashFailed(ref f)) if f.len() == 1 && f[0].0 == "b.txt")
        );
        let after = status(&git, &repo).await.unwrap();
        assert_eq!(
            after.files.len(),
            1,
            "only b.txt remains, still staged: {:?}",
            after.files
        );
        assert_eq!(after.files[0].path, "b.txt");
    }

    #[tokio::test]
    async fn refuses_conflicts_and_submodules() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        let conflicted = FileChange {
            path: "c".into(),
            old_path: None,
            kind: FileStatusKind::Conflicted,
            staged: crate::git::parse::status::StagedState::None,
            submodule: false,
        };
        let res = discard(&git, &repo, &[&conflicted], true, &|_| {
            panic!("must not trash")
        })
        .await;
        assert!(matches!(res, Err(DiscardError::Unsupported(_))));
    }
}

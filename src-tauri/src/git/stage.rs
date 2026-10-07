//! Staging and unstaging (spec §5.3). Callers hold the repository's mutation lock.

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::status::{FileChange, FileStatusKind, StagedState};

const FROM_STDIN: [&str; 2] = ["--pathspec-from-file=-", "--pathspec-file-nul"];

/// Stages `files` completely (content, additions and deletions).
/// Conflicted files are skipped: resolving them is an explicit action (spec §8.1, M6).
pub async fn stage(git: &GitBinary, root: &Path, files: &[&FileChange]) -> Result<(), GitError> {
    // Only files with unstaged changes, by current path. `git add` rejects pathspecs that are
    // in neither the index nor the working tree (a staged rename's old path, a staged deletion).
    let specs: Vec<&str> = files
        .iter()
        .filter(|f| f.kind != FileStatusKind::Conflicted && f.staged != StagedState::Full)
        .map(|f| f.path.as_str())
        .collect();
    run_with_pathspecs(git, root, &["add", "--all"], &specs).await
}

/// Removes `files` from the index, keeping working-tree content.
/// `has_head` is false before the first commit, where there is nothing to restore from.
pub async fn unstage(
    git: &GitBinary,
    root: &Path,
    files: &[&FileChange],
    has_head: bool,
) -> Result<(), GitError> {
    // Renames need the original path too, so its deletion is unstaged along with the addition.
    let mut specs = Vec::new();
    for f in files.iter().filter(|f| f.staged != StagedState::None) {
        specs.push(f.path.as_str());
        specs.extend(f.old_path.as_deref());
    }
    let args: &[&str] = if has_head {
        &["restore", "--staged"]
    } else {
        &["rm", "--cached", "-r", "-q"]
    };
    run_with_pathspecs(git, root, args, &specs).await
}

/// Runs a mutating git command with `specs` passed as literal pathspecs on stdin.
async fn run_with_pathspecs(
    git: &GitBinary,
    root: &Path,
    args: &[&str],
    specs: &[&str],
) -> Result<(), GitError> {
    if specs.is_empty() {
        return Ok(());
    }
    let mut full = args.to_vec();
    full.extend(FROM_STDIN);
    GitCommand::new(full, Access::Mutating)
        .pathspecs_on_stdin(specs)
        .cwd(root)
        .run(git)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::status::status;
    use crate::git::test_support::{git_in, init_repo, write};

    async fn staged_of(git: &GitBinary, root: &Path) -> Vec<(String, StagedState)> {
        let mut v: Vec<_> = status(git, root)
            .await
            .unwrap()
            .files
            .into_iter()
            .map(|f| (f.path, f.staged))
            .collect();
        v.sort_by(|a, b| a.0.cmp(&b.0));
        v
    }

    #[tokio::test]
    async fn stage_and_unstage_round_trip() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();

        // Before the first commit (no HEAD): uses rm --cached to unstage.
        write(&repo, "a*.txt", "1\n");
        write(&repo, "ab.txt", "1\n");
        let s = status(&git, &repo).await.unwrap();
        let star: Vec<&FileChange> = s.files.iter().filter(|f| f.path == "a*.txt").collect();
        stage(&git, &repo, &star).await.unwrap();
        assert_eq!(
            staged_of(&git, &repo).await,
            vec![
                ("a*.txt".into(), StagedState::Full),
                ("ab.txt".into(), StagedState::None)
            ],
            "a literal '*' must not stage ab.txt"
        );
        let s = status(&git, &repo).await.unwrap();
        let all: Vec<&FileChange> = s.files.iter().collect();
        unstage(&git, &repo, &all, false).await.unwrap();
        assert!(staged_of(&git, &repo)
            .await
            .iter()
            .all(|(_, st)| *st == StagedState::None));

        // With HEAD: deletions, renames, unicode, and a leading dash.
        let s = status(&git, &repo).await.unwrap();
        let all: Vec<&FileChange> = s.files.iter().collect();
        stage(&git, &repo, &all).await.unwrap();
        git_in(&repo, &["commit", "-q", "-m", "init"]).await;
        std::fs::remove_file(repo.join("ab.txt")).unwrap();
        git_in(&repo, &["mv", "--", "a*.txt", "-ü moved.txt"]).await;
        write(&repo, "-ü moved.txt", "1\n2\n");

        let s = status(&git, &repo).await.unwrap();
        let all: Vec<&FileChange> = s.files.iter().collect();
        stage(&git, &repo, &all).await.unwrap();
        let after = staged_of(&git, &repo).await;
        assert!(
            after.iter().all(|(_, st)| *st == StagedState::Full),
            "{after:?}"
        );

        let s = status(&git, &repo).await.unwrap();
        let all: Vec<&FileChange> = s.files.iter().collect();
        unstage(&git, &repo, &all, true).await.unwrap();
        let after = staged_of(&git, &repo).await;
        assert!(
            after.iter().all(|(_, st)| *st == StagedState::None),
            "{after:?}"
        );
        assert!(
            repo.join("-ü moved.txt").exists(),
            "unstage keeps working-tree content"
        );
    }
}

//! File diffs for the working directory and commits (spec §5.3).

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::diff::{parse_diff, FileDiff};
use super::parse::status::{FileChange, FileStatusKind};

const DIFF_FLAGS: [&str; 4] = ["--no-ext-diff", "--patience", "--no-color", "-M"];

/// `:(literal)` pathspec so `*`, `?` and `[` in file names are not globs.
fn literal(path: &str) -> String {
    format!(":(literal){path}")
}

/// Diff of a changed working-directory file against HEAD (staged and unstaged together).
///
/// `has_head` is false in a repository with no commits yet.
pub async fn working_dir_diff(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
) -> Result<FileDiff, GitError> {
    let treat_as_new = file.kind == FileStatusKind::Untracked || !has_head;
    let cmd = if treat_as_new {
        if file.kind == FileStatusKind::Deleted {
            return Ok(FileDiff::Unchanged);
        }
        reject_escaping_path(&file.path)?;
        // --no-index takes real paths (no pathspec magic) and exits 1 when files differ.
        let mut args = vec!["diff", "--no-index"];
        args.extend(DIFF_FLAGS);
        args.extend(["--", "/dev/null", file.path.as_str()]);
        GitCommand::new(args, Access::ReadOnly).ok_exit_codes(&[0, 1])
    } else {
        let mut args: Vec<String> = vec!["diff".into()];
        args.extend(DIFF_FLAGS.map(String::from));
        args.extend(["HEAD".into(), "--".into()]);
        if let Some(old) = &file.old_path {
            args.push(literal(old));
        }
        args.push(literal(&file.path));
        GitCommand::new(args, Access::ReadOnly)
    };
    let out = cmd.cwd(root).run(git).await?;
    Ok(parse_diff(&out.stdout))
}

/// Diff of one file in a commit, against its first parent (or the empty tree for a root commit).
pub async fn commit_file_diff(
    git: &GitBinary,
    root: &Path,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<FileDiff, GitError> {
    let mut args: Vec<String> = vec![
        "show".into(),
        "--format=".into(),
        "--diff-merges=first-parent".into(),
    ];
    args.extend(DIFF_FLAGS.map(String::from));
    args.extend(["--end-of-options".into(), sha.to_owned(), "--".into()]);
    if let Some(old) = old_path {
        args.push(literal(old));
    }
    args.push(literal(path));
    let out = GitCommand::new(args, Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    Ok(parse_diff(&out.stdout))
}

/// `--no-index` paths are real filesystem paths, so they must stay inside the repository.
fn reject_escaping_path(path: &str) -> Result<(), GitError> {
    let p = Path::new(path);
    let escapes = p.is_absolute()
        || p.components().any(|c| {
            !matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        });
    if escapes {
        return Err(GitError::Parse(format!(
            "refusing path outside repository: {path}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::parse::diff::DiffLineKind;
    use crate::git::status::status;
    use crate::git::test_support::{git_in, init_repo, write};

    fn added_lines(diff: &FileDiff) -> Vec<String> {
        match diff {
            FileDiff::Text { hunks } => hunks
                .iter()
                .flat_map(|h| &h.lines)
                .filter(|l| l.kind == DiffLineKind::Add)
                .map(|l| l.text.clone())
                .collect(),
            other => panic!("expected text, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn working_dir_diffs_by_kind() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();

        // Unborn HEAD: staged file is shown as all-new.
        write(&repo, "a*.txt", "first\n");
        write(&repo, "ab.txt", "other\n");
        git_in(&repo, &["add", "-A"]).await;
        let s = status(&git, &repo).await.unwrap();
        let star = s.files.iter().find(|f| f.path == "a*.txt").unwrap();
        assert_eq!(
            added_lines(&working_dir_diff(&git, &repo, star, false).await.unwrap()),
            vec!["first"]
        );

        git_in(&repo, &["commit", "-q", "-m", "init"]).await;
        write(&repo, "a*.txt", "first\nsecond\n");
        write(&repo, "ab.txt", "changed too\n");
        write(&repo, "dir/ü new.txt", "hello\n");
        write(&repo, "img.bin", "\0\x01\x02");

        let s = status(&git, &repo).await.unwrap();
        let get = |p: &str| s.files.iter().find(|f| f.path == p).unwrap().clone();

        // Literal pathspec: "a*.txt" must not also pull in ab.txt.
        let d = working_dir_diff(&git, &repo, &get("a*.txt"), true)
            .await
            .unwrap();
        assert_eq!(added_lines(&d), vec!["second"]);

        let d = working_dir_diff(&git, &repo, &get("dir/ü new.txt"), true)
            .await
            .unwrap();
        assert_eq!(added_lines(&d), vec!["hello"]);

        let d = working_dir_diff(&git, &repo, &get("img.bin"), true)
            .await
            .unwrap();
        assert_eq!(d, FileDiff::Binary);
    }

    #[tokio::test]
    async fn rename_and_commit_diffs() {
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();
        write(&repo, "old.txt", "a\nb\nc\nd\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "root"]).await;
        git_in(&repo, &["mv", "old.txt", "new.txt"]).await;
        write(&repo, "new.txt", "a\nb\nc\nd\ne\n");

        let s = status(&git, &repo).await.unwrap();
        let renamed = s.files.iter().find(|f| f.path == "new.txt").unwrap();
        let d = working_dir_diff(&git, &repo, renamed, true).await.unwrap();
        assert_eq!(
            added_lines(&d),
            vec!["e"],
            "rename should diff old→new, not show a whole new file"
        );

        // Root commit has no parent; show handles it.
        let root_sha = git_in(&repo, &["rev-parse", "HEAD"]).await;
        let d = commit_file_diff(&git, &repo, root_sha.trim(), "old.txt", None)
            .await
            .unwrap();
        assert_eq!(added_lines(&d), vec!["a", "b", "c", "d"]);
    }

    #[test]
    fn rejects_escaping_paths() {
        assert!(reject_escaping_path("../secret").is_err());
        assert!(reject_escaping_path("/etc/passwd").is_err());
        assert!(reject_escaping_path("a/../../b").is_err());
        assert!(reject_escaping_path("a/b c.txt").is_ok());
    }
}

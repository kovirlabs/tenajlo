//! File diffs for the working directory and commits (spec §5.3).

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::diff::{parse_diff, FileDiff, MAX_DIFF_BYTES};
use super::parse::status::{FileChange, FileStatusKind};

const DIFF_FLAGS: [&str; 4] = ["--no-ext-diff", "--patience", "--no-color", "-M"];

/// Diff of a changed working-directory file against HEAD (staged and unstaged together).
///
/// `has_head` is false in a repository with no commits yet.
pub async fn working_dir_diff(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
) -> Result<FileDiff, GitError> {
    Ok(match working_dir_patch(git, root, file, has_head).await? {
        WorkingPatch::Raw(bytes) => parse_diff(&bytes),
        WorkingPatch::Settled(diff) => diff,
    })
}

/// The raw patch behind [`working_dir_diff`], or the result when no patch is needed.
pub(crate) enum WorkingPatch {
    Raw(Vec<u8>),
    Settled(FileDiff),
}

/// Runs the HEAD → working tree diff for one file and returns git's output unparsed.
pub(crate) async fn working_dir_patch(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
) -> Result<WorkingPatch, GitError> {
    let treat_as_new = file.kind == FileStatusKind::Untracked || !has_head;
    let cmd = if treat_as_new {
        if file.kind == FileStatusKind::Deleted {
            return Ok(WorkingPatch::Settled(FileDiff::Unchanged));
        }
        reject_escaping_path(&file.path)?;
        if is_large_text_file(&root.join(&file.path)).await {
            return Ok(WorkingPatch::Settled(FileDiff::TooLarge));
        }
        // --no-index takes real paths (no pathspec magic) and exits 1 when files differ.
        let mut args = vec!["diff", "--no-index"];
        args.extend(DIFF_FLAGS);
        args.extend(["--", "/dev/null", file.path.as_str()]);
        GitCommand::new(args, Access::ReadOnly).ok_exit_codes(&[0, 1])
    } else {
        let mut args = vec!["diff"];
        args.extend(DIFF_FLAGS);
        args.extend(["HEAD", "--"]);
        args.extend(file.old_path.as_deref());
        args.push(&file.path);
        // Literal so `*`, `?` and `[` in file names are not globs.
        GitCommand::new(args, Access::ReadOnly).literal_pathspecs()
    };
    Ok(WorkingPatch::Raw(cmd.cwd(root).run(git).await?.stdout))
}

/// Raw diff of one file between two of HEAD, the index and the working tree (`args` picks
/// which, e.g. `["--cached"]` for HEAD → index, `[]` for index → working tree).
pub(crate) async fn raw_diff(
    git: &GitBinary,
    root: &Path,
    args: &[&str],
    path: &str,
) -> Result<Vec<u8>, GitError> {
    let mut all = vec!["diff"];
    all.extend(DIFF_FLAGS);
    all.extend(args);
    all.extend(["--", path]);
    let out = GitCommand::new(all, Access::ReadOnly)
        .literal_pathspecs()
        .cwd(root)
        .run(git)
        .await?;
    Ok(out.stdout)
}

/// Diff of one file in a commit, against its first parent (or the empty tree for a root commit).
pub async fn commit_file_diff(
    git: &GitBinary,
    root: &Path,
    sha: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<FileDiff, GitError> {
    let mut args = vec!["show", "--format=", "--diff-merges=first-parent"];
    args.extend(DIFF_FLAGS);
    args.extend(["--end-of-options", sha, "--"]);
    args.extend(old_path);
    args.push(path);
    let out = GitCommand::new(args, Access::ReadOnly)
        .literal_pathspecs()
        .cwd(root)
        .run(git)
        .await?;
    Ok(parse_diff(&out.stdout))
}

/// True when a new file is too big to diff and is not binary. Checked before spawning git
/// so a huge untracked file is never buffered whole just to be rejected by `parse_diff`.
/// Large binary files fall through: git prints a one-line "Binary files differ" for them.
async fn is_large_text_file(path: &Path) -> bool {
    use tokio::io::AsyncReadExt;

    // Symlinks are diffed as their (short) target text, so they are never large.
    let Ok(meta) = tokio::fs::symlink_metadata(path).await else {
        return false;
    };
    if !meta.is_file() || meta.len() <= MAX_DIFF_BYTES as u64 {
        return false;
    }
    // Same heuristic as git: a NUL byte in the first 8000 bytes means binary.
    let mut head = Vec::with_capacity(8000);
    let read = match tokio::fs::File::open(path).await {
        Ok(f) => f.take(8000).read_to_end(&mut head).await,
        Err(e) => Err(e),
    };
    read.is_ok() && !head.contains(&0)
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
    use crate::git::parse::diff::DiffLineKind;
    use crate::git::status::status;
    use crate::git::test_support::{commit_all, git, git_in, init_repo, write};

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
        let git = git();

        // Unborn HEAD: staged file is shown as all-new.
        write(&repo, "a[b].txt", "first\n");
        write(&repo, "ab.txt", "other\n");
        git_in(&repo, &["add", "-A"]).await;
        let s = status(&git, &repo).await.unwrap();
        let star = s.files.iter().find(|f| f.path == "a[b].txt").unwrap();
        assert_eq!(
            added_lines(&working_dir_diff(&git, &repo, star, false).await.unwrap()),
            vec!["first"]
        );

        git_in(&repo, &["commit", "-q", "-m", "init"]).await;
        write(&repo, "a[b].txt", "first\nsecond\n");
        write(&repo, "ab.txt", "changed too\n");
        write(&repo, "dir/ü new.txt", "hello\n");
        write(&repo, "img.bin", "\0\x01\x02");

        let s = status(&git, &repo).await.unwrap();
        let get = |p: &str| s.files.iter().find(|f| f.path == p).unwrap().clone();

        // Literal pathspec: "a[b].txt" must not also pull in ab.txt.
        let d = working_dir_diff(&git, &repo, &get("a[b].txt"), true)
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
        let git = git();
        write(&repo, "old.txt", "a\nb\nc\nd\n");
        commit_all(&repo, "root").await;
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

    #[tokio::test]
    async fn large_new_files_skip_git_unless_binary() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        let mut text = "line\n".repeat(MAX_DIFF_BYTES / 5 + 1);
        write(&repo, "big.csv", &text);
        text.insert(0, '\0');
        write(&repo, "big.bin", &text);

        let s = status(&git, &repo).await.unwrap();
        let get = |p: &str| s.files.iter().find(|f| f.path == p).unwrap().clone();
        let (csv, bin) = (get("big.csv"), get("big.bin"));
        let csv = working_dir_diff(&git, &repo, &csv, false).await.unwrap();
        let bin = working_dir_diff(&git, &repo, &bin, false).await.unwrap();
        assert_eq!(csv, FileDiff::TooLarge);
        assert_eq!(bin, FileDiff::Binary);
    }

    #[test]
    fn rejects_escaping_paths() {
        assert!(reject_escaping_path("../secret").is_err());
        assert!(reject_escaping_path("/etc/passwd").is_err());
        assert!(reject_escaping_path("a/../../b").is_err());
        assert!(reject_escaping_path("a/b c.txt").is_ok());
    }
}

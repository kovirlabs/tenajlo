//! Line staging against real temporary repositories.

use std::path::Path;

use super::*;
use crate::git::status::status;
use crate::git::test_support::{commit_all, git, git_in, init_repo, write};

/// The current status entry for `path` and whether HEAD exists.
async fn file(root: &Path, path: &str) -> (FileChange, bool) {
    let s = status(&git(), root).await.unwrap();
    let f = s.files.into_iter().find(|f| f.path == path).unwrap();
    (f, s.branch.tip.is_some())
}

async fn staging(root: &Path, path: &str) -> LineStaging {
    let (f, has_head) = file(root, path).await;
    working_diff(&git(), root, &f, has_head)
        .await
        .unwrap()
        .lines
        .expect("line staging supported")
}

/// Stages or unstages `lines` against the diff as it is now.
async fn set(root: &Path, path: &str, lines: &[(u32, u32)], stage: bool) {
    try_set(root, path, lines, stage).await.unwrap();
}

async fn try_set(
    root: &Path,
    path: &str,
    lines: &[(u32, u32)],
    stage: bool,
) -> Result<(), LineStageError> {
    let token = staging(root, path).await.token;
    let (f, has_head) = file(root, path).await;
    let refs: Vec<LineRef> = lines
        .iter()
        .map(|&(hunk, line)| LineRef { hunk, line })
        .collect();
    set_lines_staged(&git(), root, &f, has_head, &token, &refs, stage).await
}

/// The staged content of `path`, or `None` if it isn't in the index.
async fn index_content(root: &Path, path: &str) -> Option<Vec<u8>> {
    let spec = format!(":{path}");
    GitCommand::new(["cat-file", "blob", &spec], Access::ReadOnly)
        .cwd(root)
        .run(&git())
        .await
        .ok()
        .map(|o| o.stdout)
}

const TEN: &str = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n";

#[tokio::test]
async fn stage_and_unstage_single_lines_of_a_modified_file() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, "a[1] ü.txt", TEN);
    commit_all(&repo, "init").await;
    // Two hunks: 2 → two at the top, and 10 → ten plus eleven at the bottom.
    write(
        &repo,
        "a[1] ü.txt",
        "1\ntwo\n3\n4\n5\n6\n7\n8\n9\nten\neleven\n",
    );
    let path = "a[1] ü.txt";

    let s = staging(&repo, path).await;
    assert!(s.staged.iter().flatten().all(|&f| !f));
    // Hunk 0 is " 1", "-2", "+two", " 3", …; stage only "-2" and "+two".
    set(&repo, path, &[(0, 1), (0, 2)], true).await;
    assert_eq!(
        index_content(&repo, path).await.unwrap(),
        b"1\ntwo\n3\n4\n5\n6\n7\n8\n9\n10\n"
    );
    let (f, _) = file(&repo, path).await;
    assert_eq!(f.staged, StagedState::Partial);
    let s = staging(&repo, path).await;
    assert!(s.staged[0][1] && s.staged[0][2]);
    assert!(s.staged[1].iter().all(|&f| !f), "{:?}", s.staged);

    // Stage just "+eleven" from the second hunk; the first hunk stays staged.
    let eleven = s.staged[1].len() as u32 - 1;
    set(&repo, path, &[(1, eleven)], true).await;
    assert_eq!(
        index_content(&repo, path).await.unwrap(),
        b"1\ntwo\n3\n4\n5\n6\n7\n8\n9\n10\neleven\n"
    );

    // Unstage the first hunk again.
    set(&repo, path, &[(0, 1), (0, 2)], false).await;
    assert_eq!(
        index_content(&repo, path).await.unwrap(),
        b"1\n2\n3\n4\n5\n6\n7\n8\n9\n10\neleven\n"
    );
    let s = staging(&repo, path).await;
    assert!(!s.staged[0][1] && !s.staged[0][2]);
    assert!(s.staged[1][eleven as usize]);
}

#[tokio::test]
async fn new_file_lines_and_unstaging_everything() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, "keep.txt", "k\n");
    commit_all(&repo, "init").await;
    write(&repo, "new.txt", "a\nb\nc\n");

    set(&repo, "new.txt", &[(0, 0), (0, 2)], true).await;
    assert_eq!(index_content(&repo, "new.txt").await.unwrap(), b"a\nc\n");
    let (f, _) = file(&repo, "new.txt").await;
    assert_eq!(
        (f.kind, f.staged),
        (FileStatusKind::Added, StagedState::Partial)
    );

    // Unstaging every line takes the file out of the index again.
    set(&repo, "new.txt", &[(0, 0), (0, 2)], false).await;
    assert_eq!(index_content(&repo, "new.txt").await, None);
    let (f, _) = file(&repo, "new.txt").await;
    assert_eq!(f.kind, FileStatusKind::Untracked);
}

#[tokio::test]
async fn works_before_the_first_commit() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, "first.txt", "x\ny\n");
    set(&repo, "first.txt", &[(0, 1)], true).await;
    assert_eq!(index_content(&repo, "first.txt").await.unwrap(), b"y\n");
}

#[tokio::test]
async fn deleted_file_partly_then_fully() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, "gone.txt", "a\nb\n");
    commit_all(&repo, "init").await;
    std::fs::remove_file(repo.join("gone.txt")).unwrap();

    set(&repo, "gone.txt", &[(0, 0)], true).await;
    assert_eq!(index_content(&repo, "gone.txt").await.unwrap(), b"b\n");
    set(&repo, "gone.txt", &[(0, 1)], true).await;
    assert_eq!(
        index_content(&repo, "gone.txt").await,
        None,
        "whole deletion staged"
    );
    let (f, _) = file(&repo, "gone.txt").await;
    assert_eq!(
        (f.kind, f.staged),
        (FileStatusKind::Deleted, StagedState::Full)
    );
}

#[tokio::test]
async fn keeps_crlf_line_endings() {
    let (_tmp, repo) = init_repo().await;
    git_in(&repo, &["config", "core.autocrlf", "false"]).await;
    write(&repo, "win.txt", "a\r\nb\r\nc\r\n");
    commit_all(&repo, "init").await;
    write(&repo, "win.txt", "a\r\nB\r\nc\r\nd\r\n");

    // " a", "-b", "+B", " c", "+d": stage only "+d".
    set(&repo, "win.txt", &[(0, 4)], true).await;
    assert_eq!(
        index_content(&repo, "win.txt").await.unwrap(),
        b"a\r\nb\r\nc\r\nd\r\n"
    );
}

#[tokio::test]
async fn rejects_stale_tokens_and_bad_lines() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, "f.txt", "a\n");
    commit_all(&repo, "init").await;
    write(&repo, "f.txt", "a\nb\n");
    let token = staging(&repo, "f.txt").await.token;

    // Context line and out-of-range references.
    assert!(matches!(
        try_set(&repo, "f.txt", &[(0, 0)], true).await,
        Err(LineStageError::BadLine)
    ));
    assert!(matches!(
        try_set(&repo, "f.txt", &[(3, 0)], true).await,
        Err(LineStageError::BadLine)
    ));

    write(&repo, "f.txt", "a\nb\nc\n");
    let (f, has_head) = file(&repo, "f.txt").await;
    let refs = [LineRef { hunk: 0, line: 1 }];
    assert!(matches!(
        set_lines_staged(&git(), &repo, &f, has_head, &token, &refs, true).await,
        Err(LineStageError::Changed)
    ));
    assert_eq!(index_content(&repo, "f.txt").await.unwrap(), b"a\n");
}

#[tokio::test]
async fn filtered_files_are_whole_file_only() {
    let (_tmp, repo) = init_repo().await;
    write(&repo, ".gitattributes", "*.dat filter=custom\n");
    write(&repo, "x.dat", "a\n");
    write(&repo, "x.txt", "a\n");
    commit_all(&repo, "init").await;
    write(&repo, "x.dat", "a\nb\n");
    write(&repo, "x.txt", "a\nb\n");

    let (dat, has_head) = file(&repo, "x.dat").await;
    let w = working_diff(&git(), &repo, &dat, has_head).await.unwrap();
    assert!(matches!(w.diff, FileDiff::Text { .. }));
    assert_eq!(w.lines, None);
    let (txt, _) = file(&repo, "x.txt").await;
    assert!(working_diff(&git(), &repo, &txt, has_head)
        .await
        .unwrap()
        .lines
        .is_some());
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_are_whole_file_only() {
    let (_tmp, repo) = init_repo().await;
    std::os::unix::fs::symlink("target.txt", repo.join("link")).unwrap();
    let (f, has_head) = file(&repo, "link").await;
    assert_eq!(
        working_diff(&git(), &repo, &f, has_head)
            .await
            .unwrap()
            .lines,
        None
    );
}

#[test]
fn attribute_output() {
    assert!(attrs_allow_line_staging(
        b"a\0filter\0unspecified\0a\0working-tree-encoding\0unset\0"
    ));
    assert!(!attrs_allow_line_staging(
        b"a\0filter\0lfs\0a\0working-tree-encoding\0unspecified\0"
    ));
}

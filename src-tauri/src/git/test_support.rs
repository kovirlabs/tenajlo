//! Helpers for tests that need real repositories.

use std::path::{Path, PathBuf};

use super::binary::resolve;
use super::exec::{Access, GitCommand};

/// Runs git in `dir` with a fixed identity, panicking on failure.
pub async fn git_in(dir: &Path, args: &[&str]) -> String {
    let mut full = vec![
        "-c",
        "user.name=Test",
        "-c",
        "user.email=test@example.com",
        "-c",
        "commit.gpgsign=false",
    ];
    full.extend_from_slice(args);
    let out = GitCommand::new(full, Access::Mutating)
        .cwd(dir)
        .run(&resolve(None, None).unwrap())
        .await
        .unwrap_or_else(|e| panic!("git {args:?} failed: {e:?}"));
    String::from_utf8(out.stdout).unwrap()
}

/// Creates an empty repository on branch `main`. Keep the `TempDir` alive.
pub async fn init_repo() -> (tempfile::TempDir, PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("repo");
    std::fs::create_dir(&repo).unwrap();
    git_in(&repo, &["init", "-q", "-b", "main"]).await;
    (tmp, repo)
}

/// Writes `contents` to `rel` inside `repo`, creating parent dirs.
pub fn write(repo: &Path, rel: &str, contents: &str) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

//! Helpers for tests that need real repositories.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::binary::resolve;
use super::exec::{Access, GitBinary, GitCommand};

/// The git on PATH, resolved once per test run.
pub fn git() -> GitBinary {
    static GIT: OnceLock<GitBinary> = OnceLock::new();
    GIT.get_or_init(|| resolve(None, None).expect("git on PATH for tests"))
        .clone()
}

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
        .run(&git())
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

/// Stages everything and commits it with `message`.
pub async fn commit_all(repo: &Path, message: &str) {
    git_in(repo, &["add", "-A"]).await;
    git_in(repo, &["commit", "-q", "-m", message]).await;
}

/// Adds an empty bare repository (default branch `main`) next to `repo` as its `origin`.
/// Returns the bare repository's path.
pub async fn with_bare_origin(repo: &Path) -> PathBuf {
    let parent = repo.parent().expect("repo has a parent dir");
    let origin = parent.join("origin.git");
    let origin_str = origin.to_str().expect("utf-8 temp path");
    git_in(parent, &["init", "-q", "--bare", "-b", "main", origin_str]).await;
    git_in(repo, &["remote", "add", "origin", origin_str]).await;
    origin
}

/// Clones `origin` into a sibling folder named `other`, to act as a teammate.
pub async fn teammate_clone(origin: &Path) -> PathBuf {
    let parent = origin.parent().expect("origin has a parent dir");
    let other = parent.join("other");
    let args = [
        "clone",
        "-q",
        origin.to_str().expect("utf-8 temp path"),
        other.to_str().expect("utf-8 temp path"),
    ];
    git_in(parent, &args).await;
    other
}

/// Writes `contents` to `rel` inside `repo`, creating parent dirs.
pub fn write(repo: &Path, rel: &str, contents: &str) {
    let path = repo.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, contents).unwrap();
}

/// Reads a checked-out text file with `\r\n` folded to `\n`: Git for Windows checks files
/// out with CRLF (`core.autocrlf=true` in its system config).
pub fn read_text(path: &Path) -> String {
    std::fs::read_to_string(path)
        .unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
        .replace("\r\n", "\n")
}

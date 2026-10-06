//! Git executable resolution (spec §5.1).
//!
//! 1. A user-configured override (Settings → git binary), if set.
//! 2. Windows: the bundled MinGit (`<resources>/mingit/cmd/git.exe`).
//! 3. Linux/macOS: `git` from `PATH`. On Windows, `PATH` is only consulted in
//!    debug builds so development works before MinGit is bundled.

use std::path::{Path, PathBuf};

use super::error::GitError;
use super::exec::GitBinary;

#[cfg(windows)]
const GIT_EXE: &str = "git.exe";
#[cfg(not(windows))]
const GIT_EXE: &str = "git";

/// Resolves the git executable. `bundled_dir` is the MinGit root, if any.
pub fn resolve(
    override_path: Option<&Path>,
    bundled_dir: Option<&Path>,
) -> Result<GitBinary, GitError> {
    let mut searched = Vec::new();

    if let Some(path) = override_path {
        if path.is_file() {
            return Ok(GitBinary::new(path.to_path_buf()));
        }
        // An explicit override that doesn't exist is an error, not a silent fallback.
        return Err(GitError::NotFound {
            searched: vec![path.to_path_buf()],
        });
    }

    if cfg!(windows) {
        if let Some(dir) = bundled_dir {
            let candidate = dir.join("cmd").join(GIT_EXE);
            if candidate.is_file() {
                return Ok(GitBinary::new(candidate));
            }
            searched.push(candidate);
        }
    }

    if !cfg!(windows) || cfg!(debug_assertions) {
        if let Some(found) = search_path(&mut searched) {
            if cfg!(windows) {
                tracing::warn!("bundled MinGit not found; using git from PATH (debug build only)");
            }
            return Ok(GitBinary::new(found));
        }
    }

    Err(GitError::NotFound { searched })
}

/// Finds `git` on `PATH`, recording each candidate checked.
fn search_path(searched: &mut Vec<PathBuf>) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        // Relative PATH entries would resolve against an arbitrary cwd; skip them.
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(GIT_EXE);
        if candidate.is_file() {
            return Some(candidate);
        }
        searched.push(candidate);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_git_on_path() {
        let git = resolve(None, None).expect("git on PATH");
        assert!(git.path().is_absolute());
    }

    #[test]
    fn missing_override_is_an_error() {
        let bogus = Path::new("/definitely/not/here/git");
        match resolve(Some(bogus), None) {
            Err(GitError::NotFound { searched }) => assert_eq!(searched, vec![bogus.to_path_buf()]),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn existing_override_wins() {
        let real = resolve(None, None).unwrap();
        let got = resolve(Some(real.path()), Some(Path::new("/nope"))).unwrap();
        assert_eq!(got, real);
    }
}

//! Branch-name sanitizing and validation (CLAUDE.md rule 1, spec §5.2).

use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

/// Turns free text into a plausible branch name: `"Fix pump: alarms"` → `"Fix-pump-alarms"`.
/// The result still needs [`validate`]. Mirrors GitHub Desktop's sanitizedRefName.
pub fn sanitize(input: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for c in input.trim().chars() {
        let bad = c.is_whitespace()
            || c.is_control()
            || matches!(c, '~' | '^' | ':' | '?' | '*' | '[' | '\\');
        if bad {
            pending_dash = true;
            continue;
        }
        if pending_dash && !out.is_empty() {
            out.push('-');
        }
        pending_dash = false;
        out.push(c);
    }
    let mut s = out
        .replace("..", ".")
        .replace("@{", "@-")
        .replace("//", "/");
    loop {
        let before = s.len();
        s = s.trim_start_matches(['-', '.', '/']).to_owned();
        s = s.trim_end_matches(['.', '/']).to_owned();
        if let Some(stripped) = s.strip_suffix(".lock") {
            s = stripped.to_owned();
        }
        if s.len() == before {
            return s;
        }
    }
}

/// Validates `name` as a new or existing local branch name. Never lets git expand
/// `@{-N}` shorthands, and rejects names that could be read as options.
pub async fn validate(git: &GitBinary, root: &Path, name: &str) -> Result<(), GitError> {
    if name.is_empty()
        || name.starts_with('-')
        || name.contains("@{")
        || name == "@"
        || name == "HEAD"
    {
        return Err(GitError::InvalidRefName(name.to_owned()));
    }
    let out = GitCommand::new(["check-ref-format", "--branch", name], Access::ReadOnly)
        .ok_exit_codes(&[0, 128])
        .cwd(root)
        .run(git)
        .await?;
    // `--branch` prints the normalized name; anything else means git rewrote it.
    if out.exit_code != Some(0)
        || String::from_utf8_lossy(&out.stdout).trim_end_matches('\n') != name
    {
        return Err(GitError::InvalidRefName(name.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{git, init_repo};

    #[test]
    fn sanitizes() {
        assert_eq!(sanitize("Fix pump: alarms"), "Fix-pump-alarms");
        assert_eq!(sanitize("  -..feature//x.lock "), "feature/x");
        assert_eq!(sanitize("a..b@{1}"), "a.b@-1}");
        assert_eq!(sanitize("ünï cödé"), "ünï-cödé");
        assert_eq!(sanitize("???"), "");
    }

    #[tokio::test]
    async fn validates() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        for ok in ["main", "feature/ü-x", "fix-123"] {
            assert!(validate(&git, &repo, ok).await.is_ok(), "{ok}");
        }
        for bad in [
            "", "-x", "@{-1}", "a b", "x..y", "x.lock", "HEAD", "a~1", "/x",
        ] {
            assert!(validate(&git, &repo, bad).await.is_err(), "{bad}");
        }
    }
}

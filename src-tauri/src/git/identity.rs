//! The user's git identity (`user.name` / `user.email`).

use std::path::Path;

use serde::Serialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

/// Effective identity for a repository (local, then global config).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
pub struct Identity {
    pub name: Option<String>,
    pub email: Option<String>,
}

/// Reads the identity git would use in `root`.
pub async fn identity(git: &GitBinary, root: &Path) -> Result<Identity, GitError> {
    read(git, Some(root)).await
}

/// Reads the identity from the user's global git config (Settings).
pub async fn global(git: &GitBinary) -> Result<Identity, GitError> {
    read(git, None).await
}

/// `root`'s effective config, or the global config when `root` is `None`.
async fn read(git: &GitBinary, root: Option<&Path>) -> Result<Identity, GitError> {
    Ok(Identity {
        name: get(git, root, "user.name").await?,
        email: get(git, root, "user.email").await?,
    })
}

async fn get(git: &GitBinary, root: Option<&Path>, key: &str) -> Result<Option<String>, GitError> {
    let mut args = vec!["config"];
    if root.is_none() {
        args.push("--global");
    }
    args.extend(["--get", key]);
    // `config --get` exits 1 when the key is unset.
    let mut cmd = GitCommand::new(args, Access::ReadOnly).ok_exit_codes(&[0, 1]);
    if let Some(root) = root {
        cmd = cmd.cwd(root);
    }
    let out = cmd.run(git).await?;
    let value = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok((!value.is_empty()).then_some(value))
}

/// Validates an identity field: non-empty, single line, reasonable length.
pub fn valid_field(value: &str) -> bool {
    let v = value.trim();
    !v.is_empty() && v.chars().count() <= 200 && !v.chars().any(char::is_control)
}

/// Writes `user.name` and `user.email` to the user's **global** git config.
///
/// This is the only global config Tenajlo ever writes (CLAUDE.md rule 5), and only after the
/// user explicitly confirms in the UI. Callers must validate with [`valid_field`].
pub async fn set_global(git: &GitBinary, name: &str, email: &str) -> Result<(), GitError> {
    for (key, value) in [("user.name", name.trim()), ("user.email", email.trim())] {
        GitCommand::new(["config", "--global", "--", key, value], Access::Mutating)
            .run(git)
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::test_support::{git, git_in, init_repo};

    #[tokio::test]
    async fn reads_local_identity() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        git_in(&repo, &["config", "user.name", "Ëvan"]).await;
        git_in(&repo, &["config", "user.email", ""]).await;
        let id = identity(&git, &repo).await.unwrap();
        assert_eq!(id.name.as_deref(), Some("Ëvan"));
        assert_eq!(id.email, None);
    }

    #[test]
    fn validates_fields() {
        assert!(valid_field("Evan Gress"));
        assert!(!valid_field("   "));
        assert!(!valid_field("a\nb"));
        assert!(!valid_field(&"x".repeat(201)));
    }
}

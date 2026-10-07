//! `git --version` detection and minimum-version enforcement (spec §5.1).

use serde::Serialize;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::version::{parse_version, GitVersion};

/// Result of the startup git check, sent to the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    /// Absolute path of the git executable in use.
    pub path: String,
    pub version: GitVersion,
    pub minimum: GitVersion,
    /// `version >= minimum`.
    pub supported: bool,
    /// `git lfs version` output (e.g. `git-lfs/3.7.1 (…)`), or `None` if not installed.
    pub lfs_version: Option<String>,
}

/// Runs `git --version` and compares it against [`GitVersion::MINIMUM`].
pub async fn detect(git: &GitBinary) -> Result<GitInfo, GitError> {
    let out = GitCommand::new(["--version"], Access::ReadOnly)
        .run(git)
        .await?;
    let text = String::from_utf8_lossy(&out.stdout);
    let version = parse_version(&text).ok_or_else(|| GitError::Parse(text.trim().to_owned()))?;
    Ok(GitInfo {
        path: git.path().display().to_string(),
        version,
        minimum: GitVersion::MINIMUM,
        supported: version >= GitVersion::MINIMUM,
        lfs_version: lfs_version(git).await,
    })
}

/// `git lfs version`; `None` when git-lfs isn't installed ("'lfs' is not a git command").
async fn lfs_version(git: &GitBinary) -> Option<String> {
    let out = GitCommand::new(["lfs", "version"], Access::ReadOnly)
        .run(git)
        .await
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    text.starts_with("git-lfs/").then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;

    #[tokio::test]
    async fn detects_installed_git() {
        let info = detect(&resolve(None, None).unwrap()).await.unwrap();
        // CLAUDE.md: tests need git >= 2.40 on PATH.
        assert!(
            info.supported,
            "test machine git {} is too old",
            info.version
        );
        assert_eq!(info.minimum, GitVersion::MINIMUM);
        // Tests run with git-lfs installed (CLAUDE.md).
        assert!(info.lfs_version.is_some_and(|v| v.starts_with("git-lfs/")));
    }
}

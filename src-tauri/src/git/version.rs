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
    })
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
    }
}

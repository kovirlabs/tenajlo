//! Git LFS (spec §13.1: large CAD / PLC files). git-lfs is bundled on Windows; elsewhere the
//! user installs it. Filters are passed per invocation (see `GitBinary::with_lfs`).

use std::path::Path;

use serde::Serialize;

use super::error::GitError;
use super::exec::GitBinary;
use super::remote::{run_remote, RemoteRun};

/// Whether a repository needs LFS and whether this computer has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsStatus {
    /// `.gitattributes` routes files through the LFS filter, or `.lfsconfig` exists.
    pub used: bool,
    pub installed: bool,
}

impl LfsStatus {
    /// LFS files would be committed or pushed wrongly: block those operations.
    pub fn missing(self) -> bool {
        self.used && !self.installed
    }
}

/// Checks the repository's root `.gitattributes` and `.lfsconfig`.
pub fn status(git: &GitBinary, root: &Path) -> LfsStatus {
    let attributes = std::fs::read_to_string(root.join(".gitattributes")).unwrap_or_default();
    let used = attributes
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .any(|l| l.split_whitespace().any(|attr| attr == "filter=lfs"))
        || root.join(".lfsconfig").is_file();
    LfsStatus {
        used,
        installed: git.has_lfs(),
    }
}

/// Uploads the LFS objects `branch` needs to `remote` (`git lfs push`), so a push never
/// depends on the repository's pre-push hook being installed. Names must be validated.
pub async fn push_objects(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    branch: &str,
    r: RemoteRun,
) -> Result<(), GitError> {
    run_remote(
        git,
        root,
        vec![
            "lfs".into(),
            "push".into(),
            "--".into(),
            remote.into(),
            branch.into(),
        ],
        r,
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::progress::parse_progress;
    use crate::git::test_support::git;

    #[test]
    fn detects_lfs_attributes() {
        let dir = tempfile::tempdir().unwrap();
        let git = git();
        assert!(!status(&git, dir.path()).used);
        std::fs::write(
            dir.path().join(".gitattributes"),
            "# *.bin filter=lfs\n*.txt text\n",
        )
        .unwrap();
        assert!(!status(&git, dir.path()).used, "comments don't count");
        std::fs::write(
            dir.path().join(".gitattributes"),
            "*.SLDPRT filter=lfs diff=lfs merge=lfs -text\r\n",
        )
        .unwrap();
        let st = status(&git, dir.path());
        assert!(st.used && !st.installed && st.missing());
        assert!(!status(&git.with_lfs(true), dir.path()).missing());
    }

    #[test]
    fn lfs_progress_lines_parse() {
        let p = parse_progress("Downloading LFS objects:  45% (9/20), 1.2 MB | 3.0 MB/s").unwrap();
        assert_eq!(
            (p.phase.as_str(), p.percent),
            ("Downloading LFS objects", Some(45))
        );
        let p = parse_progress("Uploading LFS objects: 100% (1/1), 2.1 MB | 0 B/s, done.").unwrap();
        assert_eq!(
            (p.phase.as_str(), p.percent),
            ("Uploading LFS objects", Some(100))
        );
        let p = parse_progress("Filtering content:  50% (1/2), 5.00 MiB | 3.00 MiB/s").unwrap();
        assert_eq!(p.phase, "Filtering content");
    }
}

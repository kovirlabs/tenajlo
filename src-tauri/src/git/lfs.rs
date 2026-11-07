//! Git LFS (spec §13.1: large CAD / PLC files). git-lfs is bundled on Windows; elsewhere the
//! user installs it. Filters are passed per invocation (see `GitBinary::with_lfs`).

use std::path::Path;

use serde::Serialize;

use super::error::{GitError, GitErrorKind};
use super::exec::{Access, GitBinary, GitCommand};
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

/// Uploads `branch`'s LFS objects with lock verification whenever the server might support
/// locks. `supported` is what is known about the server's lock API (`None`: not asked yet).
///
/// Without verification git-lfs only warns about files someone else has locked and pushes
/// anyway, so an unknown server is verified too. If it turns out not to have the lock API,
/// the push runs again without verification. Returns the result and what is now known.
pub async fn push_objects_verified(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    branch: &str,
    supported: Option<bool>,
    make_run: impl Fn() -> RemoteRun,
) -> (Result<(), GitError>, Option<bool>) {
    if supported == Some(false) {
        let res = push_objects(git, root, remote, branch, false, make_run()).await;
        return (res, supported);
    }
    match push_objects(git, root, remote, branch, true, make_run()).await {
        Err(GitError::Failed {
            kind: GitErrorKind::LfsLocksUnsupported,
            ..
        }) => {
            let res = push_objects(git, root, remote, branch, false, make_run()).await;
            (res, Some(false))
        }
        res => (res, supported),
    }
}

/// Uploads the LFS objects `branch` needs to `remote` (`git lfs push`), so a push never
/// depends on the repository's pre-push hook being installed. Names must be validated.
///
/// `verify_locks` (the server supports locks): refuse when the push changes a file someone
/// else has locked. Without it git-lfs only warns. Set per invocation, never in config.
pub async fn push_objects(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    branch: &str,
    verify_locks: bool,
    mut r: RemoteRun,
) -> Result<(), GitError> {
    if verify_locks {
        // git-lfs prefers `lfs.<url>.locksverify` (which it sets to false itself after a
        // server error) to the plain key, so override those too.
        for key in url_locksverify_keys(git, root).await? {
            r.config.push(format!("{key}=true"));
        }
        r.config.push("lfs.locksverify=true".into());
    }
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

/// The repository's URL-scoped `lfs.<url>.locksverify` keys, as git names them.
async fn url_locksverify_keys(git: &GitBinary, root: &Path) -> Result<Vec<String>, GitError> {
    // Exit 1: no such keys.
    let out = GitCommand::new(
        [
            "config",
            "--name-only",
            "--get-regexp",
            r"^lfs\..+\.locksverify$",
        ],
        Access::ReadOnly,
    )
    .ok_exit_codes(&[0, 1])
    .cwd(root)
    .run(git)
    .await?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .map(str::to_owned)
        .collect())
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

//! Git LFS file locks (spec §8.1, v1.4): list, lock and unlock through `git lfs`, so the
//! server's Git LFS locking API is reached with the same credentials as a push.

use std::path::{Component, Path};
use std::time::Duration;

use super::error::GitError;
use super::exec::{Access, GitBinary};
use super::lfs;
use super::parse::lfs_locks::{
    merge_verified, parse_locks, parse_new_lock, parse_unlock, parse_verified_locks, LfsLock,
};
use super::primary_remote::primary_remote;
use super::remote::{run_remote_output, RemoteRun};

/// Lock calls are small API requests; a hung server shouldn't hold the UI for an hour.
const LOCK_TIMEOUT: Duration = Duration::from_secs(60);

/// The remote locks live on (the one fetch uses) and its URL, or `None` when locks don't
/// apply: no Git LFS, git-lfs missing, no remote, or a local-path remote (no LFS API).
pub async fn lock_remote(
    git: &GitBinary,
    root: &Path,
) -> Result<Option<(String, String)>, GitError> {
    let lfs = lfs::status(git, root);
    if !lfs.used || !lfs.installed {
        return Ok(None);
    }
    let Some((name, url)) = primary_remote(git, root).await? else {
        return Ok(None);
    };
    Ok(is_networked(&url).then_some((name, url)))
}

/// An http(s) or ssh URL, or scp-like `user@host:path`; not a local path or `file://`.
fn is_networked(url: &str) -> bool {
    if let Some((scheme, _)) = url.split_once("://") {
        return scheme != "file";
    }
    match url.split_once(':') {
        // Not a Windows drive (`C:\repo`) or a relative path with a colon.
        Some((host, _)) => host.len() > 1 && !host.contains('/') && !host.contains('\\'),
        None => false,
    }
}

/// Every lock on `remote`'s server for this repository, marked ours or theirs. Doesn't change
/// the repository, so it runs without the mutation lock. `remote` must be validated.
///
/// Two requests: the plain listing has every lock, and `--verify` says which are ours. The
/// verify endpoint needs write access and shows a read-only user nothing, so it can't be
/// the list on its own. `make_run` gives each request its own run (same prompts and cancel).
pub async fn list(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    make_run: impl Fn() -> RemoteRun,
) -> Result<Vec<LfsLock>, GitError> {
    let listing = |verify: bool| {
        let mut args: Vec<String> = vec!["lfs".into(), "locks".into()];
        if verify {
            args.push("--verify".into());
        }
        args.push("--json".into());
        args.push(format!("--remote={remote}"));
        run_remote_output(git, root, args, make_run(), Access::ReadOnly, LOCK_TIMEOUT)
    };
    let all = parse_locks(&listing(false).await?).map_err(GitError::Parse)?;
    let verified = parse_verified_locks(&listing(true).await?).map_err(GitError::Parse)?;
    Ok(merge_verified(all, verified))
}

/// Locks `path` (repository-relative, `/` separators) on `remote`'s server. git-lfs may make
/// a lockable file writable, so callers hold the mutation lock.
pub async fn lock(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    path: &str,
    r: RemoteRun,
) -> Result<LfsLock, GitError> {
    if !valid_path(path) {
        return Err(GitError::Parse(format!("not a repository path: {path}")));
    }
    let args = vec![
        "lfs".into(),
        "lock".into(),
        "--json".into(),
        format!("--remote={remote}"),
        "--".into(),
        path.into(),
    ];
    let out = run_remote_output(git, root, args, r, Access::Mutating, LOCK_TIMEOUT).await?;
    parse_new_lock(&out).map_err(GitError::Parse)
}

/// Removes the lock with server id `id` (by id, so a renamed or deleted file can still be
/// unlocked). `force` removes someone else's lock. Callers hold the mutation lock.
pub async fn unlock(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    id: &str,
    force: bool,
    r: RemoteRun,
) -> Result<(), GitError> {
    if !valid_lock_id(id) {
        return Err(GitError::Parse(format!("not a lock id: {id}")));
    }
    let mut args: Vec<String> = vec![
        "lfs".into(),
        "unlock".into(),
        "--json".into(),
        format!("--remote={remote}"),
        format!("--id={id}"),
    ];
    if force {
        args.push("--force".into());
    }
    let out = run_remote_output(git, root, args, r, Access::Mutating, LOCK_TIMEOUT).await?;
    parse_unlock(&out).map_err(|reason| GitError::failed(&reason, b"", None))
}

/// A repository-relative path: not empty or absolute, no `.`/`..`, no leading `-`.
pub fn valid_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('-')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

/// Lock ids are short server-generated tokens (digits on Forgejo, UUIDs elsewhere).
pub fn valid_lock_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && !id.starts_with('-')
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// `file` (an absolute path from a picker) relative to `root` with `/` separators, or `None`
/// if it isn't a file inside the repository (or is inside `.git`).
pub fn repo_relative(root: &Path, file: &Path) -> Option<String> {
    let root = dunce::canonicalize(root).ok()?;
    let file = dunce::canonicalize(file).ok()?;
    if !file.is_file() {
        return None;
    }
    let rel = file.strip_prefix(&root).ok()?;
    let parts: Vec<&str> = rel
        .components()
        .map(|c| match c {
            Component::Normal(s) => s.to_str(),
            _ => None,
        })
        .collect::<Option<_>>()?;
    if parts.is_empty() || parts[0] == ".git" {
        return None;
    }
    Some(parts.join("/"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_must_stay_inside_the_repository() {
        for ok in ["a.bin", "CAD/Pumpe Gehäuse.SLDPRT", "plc/x-1.L5X"] {
            assert!(valid_path(ok), "{ok}");
        }
        for bad in ["", "-x", "../a", "/etc/passwd", "a/../../b", "./a"] {
            assert!(!valid_path(bad), "{bad}");
        }
        assert!(valid_lock_id("12") && !valid_lock_id("") && !valid_lock_id("--force"));
    }

    #[test]
    fn only_network_remotes_have_locks() {
        for yes in [
            "https://git.example.com/team/pump.git",
            "ssh://git@git.example.com:2222/team/pump.git",
            "git@git.example.com:team/pump.git",
        ] {
            assert!(is_networked(yes), "{yes}");
        }
        for no in [
            "C:\\src\\pump",
            "/srv/git/pump.git",
            "./a:b",
            "file:///srv/pump.git",
            "pump",
        ] {
            assert!(!is_networked(no), "{no}");
        }
    }

    #[test]
    fn picked_files_become_repo_relative() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("CAD/sub dir")).unwrap();
        std::fs::create_dir_all(root.join(".git")).unwrap();
        std::fs::write(root.join("CAD/sub dir/Gehäuse.SLDPRT"), b"x").unwrap();
        std::fs::write(root.join(".git/config"), b"x").unwrap();
        assert_eq!(
            repo_relative(root, &root.join("CAD/sub dir/Gehäuse.SLDPRT")).as_deref(),
            Some("CAD/sub dir/Gehäuse.SLDPRT")
        );
        assert_eq!(repo_relative(root, &root.join(".git/config")), None);
        assert_eq!(repo_relative(root, &root.join("CAD")), None, "folders");
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("x"), b"x").unwrap();
        assert_eq!(repo_relative(root, &other.path().join("x")), None);
    }
}

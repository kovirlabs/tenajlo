//! `git clone` (spec §5.3), plus the URL and folder rules for the clone dialog.

use std::path::{Path, PathBuf};

use reqwest::Url;

use super::error::GitError;
use super::exec::GitBinary;
use super::remote::{run_remote, RemoteRun};
use crate::forgejo::address::is_loopback;

/// Why a clone couldn't start or finish.
#[derive(Debug, thiserror::Error)]
pub enum CloneError {
    #[error("unsupported clone URL: {0}")]
    InvalidUrl(&'static str),
    #[error("{} already exists", .0.display())]
    DestinationExists(PathBuf),
    #[error("invalid destination: {0}")]
    InvalidDestination(&'static str),
    #[error("could not create {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Git(#[from] GitError),
}

/// Accepts `https://` URLs (and `http://` for loopback test servers). SSH arrives in M5.
/// Rejects embedded passwords: git would save them in `.git/config`.
pub fn validate_url(url: &str) -> Result<(), CloneError> {
    let url = url.trim();
    if url.starts_with('-') {
        return Err(CloneError::InvalidUrl("starts with -"));
    }
    let parsed = Url::parse(url).map_err(|_| CloneError::InvalidUrl("not a URL"))?;
    match parsed.scheme() {
        "https" => {}
        "http" if is_loopback(&parsed) => {}
        "http" => return Err(CloneError::InvalidUrl("plain http")),
        _ => return Err(CloneError::InvalidUrl("not https")),
    }
    if parsed.host_str().is_none_or(str::is_empty) {
        return Err(CloneError::InvalidUrl("no host"));
    }
    if parsed.password().is_some() {
        return Err(CloneError::InvalidUrl("contains a password"));
    }
    Ok(())
}

/// Folder name for a clone of `url`: its last path segment without `.git`.
/// `None` if that isn't a usable folder name on every platform.
pub fn folder_name(url: &str) -> Option<String> {
    let parsed = Url::parse(url.trim()).ok()?;
    let last = parsed.path_segments()?.rev().find(|s| !s.is_empty())?;
    let name = last.strip_suffix(".git").unwrap_or(last);
    let invalid = |c: char| c.is_control() || r#"<>:"/\|?*%"#.contains(c);
    let usable = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.ends_with(['.', ' '])
        && !name.chars().any(invalid);
    usable.then(|| name.to_owned())
}

/// Clones `url` into `dest`, which must not exist yet (its parent is created if needed).
/// If the clone fails or is cancelled, the partly-written `dest` is removed: it was created
/// by this clone and holds nothing of the user's.
pub async fn clone(
    git: &GitBinary,
    url: &str,
    dest: &Path,
    r: RemoteRun,
) -> Result<(), CloneError> {
    if !dest.is_absolute() {
        return Err(CloneError::InvalidDestination("not an absolute path"));
    }
    let (Some(parent), Some(dest_str)) = (dest.parent(), dest.to_str()) else {
        return Err(CloneError::InvalidDestination("no parent folder"));
    };
    if dest.symlink_metadata().is_ok() {
        return Err(CloneError::DestinationExists(dest.to_owned()));
    }
    std::fs::create_dir_all(parent).map_err(|source| CloneError::Io {
        path: parent.to_owned(),
        source,
    })?;

    let args = vec![
        "clone".into(),
        "--progress".into(),
        "--".into(),
        url.to_owned(),
        dest_str.to_owned(),
    ];
    let result = run_remote(git, parent, args, r).await;
    if result.is_err() && dest.symlink_metadata().is_ok() {
        // git removes its own partial clone on a normal failure, but not when killed.
        if let Err(e) = std::fs::remove_dir_all(dest) {
            tracing::warn!(error = %e, "could not remove a partial clone");
        }
    }
    Ok(result?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::error::GitErrorKind;
    use crate::git::test_support::{git_in, init_repo, write};
    use std::sync::{Arc, Mutex};

    #[test]
    fn url_policy() {
        assert!(validate_url("https://tmc-git01.tmus.local/team/plc.git").is_ok());
        assert!(validate_url(" https://evan@h/x.git ").is_ok());
        assert!(validate_url("http://localhost:3000/x.git").is_ok());
        for bad in [
            "http://h/x.git",
            "https://evan:pat@h/x.git",
            "git@h:team/x.git",
            "ssh://git@h/x.git",
            "file:///tmp/x",
            "--upload-pack=evil",
            "https://",
            "",
        ] {
            assert!(validate_url(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn folder_names() {
        assert_eq!(
            folder_name("https://h/team/plc.git").as_deref(),
            Some("plc")
        );
        assert_eq!(
            folder_name("https://h/team/Ünïcode-repo/").as_deref(),
            None,
            "percent-encoded by the URL parser"
        );
        assert_eq!(
            folder_name("https://h/team/my.repo").as_deref(),
            Some("my.repo")
        );
        assert_eq!(folder_name("https://h/"), None);
        assert_eq!(folder_name("https://h/team/..").as_deref(), None);
        assert_eq!(folder_name("https://h/team/.git"), None);
    }

    /// A bare origin with one commit, cloned through `file://` so git reports progress.
    async fn origin() -> (tempfile::TempDir, String) {
        let (tmp, repo) = init_repo().await;
        write(&repo, "a.txt", "hello\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "first"]).await;
        let bare = tmp.path().join("origin.git");
        git_in(tmp.path(), &["clone", "-q", "--bare", "repo", "origin.git"]).await;
        let url = Url::from_file_path(&bare).unwrap().to_string();
        (tmp, url)
    }

    #[tokio::test]
    async fn clones_with_progress_into_new_folders() {
        let (tmp, url) = origin().await;
        let dest = tmp.path().join("Documents with space/Tenajlo/plc");
        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let run = RemoteRun {
            on_progress: Box::new(move |p| sink.lock().unwrap().push(p.phase)),
            ..RemoteRun::quiet()
        };
        clone(&resolve(None, None).unwrap(), &url, &dest, run)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(dest.join("a.txt")).unwrap(),
            "hello\n"
        );
        assert!(!seen.lock().unwrap().is_empty(), "progress reported");
    }

    #[tokio::test]
    async fn refuses_existing_destination() {
        let (tmp, url) = origin().await;
        let dest = tmp.path().join("taken");
        std::fs::create_dir(&dest).unwrap();
        let res = clone(
            &resolve(None, None).unwrap(),
            &url,
            &dest,
            RemoteRun::quiet(),
        )
        .await;
        assert!(matches!(res, Err(CloneError::DestinationExists(_))));
        assert!(dest.exists(), "left untouched");
    }

    #[tokio::test]
    async fn failed_clone_leaves_no_folder() {
        let (tmp, _) = origin().await;
        let missing = Url::from_file_path(tmp.path().join("nope.git"))
            .unwrap()
            .to_string();
        let dest = tmp.path().join("out");
        let res = clone(
            &resolve(None, None).unwrap(),
            &missing,
            &dest,
            RemoteRun::quiet(),
        )
        .await;
        assert!(
            matches!(
                res,
                Err(CloneError::Git(GitError::Failed {
                    kind: GitErrorKind::RepositoryNotFound,
                    ..
                }))
            ),
            "{res:?}"
        );
        assert!(!dest.exists());
    }
}

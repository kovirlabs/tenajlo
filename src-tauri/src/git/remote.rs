//! Fetch, pull, push and publish (spec §5.3). Callers hold the mutation lock.

use std::ffi::OsString;
use std::path::Path;
use std::time::{Duration, UNIX_EPOCH};

use tokio_util::sync::CancellationToken;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::progress::{parse_progress, Progress};

/// Network operations can be slow on VPN; cancellation is the user's escape hatch.
const REMOTE_TIMEOUT: Duration = Duration::from_secs(60 * 60);

/// Configured remote names.
pub async fn list_remotes(git: &GitBinary, root: &Path) -> Result<Vec<String>, GitError> {
    let out = GitCommand::new(["remote"], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::trim)
        .filter(|r| !r.is_empty())
        .map(str::to_owned)
        .collect())
}

/// URL of `remote` (its push URL with `push`), or `None` if it has none.
/// `remote` must be a configured remote name.
pub async fn remote_url(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    push: bool,
) -> Result<Option<String>, GitError> {
    let mut args = vec!["remote", "get-url"];
    if push {
        args.push("--push");
    }
    args.extend(["--", remote]);
    let out = GitCommand::new(args, Access::ReadOnly)
        .cwd(root)
        .ok_exit_codes(&[0, 2])
        .run(git)
        .await?;
    let url = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    Ok((!url.is_empty()).then_some(url))
}

/// Unix seconds of the last fetch, from FETCH_HEAD's mtime.
pub async fn last_fetched(git: &GitBinary, root: &Path) -> Result<Option<u32>, GitError> {
    let out = GitCommand::new(["rev-parse", "--git-path", "FETCH_HEAD"], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    let rel = String::from_utf8_lossy(&out.stdout)
        .trim_end_matches(['\n', '\r'])
        .to_owned();
    let path = root.join(rel);
    let secs = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .and_then(|d| u32::try_from(d.as_secs()).ok());
    Ok(secs)
}

/// How a remote operation reports back: auth env, cancellation, progress.
pub struct RemoteRun {
    /// Askpass trampoline environment for this operation.
    pub env: Vec<(OsString, OsString)>,
    /// Per-invocation `-c` flags (the account credential helper).
    pub config: Vec<String>,
    pub cancel: CancellationToken,
    pub on_progress: Box<dyn FnMut(Progress) + Send>,
}

impl RemoteRun {
    /// A run with no prompts or progress (tests).
    pub fn quiet() -> Self {
        Self {
            env: Vec::new(),
            config: Vec::new(),
            cancel: CancellationToken::new(),
            on_progress: Box::new(|_| {}),
        }
    }
}

/// Runs a network git command in `root` with progress, auth env/config and cancellation.
pub(super) async fn run_remote(
    git: &GitBinary,
    root: &Path,
    args: Vec<String>,
    r: RemoteRun,
) -> Result<(), GitError> {
    let RemoteRun {
        env,
        config,
        cancel,
        mut on_progress,
    } = r;
    let mut cmd = GitCommand::new(args, Access::Mutating)
        .cwd(root)
        .timeout(REMOTE_TIMEOUT)
        .cancel_on(cancel)
        .on_stderr_line(move |line| {
            if let Some(p) = parse_progress(line) {
                on_progress(p);
            }
        });
    for (k, v) in env {
        cmd = cmd.env(k, v);
    }
    for kv in config {
        cmd = cmd.config(kv);
    }
    cmd.run(git).await?;
    Ok(())
}

/// `git fetch --prune <remote>`. `remote` must be a configured remote name.
pub async fn fetch(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    r: RemoteRun,
) -> Result<(), GitError> {
    run_remote(
        git,
        root,
        vec![
            "fetch".into(),
            "--progress".into(),
            "--prune".into(),
            "--".into(),
            remote.into(),
        ],
        r,
    )
    .await
}

/// Fast-forward-only pull from the upstream. `--no-rebase` so `pull.rebase` config can't change behavior.
pub async fn pull(git: &GitBinary, root: &Path, r: RemoteRun) -> Result<(), GitError> {
    run_remote(
        git,
        root,
        vec![
            "pull".into(),
            "--progress".into(),
            "--ff-only".into(),
            "--no-rebase".into(),
        ],
        r,
    )
    .await
}

/// Pull that merges when the branches have diverged ("Merge the server's changes").
/// Conflicts fail with `MergeConflict` and leave the merge in progress for the banner.
pub async fn pull_merge(git: &GitBinary, root: &Path, mut r: RemoteRun) -> Result<(), GitError> {
    // Never open an editor for the merge message.
    r.env.push(("GIT_MERGE_AUTOEDIT".into(), "no".into()));
    run_remote(
        git,
        root,
        vec![
            "pull".into(),
            "--progress".into(),
            "--no-rebase".into(),
            "--no-edit".into(),
        ],
        r,
    )
    .await
}

/// Pull that rebases local commits onto the server's (Settings → Pull: rebase). A conflict
/// leaves the rebase in progress; Tenajlo only offers to abort it.
pub async fn pull_rebase(git: &GitBinary, root: &Path, r: RemoteRun) -> Result<(), GitError> {
    run_remote(
        git,
        root,
        vec!["pull".into(), "--progress".into(), "--rebase".into()],
        r,
    )
    .await
}

/// Pushes `local` to `remote_branch` on `remote`. With `set_upstream`, also starts tracking
/// it (publish). Names must be validated by the caller.
pub async fn push(
    git: &GitBinary,
    root: &Path,
    remote: &str,
    local: &str,
    remote_branch: &str,
    set_upstream: bool,
    r: RemoteRun,
) -> Result<(), GitError> {
    let mut args: Vec<String> = vec!["push".into(), "--progress".into()];
    if set_upstream {
        args.push("--set-upstream".into());
    }
    args.extend([
        "--".into(),
        remote.into(),
        format!("refs/heads/{local}:refs/heads/{remote_branch}"),
    ]);
    run_remote(git, root, args, r).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::error::GitErrorKind;
    use crate::git::status::status;
    use crate::git::test_support::{git_in, init_repo, write};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};

    /// A repo with a bare `origin` and a second clone (`other`) to simulate a teammate.
    async fn setup() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let (tmp, repo) = init_repo().await;
        let origin = tmp.path().join("origin.git");
        git_in(
            tmp.path(),
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        )
        .await;
        git_in(
            &repo,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        )
        .await;
        write(&repo, "a.txt", "1\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "init"]).await;
        let other = tmp.path().join("other");
        (tmp, repo, other)
    }

    #[tokio::test]
    async fn publish_push_fetch_pull_round_trip() {
        let (tmp, repo, other) = setup().await;
        let git = resolve(None, None).unwrap();

        push(
            &git,
            &repo,
            "origin",
            "main",
            "main",
            true,
            RemoteRun::quiet(),
        )
        .await
        .unwrap();
        let s = status(&git, &repo).await.unwrap();
        assert_eq!(
            s.branch.upstream.as_deref(),
            Some("origin/main"),
            "publish sets upstream"
        );
        assert!(last_fetched(&git, &repo).await.unwrap().is_none());

        // A teammate pushes; we fetch and see we're behind, then pull.
        git_in(
            tmp.path(),
            &[
                "clone",
                "-q",
                tmp.path().join("origin.git").to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        )
        .await;
        write(&other, "b.txt", "from teammate\n");
        git_in(&other, &["add", "-A"]).await;
        git_in(&other, &["commit", "-q", "-m", "teammate"]).await;
        git_in(&other, &["push", "-q"]).await;

        let seen = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let run = RemoteRun {
            on_progress: Box::new(move |p| sink.lock().unwrap().push(p.phase)),
            ..RemoteRun::quiet()
        };
        fetch(&git, &repo, "origin", run).await.unwrap();
        assert_eq!(status(&git, &repo).await.unwrap().branch.behind, 1);
        assert!(last_fetched(&git, &repo).await.unwrap().is_some());

        pull(&git, &repo, RemoteRun::quiet()).await.unwrap();
        assert!(repo.join("b.txt").exists());

        // Our new commit → push.
        write(&repo, "c.txt", "mine\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "mine"]).await;
        push(
            &git,
            &repo,
            "origin",
            "main",
            "main",
            false,
            RemoteRun::quiet(),
        )
        .await
        .unwrap();
        let s = status(&git, &repo).await.unwrap();
        assert_eq!((s.branch.ahead, s.branch.behind), (0, 0));
    }

    #[tokio::test]
    async fn diverged_pull_and_rejected_push_are_classified() {
        let (tmp, repo, other) = setup().await;
        let git = resolve(None, None).unwrap();
        push(
            &git,
            &repo,
            "origin",
            "main",
            "main",
            true,
            RemoteRun::quiet(),
        )
        .await
        .unwrap();
        git_in(
            tmp.path(),
            &[
                "clone",
                "-q",
                tmp.path().join("origin.git").to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        )
        .await;
        git_in(&other, &["commit", "-q", "--allow-empty", "-m", "theirs"]).await;
        git_in(&other, &["push", "-q"]).await;
        git_in(&repo, &["commit", "-q", "--allow-empty", "-m", "ours"]).await;

        match push(
            &git,
            &repo,
            "origin",
            "main",
            "main",
            false,
            RemoteRun::quiet(),
        )
        .await
        {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::PushRejected),
            other => panic!("unexpected {other:?}"),
        }
        fetch(&git, &repo, "origin", RemoteRun::quiet())
            .await
            .unwrap();
        match pull(&git, &repo, RemoteRun::quiet()).await {
            Err(GitError::Failed { kind, .. }) => assert_eq!(kind, GitErrorKind::PullDiverged),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn cancelled_fetch_reports_cancelled() {
        let (_tmp, repo, _) = setup().await;
        let git = resolve(None, None).unwrap();
        let run = RemoteRun::quiet();
        run.cancel.cancel();
        assert!(matches!(
            fetch(&git, &repo, "origin", run).await,
            Err(GitError::Cancelled)
        ));
    }

    /// `repo` and a teammate's clone both commit after `init`, editing `file`.
    async fn diverged(file_ours: &str, file_theirs: &str) -> (tempfile::TempDir, PathBuf) {
        let (tmp, repo, other) = setup().await;
        let git = resolve(None, None).unwrap();
        push(
            &git,
            &repo,
            "origin",
            "main",
            "main",
            true,
            RemoteRun::quiet(),
        )
        .await
        .unwrap();
        let origin = tmp.path().join("origin.git");
        git_in(
            tmp.path(),
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                other.to_str().unwrap(),
            ],
        )
        .await;
        write(&other, file_theirs, "theirs\n");
        git_in(&other, &["add", "-A"]).await;
        git_in(&other, &["commit", "-q", "-m", "theirs"]).await;
        git_in(&other, &["push", "-q"]).await;
        write(&repo, file_ours, "ours\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "ours"]).await;
        // pull_merge commits as the user; tests have no global identity.
        git_in(&repo, &["config", "user.name", "Test"]).await;
        git_in(&repo, &["config", "user.email", "t@example.com"]).await;
        (tmp, repo)
    }

    #[tokio::test]
    async fn diverged_pull_fails_ff_only_then_merges() {
        let (_tmp, repo) = diverged("mine.txt", "theirs.txt").await;
        let git = resolve(None, None).unwrap();
        let err = pull(&git, &repo, RemoteRun::quiet()).await.unwrap_err();
        assert!(
            matches!(
                err,
                GitError::Failed {
                    kind: GitErrorKind::PullDiverged,
                    ..
                }
            ),
            "{err:?}"
        );
        pull_merge(&git, &repo, RemoteRun::quiet()).await.unwrap();
        assert!(repo.join("theirs.txt").exists() && repo.join("mine.txt").exists());
        let st = status(&git, &repo).await.unwrap();
        assert_eq!(
            (st.branch.ahead, st.branch.behind),
            (2, 0),
            "ours + merge commit"
        );
    }

    #[tokio::test]
    async fn conflicting_merge_is_left_in_progress() {
        let (_tmp, repo) = diverged("a.txt", "a.txt").await;
        let git = resolve(None, None).unwrap();
        let err = pull_merge(&git, &repo, RemoteRun::quiet())
            .await
            .unwrap_err();
        assert!(
            matches!(
                err,
                GitError::Failed {
                    kind: GitErrorKind::MergeConflict,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(status(&git, &repo).await.unwrap().has_conflicts);
    }

    #[tokio::test]
    async fn rebase_pull_replays_local_commits() {
        let (_tmp, repo) = diverged("mine.txt", "theirs.txt").await;
        let git = resolve(None, None).unwrap();
        pull_rebase(&git, &repo, RemoteRun::quiet()).await.unwrap();
        let st = status(&git, &repo).await.unwrap();
        assert_eq!(
            (st.branch.ahead, st.branch.behind),
            (1, 0),
            "no merge commit"
        );
        assert!(repo.join("theirs.txt").exists());
    }
}

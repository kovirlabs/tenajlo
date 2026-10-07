//! The only place in Tenajlo that spawns git (CLAUDE.md rule 1, spec §5.2).
//!
//! Every invocation gets argument-array spawning (never a shell), the base
//! environment, per-invocation `-c` flags, a timeout, and optional cancellation.
//! The child is killed if the future is dropped, times out, or is cancelled.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::error::{classify, GitError};
use super::process_tree::TreeKiller;
use crate::redact::redact;

/// Default timeout for short, local operations.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

/// Path to a resolved git executable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitBinary(PathBuf);

impl GitBinary {
    /// Wraps an already-resolved git path. Use `git::binary::resolve` to find one.
    pub fn new(path: PathBuf) -> Self {
        Self(path)
    }

    /// Filesystem path of the executable.
    pub fn path(&self) -> &Path {
        &self.0
    }
}

/// Whether an invocation may modify the repository.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Background reads (status, log). Sets `GIT_OPTIONAL_LOCKS=0`.
    ReadOnly,
    /// Must be run while holding the repo's mutation lock (`RepoManager`).
    Mutating,
}

/// Captured output of a successful git invocation.
#[derive(Debug)]
pub struct GitOutput {
    pub stdout: Vec<u8>,
    /// Raw stderr. Callers must pass it through `redact` before logging.
    pub stderr: String,
    pub exit_code: Option<i32>,
}

/// A single git invocation. Build with [`GitCommand::new`] and run with [`GitCommand::run`].
#[derive(Debug)]
pub struct GitCommand {
    args: Vec<OsString>,
    cwd: Option<PathBuf>,
    stdin: Option<Vec<u8>>,
    access: Access,
    timeout: Duration,
    cancel: Option<CancellationToken>,
    ok_exit_codes: &'static [i32],
    literal_pathspecs: bool,
}

impl GitCommand {
    /// Creates a command with the given arguments (excluding `git` and base `-c` flags).
    pub fn new<I, S>(args: I, access: Access) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        Self {
            args: args
                .into_iter()
                .map(|a| a.as_ref().to_os_string())
                .collect(),
            cwd: None,
            stdin: None,
            access,
            timeout: DEFAULT_TIMEOUT,
            cancel: None,
            literal_pathspecs: false,
            ok_exit_codes: &[0],
        }
    }

    /// Working directory (the repository root).
    pub fn cwd(mut self, dir: impl Into<PathBuf>) -> Self {
        self.cwd = Some(dir.into());
        self
    }

    /// Bytes written to git's stdin (e.g. a commit message for `commit -F -`).
    pub fn stdin(mut self, data: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(data.into());
        self
    }

    /// Overrides the default timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Kills the process when `token` is cancelled.
    pub fn cancel_on(mut self, token: CancellationToken) -> Self {
        self.cancel = Some(token);
        self
    }

    /// Pathspecs are literal paths (`GIT_LITERAL_PATHSPECS=1`): no globs, no `:(magic)`.
    pub fn literal_pathspecs(mut self) -> Self {
        self.literal_pathspecs = true;
        self
    }

    /// Feeds `paths` as NUL-separated pathspecs on stdin. Add
    /// `--pathspec-from-file=- --pathspec-file-nul` to the args. Avoids command-line
    /// length limits (Windows: 32K chars) and implies [`Self::literal_pathspecs`].
    pub fn pathspecs_on_stdin<S: AsRef<str>>(self, paths: &[S]) -> Self {
        let mut buf = Vec::new();
        for p in paths {
            buf.extend_from_slice(p.as_ref().as_bytes());
            buf.push(0);
        }
        self.stdin(buf).literal_pathspecs()
    }

    /// Exit codes treated as success (default `[0]`), e.g. `[0, 1]` for `diff --no-index`.
    pub fn ok_exit_codes(mut self, codes: &'static [i32]) -> Self {
        self.ok_exit_codes = codes;
        self
    }

    /// Full argument vector including base `-c` flags. Exposed for tests.
    fn full_args(&self) -> Vec<OsString> {
        let mut out: Vec<OsString> = Vec::with_capacity(self.args.len() + 4);
        for flag in base_config_flags() {
            out.push("-c".into());
            out.push(flag.into());
        }
        out.extend(self.args.iter().cloned());
        out
    }

    /// Spawns git and waits for it to exit, honoring timeout and cancellation.
    pub async fn run(self, git: &GitBinary) -> Result<GitOutput, GitError> {
        let args = self.full_args();
        tracing::debug!(
            args = %redact(&args.iter().map(|a| a.to_string_lossy()).collect::<Vec<_>>().join(" ")),
            access = ?self.access,
            "spawning git"
        );

        let mut cmd = Command::new(git.path());
        cmd.args(&args)
            .stdin(if self.stdin.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        apply_base_env(&mut cmd, self.access);
        if self.literal_pathspecs {
            cmd.env("GIT_LITERAL_PATHSPECS", "1");
        }
        if let Some(dir) = &self.cwd {
            cmd.current_dir(dir);
        }
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }
        // Own process group, so helpers git spawns (ssh, git-remote-https) die with it.
        #[cfg(unix)]
        cmd.process_group(0);

        let mut child = cmd.spawn().map_err(GitError::Spawn)?;
        let pid = child.id();
        let stdin_pipe = child.stdin.take();
        let input = self.stdin;
        let write_stdin = async move {
            if let (Some(mut pipe), Some(data)) = (stdin_pipe, input) {
                // A broken pipe here surfaces as a git failure below; ignore it.
                let _ = pipe.write_all(&data).await;
                let _ = pipe.shutdown().await;
            }
        };
        // Write stdin concurrently with reading output so large payloads can't deadlock.
        let wait = async {
            let ((), out) = tokio::join!(write_stdin, child.wait_with_output());
            out
        };
        let cancelled = async {
            match &self.cancel {
                Some(token) => token.cancelled().await,
                None => std::future::pending().await,
            }
        };

        // `wait` is pinned so the child stays unreaped until the tree is killed;
        // otherwise its process-group id could be recycled before `killpg`.
        let wait = tokio::time::timeout(self.timeout, wait);
        tokio::pin!(wait);
        let mut tree = TreeKiller::new(pid);
        let finished = tokio::select! {
            res = &mut wait => Some(res),
            () = cancelled => None,
        };
        let output = match finished {
            Some(Ok(out)) => {
                tree.disarm();
                out.map_err(GitError::Spawn)?
            }
            Some(Err(_elapsed)) => return Err(GitError::TimedOut),
            None => return Err(GitError::Cancelled),
        };

        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        let exit_code = output.status.code();
        if exit_code.is_some_and(|c| self.ok_exit_codes.contains(&c)) {
            return Ok(GitOutput {
                stdout: output.stdout,
                stderr,
                exit_code,
            });
        }
        let stderr = redact(&stderr);
        tracing::debug!(?exit_code, %stderr, "git failed");
        Err(GitError::Failed {
            kind: classify(&stderr),
            exit_code,
            stderr,
        })
    }
}

/// `-c` flags applied to every invocation. Never written to the user's config.
fn base_config_flags() -> Vec<&'static str> {
    // TODO(M3/M4): credential.helper handling per spec §5.2 / §6.2.
    #[allow(unused_mut)]
    let mut flags = vec!["core.quotepath=false"];
    #[cfg(windows)]
    flags.push("http.sslBackend=schannel");
    flags
}

/// Base environment for every invocation (spec §5.2).
fn apply_base_env(cmd: &mut Command, access: Access) {
    // Repo-targeting variables inherited from a parent shell must not redirect us.
    for var in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_LITERAL_PATHSPECS",
        "GIT_GLOB_PATHSPECS",
        "GIT_NOGLOB_PATHSPECS",
        "GIT_ICASE_PATHSPECS",
    ] {
        cmd.env_remove(var);
    }
    cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    if access == Access::ReadOnly {
        cmd.env("GIT_OPTIONAL_LOCKS", "0");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::error::GitErrorKind;

    fn git() -> GitBinary {
        resolve(None, None).expect("git on PATH for tests")
    }

    #[test]
    fn base_flags_precede_user_args() {
        let cmd = GitCommand::new(["status", "--", "-weird-file"], Access::ReadOnly);
        let args = cmd.full_args();
        assert_eq!(args[0], "-c");
        assert_eq!(args[1], "core.quotepath=false");
        assert_eq!(args[args.len() - 1], "-weird-file");
    }

    #[tokio::test]
    async fn runs_version() {
        let out = GitCommand::new(["--version"], Access::ReadOnly)
            .run(&git())
            .await
            .unwrap();
        assert!(String::from_utf8_lossy(&out.stdout).starts_with("git version "));
    }

    #[tokio::test]
    async fn failure_is_classified_and_redacted() {
        let dir = tempfile::tempdir().unwrap();
        let err = GitCommand::new(["status"], Access::ReadOnly)
            .cwd(dir.path())
            .run(&git())
            .await
            .unwrap_err();
        match err {
            GitError::Failed { kind, .. } => assert_eq!(kind, GitErrorKind::NotARepository),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[tokio::test]
    async fn stdin_is_delivered() {
        let out = GitCommand::new(["hash-object", "--stdin"], Access::ReadOnly)
            .stdin("hello\n")
            .run(&git())
            .await
            .unwrap();
        assert_eq!(
            String::from_utf8_lossy(&out.stdout).trim(),
            "ce013625030ba8dba906f756967f9e9ca394464a"
        );
    }

    /// A git invocation that blocks for 30s (shell alias; unix-only test helper).
    #[cfg(unix)]
    fn sleeper() -> GitCommand {
        GitCommand::new(["-c", "alias.zz=!sleep 30", "zz"], Access::ReadOnly)
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_process() {
        let token = CancellationToken::new();
        let canceller = token.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            canceller.cancel();
        });
        let started = std::time::Instant::now();
        let res = sleeper().cancel_on(token).run(&git()).await;
        assert!(matches!(res, Err(GitError::Cancelled)), "{res:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_kills_grandchildren() {
        let dir = tempfile::tempdir().unwrap();
        let pid_file = dir.path().join("pid");
        let alias = format!(
            "alias.zz=!echo $$ > '{}'; exec sleep 30",
            pid_file.display()
        );
        let token = CancellationToken::new();
        let canceller = token.clone();
        let watch = pid_file.clone();
        tokio::spawn(async move {
            while !watch.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            canceller.cancel();
        });
        let res = GitCommand::new(["-c", &alias, "zz"], Access::ReadOnly)
            .cancel_on(token)
            .run(&git())
            .await;
        assert!(matches!(res, Err(GitError::Cancelled)), "{res:?}");

        let pid: libc::pid_t = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        // SAFETY: signal 0 only checks for existence.
        while unsafe { libc::kill(pid, 0) } == 0 {
            assert!(
                std::time::Instant::now() < deadline,
                "grandchild {pid} survived"
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timeout_is_reported() {
        let res = sleeper()
            .timeout(Duration::from_millis(200))
            .run(&git())
            .await;
        assert!(matches!(res, Err(GitError::TimedOut)), "{res:?}");
    }
}

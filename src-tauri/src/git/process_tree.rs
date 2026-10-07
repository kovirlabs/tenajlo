//! Killing git together with the helpers it spawned (ssh, git-remote-https, hooks).
//!
//! Unix: git runs in its own process group (set at spawn); we `killpg` it.
//! Windows: git is assigned to a job object right after spawn; we terminate the job.
//! Nothing is killed after a normal exit, so long-lived helpers such as
//! `git-credential-cache--daemon` survive successful operations.

use tokio::process::Child;

/// Kills git's whole process tree when dropped, unless disarmed after a normal exit.
///
/// Must be dropped *before* the `Child` is reaped: on Unix the group leader's pid is the
/// group id, and it can't be reused while the leader is unreaped.
pub(super) struct TreeKiller {
    #[cfg(unix)]
    pgid: Option<u32>,
    /// Job handle stored as an integer so the killer is `Send`.
    #[cfg(windows)]
    job: Option<isize>,
    armed: bool,
}

impl TreeKiller {
    /// Captures `child`'s process tree. Call immediately after spawning.
    pub(super) fn new(child: &Child) -> Self {
        #[cfg(unix)]
        {
            Self {
                pgid: child.id(),
                armed: true,
            }
        }
        #[cfg(windows)]
        {
            Self {
                job: windows::assign_job(child),
                armed: true,
            }
        }
    }

    /// Call after the child exited normally.
    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TreeKiller {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let (true, Some(pgid)) = (self.armed, self.pgid) {
            if let Ok(pgid) = libc::pid_t::try_from(pgid) {
                // SAFETY: killpg has no memory-safety preconditions; a stale or invalid pgid
                // just returns ESRCH.
                unsafe {
                    libc::killpg(pgid, libc::SIGKILL);
                }
            }
        }
        #[cfg(windows)]
        if let Some(job) = self.job.take() {
            windows::finish_job(job, self.armed);
        }
    }
}

#[cfg(windows)]
mod windows {
    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, TerminateJobObject,
    };

    /// Creates a job and puts `child` in it. `None` if either step fails (we then fall back
    /// to killing only git via `kill_on_drop`).
    pub(super) fn assign_job(child: &Child) -> Option<isize> {
        let process = child.raw_handle()? as HANDLE;
        // SAFETY: null attributes and name create an anonymous job; the returned handle is
        // checked before use and closed in `finish_job`.
        unsafe {
            let job = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if job.is_null() {
                return None;
            }
            if AssignProcessToJobObject(job, process) == 0 {
                CloseHandle(job);
                return None;
            }
            Some(job as isize)
        }
    }

    /// Terminates the job's processes if `kill`, then closes the handle.
    pub(super) fn finish_job(job: isize, kill: bool) {
        let job = job as HANDLE;
        // SAFETY: `job` came from CreateJobObjectW and is closed exactly once here.
        unsafe {
            if kill {
                TerminateJobObject(job, 1);
            }
            CloseHandle(job);
        }
    }
}

//! Killing git together with the helpers it spawned (ssh, git-remote-https, hooks).

/// Kills git's whole process tree when dropped, unless disarmed after a normal exit.
///
/// Declared after the pinned `wait` future, so it drops first: the group leader is
/// still unreaped and its pid (the group id) cannot have been reused.
pub(super) struct TreeKiller(Option<u32>);

impl TreeKiller {
    /// Arms a killer for the process group led by `pid`.
    pub(super) fn new(pid: Option<u32>) -> Self {
        Self(pid)
    }

    /// Call after the child exited normally.
    pub(super) fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for TreeKiller {
    fn drop(&mut self) {
        if let Some(pid) = self.0.take() {
            kill_tree(pid);
        }
    }
}

#[cfg(unix)]
fn kill_tree(pgid: u32) {
    let Ok(pgid) = libc::pid_t::try_from(pgid) else {
        return;
    };
    // SAFETY: killpg has no memory-safety preconditions; a stale or invalid pgid just returns ESRCH.
    unsafe {
        libc::killpg(pgid, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn kill_tree(_pid: u32) {
    // TODO(M3): assign git to a job object and terminate the job here.
    // Until then `kill_on_drop` kills only git itself.
}

//! Per-operation registration: the random token, git's environment and helper config.

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use tenajlo_askpass::{credential_helper_config, ENV_PORT, ENV_TOKEN};
use tokio_util::sync::CancellationToken;

use super::{Inner, OpEntry, Trampoline};
use crate::auth::credential_helper::AccountCredential;

impl Trampoline {
    /// Registers an operation. Prompts for it may cancel `cancel` (which kills git).
    /// With `account`, git's credential helper requests for that server are answered from it.
    pub fn register(
        &self,
        repo_id: &str,
        op_id: &str,
        cancel: CancellationToken,
        account: Option<AccountCredential>,
    ) -> std::io::Result<OpToken> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let entry = OpEntry {
            repo_id: repo_id.to_owned(),
            op_id: op_id.to_owned(),
            cancel,
            account,
            account_used: false,
            cached_password: None,
            passphrase_keys: Vec::new(),
            interactive: true,
            remember_login: false,
        };
        self.inner.lock().insert(token.clone(), entry);
        Ok(OpToken {
            token,
            inner: self.inner.clone(),
        })
    }
}

/// An operation's trampoline registration. Dropping it revokes the token.
pub struct OpToken {
    pub(super) token: String,
    pub(super) inner: Arc<Inner>,
}

impl OpToken {
    /// Environment for git so it routes prompts through `askpass` to this operation.
    pub fn env(&self, askpass: &Path) -> Vec<(OsString, OsString)> {
        let mut env: Vec<(OsString, OsString)> = vec![
            ("GIT_ASKPASS".into(), askpass.as_os_str().to_owned()),
            // OpenSSH passphrase and host-key prompts (spec §6.3). `force` makes ssh use
            // askpass even when Tenajlo was started from a terminal.
            ("SSH_ASKPASS".into(), askpass.as_os_str().to_owned()),
            ("SSH_ASKPASS_REQUIRE".into(), "force".into()),
            (ENV_PORT.into(), self.inner.port.to_string().into()),
            (ENV_TOKEN.into(), self.token.clone().into()),
        ];
        // OpenSSH before 8.4 ignores SSH_ASKPASS_REQUIRE and only uses askpass with DISPLAY set.
        if std::env::var_os("DISPLAY").is_none() {
            env.push(("DISPLAY".into(), ".".into()));
        }
        env
    }
}

impl OpToken {
    /// `-c` flags for git's credential helpers (spec §6.2). With an account, Tenajlo's helper
    /// replaces the others so the account's token is used. Without one, it's added after the
    /// user's own helpers, which run first as before, to offer and save remembered logins.
    pub fn config(&self, askpass: &Path) -> Vec<String> {
        let has_account = self
            .inner
            .lock()
            .get(&self.token)
            .is_some_and(|e| e.account.is_some());
        let ours = format!("credential.helper={}", credential_helper_config(askpass));
        if has_account {
            // An empty value clears every helper configured so far, including URL-specific ones.
            vec!["credential.helper=".into(), ours]
        } else if self.inner.saved.is_some() {
            vec![ours]
        } else {
            Vec::new()
        }
    }

    /// Stops prompts for this operation (background fetch): askpass gets no answer, so git
    /// fails instead of showing a dialog. The account credential helper still works.
    pub fn set_interactive(&self, interactive: bool) {
        if let Some(entry) = self.inner.lock().get_mut(&self.token) {
            entry.interactive = interactive;
        }
    }

    /// True if git used the account credential during this operation.
    pub fn account_used(&self) -> bool {
        self.inner
            .lock()
            .get(&self.token)
            .is_some_and(|e| e.account_used)
    }
}

impl Drop for OpToken {
    fn drop(&mut self) {
        self.inner.lock().remove(&self.token);
    }
}

impl std::fmt::Debug for OpToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OpToken(***)")
    }
}

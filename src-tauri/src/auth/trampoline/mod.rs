//! Loopback server answering `tenajlo-askpass` requests (spec §6.5).
//!
//! Each git operation registers a random 32-byte token, valid only while its [`OpToken`]
//! lives. Requests with unknown tokens are rejected and logged without the token.
//!
//! Operations on an account's server also get an [`AccountCredential`]: in credential-helper
//! mode the trampoline answers git's `get` with the account's login and PAT, only for that
//! server's protocol and host. Other HTTPS operations get [`SavedSecrets`] logins instead, as
//! a helper after the user's own; SSH passphrases the user chose to remember are answered
//! without a dialog.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tenajlo_askpass::{
    credential_helper_config, CredentialOp, Fields, Mode, Request, Response, ENV_PORT, ENV_TOKEN,
    MAX_LINE,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::credential_helper::{self, AccountCredential};
use super::prompt::{parse_prompt, AuthAnswer, PromptKind};
use super::saved_secrets::SavedSecrets;
use super::secrets::Secret;

/// How long the user has to answer a prompt before the operation is cancelled.
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How long a connected helper has to send its request line.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A prompt waiting for the user.
#[derive(Debug, Clone)]
pub struct PendingPrompt {
    pub repo_id: String,
    pub op_id: String,
    pub kind: PromptKind,
}

/// Shows a prompt and resolves with the user's answer (`None` = cancelled).
pub type PromptFn = Arc<
    dyn Fn(PendingPrompt) -> Pin<Box<dyn Future<Output = Option<AuthAnswer>> + Send>> + Send + Sync,
>;

struct OpEntry {
    repo_id: String,
    op_id: String,
    cancel: CancellationToken,
    account: Option<AccountCredential>,
    /// git asked for and received the account's credential.
    account_used: bool,
    /// Password from a combined username+password dialog, handed to git's next
    /// `Password for …` prompt and then forgotten.
    cached_password: Option<String>,
    /// Keys whose passphrase was already asked for: asking again means it was wrong.
    passphrase_keys: Vec<String>,
    /// Background operations never show prompts; git just gets no answer.
    interactive: bool,
    /// The user ticked "Remember" when signing in: save the login once git reports it worked.
    remember_login: bool,
}

struct Inner {
    port: u16,
    ops: Mutex<HashMap<String, OpEntry>>,
    prompt: PromptFn,
    prompt_timeout: Duration,
    /// Remembered logins and passphrases; `None` in tests that don't need them.
    saved: Option<Arc<SavedSecrets>>,
}

/// The running trampoline server. Cheap to clone.
#[derive(Clone)]
pub struct Trampoline {
    inner: Arc<Inner>,
}

impl Trampoline {
    /// Binds `127.0.0.1:0` and starts accepting helper connections. Nothing is remembered.
    pub async fn start(prompt: PromptFn) -> std::io::Result<Self> {
        Self::spawn(prompt, PROMPT_TIMEOUT, None).await
    }

    /// Like [`Self::start`], also answering from and saving to `saved`.
    pub async fn start_with_saved(
        prompt: PromptFn,
        saved: Arc<SavedSecrets>,
    ) -> std::io::Result<Self> {
        Self::spawn(prompt, PROMPT_TIMEOUT, Some(saved)).await
    }

    /// Like [`Self::start`] with a custom prompt timeout (tests).
    pub async fn start_with_timeout(
        prompt: PromptFn,
        prompt_timeout: Duration,
    ) -> std::io::Result<Self> {
        Self::spawn(prompt, prompt_timeout, None).await
    }

    async fn spawn(
        prompt: PromptFn,
        prompt_timeout: Duration,
        saved: Option<Arc<SavedSecrets>>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        tracing::info!(port, "askpass trampoline listening on loopback");
        let inner = Arc::new(Inner {
            port,
            ops: Mutex::default(),
            prompt,
            prompt_timeout,
            saved,
        });
        let server = inner.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let inner = server.clone();
                        tokio::spawn(async move { handle(&inner, stream).await });
                    }
                    Err(e) => {
                        // Errors like EMFILE persist; back off instead of spinning the loop.
                        tracing::warn!(error = %e, "trampoline accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        Ok(Self { inner })
    }

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

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, OpEntry>> {
        self.ops.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// An operation's trampoline registration. Dropping it revokes the token.
pub struct OpToken {
    token: String,
    inner: Arc<Inner>,
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

async fn handle(inner: &Inner, mut stream: TcpStream) {
    let response = match read_request(&mut stream).await {
        Some(req) if req.mode == Mode::Credential => Response {
            answer: None,
            fields: credential(inner, &req).await,
        },
        Some(req) => Response {
            answer: answer(inner, req).await,
            fields: Vec::new(),
        },
        None => Response {
            answer: None,
            fields: Vec::new(),
        },
    };
    let mut line = serde_json::to_vec(&response).unwrap_or_else(|_| b"{\"answer\":null}".to_vec());
    line.push(b'\n');
    let _ = stream.write_all(&line).await;
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream.take(MAX_LINE as u64));
    let read = tokio::time::timeout(REQUEST_TIMEOUT, reader.read_line(&mut line)).await;
    match read {
        Ok(Ok(n)) if n > 0 => serde_json::from_str(&line).ok(),
        _ => None,
    }
}

/// Credential-helper request: answered from the operation's account if it has one, else
/// from remembered logins.
async fn credential(inner: &Inner, req: &Request) -> Fields {
    let remember = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return Vec::new();
        };
        if let Some(account) = &entry.account {
            // `store` and `erase` are ignored (see [`credential_helper::answer`]).
            return match credential_helper::answer(account, req.op, &req.fields) {
                Some(fields) => {
                    entry.account_used = true;
                    fields
                }
                None => Vec::new(),
            };
        }
        entry.remember_login
    };
    let Some(saved) = &inner.saved else {
        return Vec::new();
    };
    saved_login(saved, req.op, &req.fields, remember).await
}

/// `get` answers from a remembered login; `store` saves the login the user asked to remember;
/// `erase` forgets a remembered login the server rejected.
async fn saved_login(
    saved: &SavedSecrets,
    op: Option<CredentialOp>,
    fields: &Fields,
    remember: bool,
) -> Fields {
    let field = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let (Some(protocol @ ("https" | "http")), Some(host)) = (field("protocol"), field("host"))
    else {
        return Vec::new();
    };
    match op {
        Some(CredentialOp::Get) => match saved.login(protocol, host, field("username")).await {
            Some((username, password)) => vec![
                ("username".into(), username),
                ("password".into(), password.expose().to_owned()),
            ],
            None => Vec::new(),
        },
        Some(CredentialOp::Store) if remember => {
            if let (Some(username), Some(password)) = (field("username"), field("password")) {
                let password = Secret::new(password.to_owned());
                if let Err(e) = saved.save_login(protocol, host, username, password).await {
                    tracing::warn!(error = %e, "could not remember a login");
                }
            }
            Vec::new()
        }
        Some(CredentialOp::Erase) => {
            if let (Some(username), Some(password)) = (field("username"), field("password")) {
                saved
                    .forget_rejected_login(protocol, host, username, password)
                    .await;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

async fn answer(inner: &Inner, req: Request) -> Option<String> {
    let mut kind = parse_prompt(req.prompt.as_deref().unwrap_or_default());
    let interactive = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return None;
        };
        if let PromptKind::Passphrase { key, retry } = &mut kind {
            *retry = entry.passphrase_keys.contains(key);
            if !*retry {
                entry.passphrase_keys.push(key.clone());
            }
        }
        entry.interactive
    };
    // A remembered passphrase answers without a dialog, even for background operations.
    // OpenSSH asking again for the same key means it was wrong, so it's forgotten.
    if let (PromptKind::Passphrase { key, retry }, Some(saved)) = (&kind, &inner.saved) {
        if *retry {
            saved.forget_passphrase(key).await;
        } else if let Some(passphrase) = saved.passphrase(key).await {
            return Some(passphrase.expose().to_owned());
        }
    }
    if !interactive {
        return None;
    }

    let (pending, cancel) = {
        let mut ops = inner.lock();
        let entry = ops.get_mut(&req.token)?;
        if let (PromptKind::Password { .. }, Some(pw)) = (&kind, entry.cached_password.take()) {
            return Some(pw);
        }
        let pending = PendingPrompt {
            repo_id: entry.repo_id.clone(),
            op_id: entry.op_id.clone(),
            kind: kind.clone(),
        };
        (pending, entry.cancel.clone())
    };

    let asked = tokio::select! {
        res = tokio::time::timeout(inner.prompt_timeout, (inner.prompt)(pending)) => res.ok().flatten(),
        () = cancel.cancelled() => None,
    };
    let Some(answer) = asked else {
        // Spec §6.5: cancelling or timing out kills the git process.
        cancel.cancel();
        return None;
    };
    match kind {
        PromptKind::Credentials { .. } | PromptKind::Password { .. } => {
            let mut ops = inner.lock();
            let entry = ops.get_mut(&req.token);
            if let Some(entry) = entry {
                // Saved by `store` once git reports the login worked.
                entry.remember_login |= answer.remember;
                if matches!(kind, PromptKind::Credentials { .. }) {
                    entry.cached_password = Some(answer.secret);
                    return Some(answer.username.unwrap_or_default());
                }
            }
            Some(answer.secret)
        }
        PromptKind::Passphrase { key, .. } => {
            if let (true, Some(saved)) = (answer.remember, &inner.saved) {
                // Saved now: OpenSSH never says it worked, but asks again if it didn't.
                let passphrase = Secret::new(answer.secret.clone());
                if let Err(e) = saved.save_passphrase(&key, passphrase).await {
                    tracing::warn!(error = %e, "could not remember an SSH key passphrase");
                }
            }
            Some(answer.secret)
        }
        // Accepting is decided here, not by the UI's text: OpenSSH expects exactly "yes".
        PromptKind::HostKey { .. } => Some("yes".to_owned()),
        PromptKind::Other { .. } => Some(answer.secret),
    }
}

#[cfg(test)]
mod tests;

//! Loopback server answering `tenajlo-askpass` requests (spec §6.5).
//!
//! Each git operation registers a random 32-byte token, valid only while its [`OpToken`]
//! lives. Requests with unknown tokens are rejected and logged without the token.
//!
//! Operations on an account's server also get an [`AccountCredential`]: in credential-helper
//! mode the trampoline answers git's `get` with the account's login and PAT, only for that
//! server's protocol and host.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tenajlo_askpass::{
    credential_helper_config, Fields, Mode, Request, Response, ENV_PORT, ENV_TOKEN, MAX_LINE,
};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

use super::credential_helper::{self, AccountCredential};
use super::prompt::{parse_prompt, AuthAnswer, PromptKind};

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
}

struct Inner {
    port: u16,
    ops: Mutex<HashMap<String, OpEntry>>,
    prompt: PromptFn,
    prompt_timeout: Duration,
}

/// The running trampoline server. Cheap to clone.
#[derive(Clone)]
pub struct Trampoline {
    inner: Arc<Inner>,
}

impl Trampoline {
    /// Binds `127.0.0.1:0` and starts accepting helper connections.
    pub async fn start(prompt: PromptFn) -> std::io::Result<Self> {
        Self::start_with_timeout(prompt, PROMPT_TIMEOUT).await
    }

    /// Like [`Self::start`] with a custom prompt timeout (tests).
    pub async fn start_with_timeout(
        prompt: PromptFn,
        prompt_timeout: Duration,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        tracing::info!(port, "askpass trampoline listening on loopback");
        let inner = Arc::new(Inner {
            port,
            ops: Mutex::default(),
            prompt,
            prompt_timeout,
        });
        let server = inner.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let inner = server.clone();
                        tokio::spawn(async move { handle(&inner, stream).await });
                    }
                    Err(e) => tracing::warn!(error = %e, "trampoline accept failed"),
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
    /// `-c` flags that make git ask this operation's account credential first. Empty when
    /// the operation has no account, leaving the user's own helpers alone (spec §6.2).
    pub fn config(&self, askpass: &Path) -> Vec<String> {
        let has_account = self
            .inner
            .lock()
            .get(&self.token)
            .is_some_and(|e| e.account.is_some());
        if !has_account {
            return Vec::new();
        }
        vec![
            // An empty value clears every helper configured so far, including URL-specific ones.
            "credential.helper=".into(),
            format!("credential.helper={}", credential_helper_config(askpass)),
        ]
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
            fields: credential(inner, &req),
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

/// Credential-helper request: answered from the operation's account, if it matches.
/// `store` and `erase` are ignored (see [`credential_helper::answer`]).
fn credential(inner: &Inner, req: &Request) -> Fields {
    let mut ops = inner.lock();
    let Some(entry) = ops.get_mut(&req.token) else {
        tracing::warn!("rejected trampoline request: unknown or expired token");
        return Vec::new();
    };
    let Some(account) = &entry.account else {
        return Vec::new();
    };
    match credential_helper::answer(account, req.op, &req.fields) {
        Some(fields) => {
            entry.account_used = true;
            fields
        }
        None => Vec::new(),
    }
}

async fn answer(inner: &Inner, req: Request) -> Option<String> {
    let mut kind = parse_prompt(req.prompt.as_deref().unwrap_or_default());
    let (pending, cancel) = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return None;
        };
        if !entry.interactive {
            return None;
        }
        if let (PromptKind::Password { .. }, Some(pw)) = (&kind, entry.cached_password.take()) {
            return Some(pw);
        }
        if let PromptKind::Passphrase { key, retry } = &mut kind {
            *retry = entry.passphrase_keys.contains(key);
            if !*retry {
                entry.passphrase_keys.push(key.clone());
            }
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
        PromptKind::Credentials { .. } => {
            if let Some(entry) = inner.lock().get_mut(&req.token) {
                entry.cached_password = Some(answer.secret);
            }
            Some(answer.username.unwrap_or_default())
        }
        // Accepting is decided here, not by the UI's text: OpenSSH expects exactly "yes".
        PromptKind::HostKey { .. } => Some("yes".to_owned()),
        PromptKind::Password { .. } | PromptKind::Passphrase { .. } | PromptKind::Other { .. } => {
            Some(answer.secret)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::secrets::Secret;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tenajlo_askpass::CredentialOp;

    /// Fake user: answers with `answer`, counting prompts.
    fn user(
        answer: Option<AuthAnswer>,
    ) -> (PromptFn, Arc<AtomicUsize>, Arc<Mutex<Vec<PromptKind>>>) {
        let count = Arc::new(AtomicUsize::new(0));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let (c, s) = (count.clone(), seen.clone());
        let f: PromptFn = Arc::new(move |p: PendingPrompt| {
            c.fetch_add(1, Ordering::SeqCst);
            s.lock().unwrap().push(p.kind);
            let a = answer.clone();
            Box::pin(async move { a })
        });
        (f, count, seen)
    }

    async fn ask(t: &Trampoline, token: &str, prompt: &str) -> Option<String> {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, t.inner.port))
            .await
            .unwrap();
        let req = Request {
            token: token.into(),
            mode: Mode::Askpass,
            prompt: Some(prompt.into()),
            op: None,
            fields: Vec::new(),
        };
        let mut line = serde_json::to_vec(&req).unwrap();
        line.push(b'\n');
        s.write_all(&line).await.unwrap();
        let mut reply = String::new();
        BufReader::new(s).read_line(&mut reply).await.unwrap();
        serde_json::from_str::<Response>(&reply).unwrap().answer
    }

    fn token_of(op: &OpToken) -> String {
        op.token.clone()
    }

    #[tokio::test]
    async fn one_dialog_answers_both_git_prompts() {
        let answer = AuthAnswer {
            username: Some("evan".into()),
            secret: "pat-123".into(),
        };
        let (prompt, count, seen) = user(Some(answer));
        let t = Trampoline::start(prompt).await.unwrap();
        let op = t
            .register("repo", "op1", CancellationToken::new(), None)
            .unwrap();
        let tok = token_of(&op);
        assert_eq!(tok.len(), 64);

        assert_eq!(
            ask(&t, &tok, "Username for 'https://h': ").await.as_deref(),
            Some("evan")
        );
        assert_eq!(
            ask(&t, &tok, "Password for 'https://evan@h': ")
                .await
                .as_deref(),
            Some("pat-123")
        );
        assert_eq!(
            count.load(Ordering::SeqCst),
            1,
            "password came from the same dialog"
        );
        assert_eq!(
            seen.lock().unwrap()[0],
            PromptKind::Credentials { host: "h".into() }
        );

        // The cached password is used once; a second password prompt asks again.
        ask(&t, &tok, "Password for 'https://evan@h': ").await;
        assert_eq!(count.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn ssh_prompts_host_key_yes_and_passphrase_retry() {
        let (prompt, _, seen) = user(Some(AuthAnswer {
            username: None,
            secret: "pass phrase".into(),
        }));
        let t = Trampoline::start(prompt).await.unwrap();
        let op = t
            .register("repo", "op1", CancellationToken::new(), None)
            .unwrap();
        let tok = token_of(&op);
        let host_key = "The authenticity of host 'h (1.2.3.4)' can't be established.\nED25519 key fingerprint is SHA256:abc.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert_eq!(ask(&t, &tok, host_key).await.as_deref(), Some("yes"));
        let pp = "Enter passphrase for key '/k': ";
        assert_eq!(ask(&t, &tok, pp).await.as_deref(), Some("pass phrase"));
        ask(&t, &tok, pp).await;
        let seen = seen.lock().unwrap();
        assert_eq!(
            seen[1],
            PromptKind::Passphrase {
                key: "/k".into(),
                retry: false
            }
        );
        assert_eq!(
            seen[2],
            PromptKind::Passphrase {
                key: "/k".into(),
                retry: true
            }
        );
    }

    #[tokio::test]
    async fn env_routes_git_and_ssh_prompts() {
        let (prompt, _, _) = user(None);
        let t = Trampoline::start(prompt).await.unwrap();
        let op = t
            .register("repo", "op1", CancellationToken::new(), None)
            .unwrap();
        let env = op.env(Path::new("/opt/tenajlo-askpass"));
        let get = |k: &str| env.iter().find(|(key, _)| key == k).map(|(_, v)| v.clone());
        assert_eq!(get("GIT_ASKPASS"), Some("/opt/tenajlo-askpass".into()));
        assert_eq!(get("SSH_ASKPASS"), Some("/opt/tenajlo-askpass".into()));
        assert_eq!(get("SSH_ASKPASS_REQUIRE"), Some("force".into()));
        assert_eq!(get(ENV_TOKEN), Some(token_of(&op).into()));
    }

    #[tokio::test]
    async fn non_interactive_ops_never_prompt_or_cancel() {
        let (prompt, count, _) = user(Some(AuthAnswer {
            username: Some("u".into()),
            secret: "s".into(),
        }));
        let t = Trampoline::start(prompt).await.unwrap();
        let cancel = CancellationToken::new();
        let op = t.register("repo", "op1", cancel.clone(), None).unwrap();
        op.set_interactive(false);
        assert_eq!(
            ask(&t, &token_of(&op), "Username for 'https://h': ").await,
            None
        );
        assert_eq!(count.load(Ordering::SeqCst), 0);
        assert!(!cancel.is_cancelled());
    }

    #[tokio::test]
    async fn rejects_unknown_revoked_and_malformed() {
        let (prompt, count, _) = user(Some(AuthAnswer {
            username: None,
            secret: "x".into(),
        }));
        let t = Trampoline::start(prompt).await.unwrap();
        assert_eq!(ask(&t, "nope", "Password: ").await, None);

        let op = t
            .register("repo", "op1", CancellationToken::new(), None)
            .unwrap();
        let tok = token_of(&op);
        drop(op);
        assert_eq!(
            ask(&t, &tok, "Password: ").await,
            None,
            "token revoked when the op ends"
        );

        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, t.inner.port))
            .await
            .unwrap();
        s.write_all(b"not json\n").await.unwrap();
        let mut reply = String::new();
        BufReader::new(s).read_line(&mut reply).await.unwrap();
        assert_eq!(reply.trim(), r#"{"answer":null}"#);
        assert_eq!(count.load(Ordering::SeqCst), 0, "user never prompted");
    }

    #[tokio::test]
    async fn cancel_and_timeout_cancel_the_operation() {
        let (prompt, _, _) = user(None);
        let t = Trampoline::start(prompt).await.unwrap();
        let cancel = CancellationToken::new();
        let op = t.register("repo", "op1", cancel.clone(), None).unwrap();
        assert_eq!(
            ask(&t, &token_of(&op), "Username for 'https://h': ").await,
            None
        );
        assert!(cancel.is_cancelled(), "user cancel kills git");

        let never: PromptFn = Arc::new(|_| Box::pin(std::future::pending()));
        let t = Trampoline::start_with_timeout(never, Duration::from_millis(50))
            .await
            .unwrap();
        let cancel = CancellationToken::new();
        let op = t.register("repo", "op2", cancel.clone(), None).unwrap();
        assert_eq!(ask(&t, &token_of(&op), "Password: ").await, None);
        assert!(cancel.is_cancelled(), "timeout kills git");
    }

    async fn ask_credential(
        t: &Trampoline,
        token: &str,
        op: CredentialOp,
        fields: &[(&str, &str)],
    ) -> Fields {
        let mut s = TcpStream::connect((Ipv4Addr::LOCALHOST, t.inner.port))
            .await
            .unwrap();
        let req = Request {
            token: token.into(),
            mode: Mode::Credential,
            prompt: None,
            op: Some(op),
            fields: fields
                .iter()
                .map(|(k, v)| ((*k).into(), (*v).into()))
                .collect(),
        };
        let mut line = serde_json::to_vec(&req).unwrap();
        line.push(b'\n');
        s.write_all(&line).await.unwrap();
        let mut reply = String::new();
        BufReader::new(s).read_line(&mut reply).await.unwrap();
        serde_json::from_str::<Response>(&reply).unwrap().fields
    }

    fn account() -> AccountCredential {
        AccountCredential {
            protocol: "https".into(),
            host: "git.example.com".into(),
            login: "evan".into(),
            token: Secret::new("pat-123".into()),
        }
    }

    #[tokio::test]
    async fn credential_requests_use_the_operations_account() {
        let (prompt, count, _) = user(None);
        let t = Trampoline::start(prompt).await.unwrap();
        let op = t
            .register("repo", "op1", CancellationToken::new(), Some(account()))
            .unwrap();
        let tok = token_of(&op);
        assert_eq!(
            op.config(Path::new("/opt/tenajlo-askpass"))[0],
            "credential.helper="
        );

        let wrong = [("protocol", "https"), ("host", "evil.example")];
        assert!(ask_credential(&t, &tok, CredentialOp::Get, &wrong)
            .await
            .is_empty());
        assert!(!op.account_used());

        let right = [("protocol", "https"), ("host", "git.example.com")];
        assert!(ask_credential(&t, "nope", CredentialOp::Get, &right)
            .await
            .is_empty());
        let got = ask_credential(&t, &tok, CredentialOp::Get, &right).await;
        assert_eq!(got[1], ("password".into(), "pat-123".into()));
        assert!(op.account_used());
        assert_eq!(count.load(Ordering::SeqCst), 0, "user never prompted");
    }

    #[tokio::test]
    async fn no_account_means_no_helper_override() {
        let (prompt, _, _) = user(None);
        let t = Trampoline::start(prompt).await.unwrap();
        let op = t
            .register("repo", "op1", CancellationToken::new(), None)
            .unwrap();
        assert!(op.config(Path::new("/x")).is_empty());
        let fields = [("protocol", "https"), ("host", "git.example.com")];
        assert!(
            ask_credential(&t, &token_of(&op), CredentialOp::Get, &fields)
                .await
                .is_empty()
        );
    }
}

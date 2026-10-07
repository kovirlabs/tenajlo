//! Loopback server answering `tenajlo-askpass` requests (spec §6.5).
//!
//! Each git operation registers a random 32-byte token, valid only while its [`OpToken`]
//! lives. Requests with unknown tokens are rejected and logged without the token.

use std::collections::HashMap;
use std::ffi::OsString;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tenajlo_askpass::{Request, Response, ENV_PORT, ENV_TOKEN, MAX_LINE};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::sync::CancellationToken;

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
    /// Password from a combined username+password dialog, handed to git's next
    /// `Password for …` prompt and then forgotten.
    cached_password: Option<String>,
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
    pub fn register(
        &self,
        repo_id: &str,
        op_id: &str,
        cancel: CancellationToken,
    ) -> std::io::Result<OpToken> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|e| std::io::Error::other(e.to_string()))?;
        let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let entry = OpEntry {
            repo_id: repo_id.to_owned(),
            op_id: op_id.to_owned(),
            cancel,
            cached_password: None,
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
        vec![
            ("GIT_ASKPASS".into(), askpass.as_os_str().to_owned()),
            (ENV_PORT.into(), self.inner.port.to_string().into()),
            (ENV_TOKEN.into(), self.token.clone().into()),
        ]
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
    let answer = match read_request(&mut stream).await {
        Some(req) => answer(inner, req).await,
        None => None,
    };
    let mut line =
        serde_json::to_vec(&Response { answer }).unwrap_or_else(|_| b"{\"answer\":null}".to_vec());
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

async fn answer(inner: &Inner, req: Request) -> Option<String> {
    let kind = parse_prompt(req.prompt.as_deref().unwrap_or_default());
    let (pending, cancel) = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return None;
        };
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
        PromptKind::Credentials { .. } => {
            if let Some(entry) = inner.lock().get_mut(&req.token) {
                entry.cached_password = Some(answer.secret);
            }
            Some(answer.username.unwrap_or_default())
        }
        PromptKind::Password { .. } | PromptKind::Other { .. } => Some(answer.secret),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tenajlo_askpass::Mode;

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
        let op = t.register("repo", "op1", CancellationToken::new()).unwrap();
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
    async fn rejects_unknown_revoked_and_malformed() {
        let (prompt, count, _) = user(Some(AuthAnswer {
            username: None,
            secret: "x".into(),
        }));
        let t = Trampoline::start(prompt).await.unwrap();
        assert_eq!(ask(&t, "nope", "Password: ").await, None);

        let op = t.register("repo", "op1", CancellationToken::new()).unwrap();
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
        let op = t.register("repo", "op1", cancel.clone()).unwrap();
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
        let op = t.register("repo", "op2", cancel.clone()).unwrap();
        assert_eq!(ask(&t, &token_of(&op), "Password: ").await, None);
        assert!(cancel.is_cancelled(), "timeout kills git");
    }
}

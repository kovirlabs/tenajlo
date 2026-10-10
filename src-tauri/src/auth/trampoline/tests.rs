//! Trampoline tests: prompts, credential helper requests, tokens and cancellation.

use super::*;
use crate::auth::saved_secrets::SavedSecrets;
use crate::auth::secrets::Secret;
use std::sync::atomic::{AtomicUsize, Ordering};
use tenajlo_askpass::CredentialOp;

/// Fake user: answers with `answer`, counting prompts.
fn user(answer: Option<AuthAnswer>) -> (PromptFn, Arc<AtomicUsize>, Arc<Mutex<Vec<PromptKind>>>) {
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
        remember: false,
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
        remember: false,
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
        remember: false,
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
        remember: false,
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

/// A trampoline that remembers into an in-memory keychain.
async fn with_saved(
    answer: Option<AuthAnswer>,
) -> (
    Trampoline,
    Arc<SavedSecrets>,
    Arc<AtomicUsize>,
    Arc<Mutex<Vec<PromptKind>>>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let saved = Arc::new(SavedSecrets::load(
        dir.path(),
        Arc::new(crate::auth::secrets::MemoryStore::default()),
    ));
    let (prompt, count, seen) = user(answer);
    let t = Trampoline::start_with_saved(prompt, saved.clone())
        .await
        .unwrap();
    (t, saved, count, seen, dir)
}

#[tokio::test]
async fn remembered_logins_are_offered_saved_and_forgotten() {
    let answer = AuthAnswer {
        username: Some("evan".into()),
        secret: "pw-1".into(),
        remember: true,
    };
    let (t, saved, count, _, _dir) = with_saved(Some(answer)).await;
    let op = t
        .register("repo", "op1", CancellationToken::new(), None)
        .unwrap();
    let tok = token_of(&op);
    // Added after the user's helpers, not replacing them.
    assert_eq!(
        op.config(Path::new("/x")),
        vec![format!(
            "credential.helper={}",
            credential_helper_config(Path::new("/x"))
        )]
    );

    let host = [("protocol", "https"), ("host", "git.example.com")];
    assert!(ask_credential(&t, &tok, CredentialOp::Get, &host)
        .await
        .is_empty());
    // git falls back to askpass; the user ticks "Remember".
    ask(&t, &tok, "Username for 'https://git.example.com': ").await;
    ask(&t, &tok, "Password for 'https://evan@git.example.com': ").await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    let worked = [
        ("protocol", "https"),
        ("host", "git.example.com"),
        ("username", "evan"),
        ("password", "pw-1"),
    ];
    ask_credential(&t, &tok, CredentialOp::Store, &worked).await;
    drop(op);

    // A later operation gets it from the helper, with no dialog.
    let op = t
        .register("repo", "op2", CancellationToken::new(), None)
        .unwrap();
    let tok = token_of(&op);
    assert_eq!(
        ask_credential(&t, &tok, CredentialOp::Get, &host).await,
        vec![
            ("username".to_owned(), "evan".to_owned()),
            ("password".to_owned(), "pw-1".to_owned())
        ]
    );
    // The server rejects it: git erases it, and it's forgotten.
    ask_credential(&t, &tok, CredentialOp::Erase, &worked).await;
    assert!(saved
        .login("https", "git.example.com", None)
        .await
        .is_none());
}

#[tokio::test]
async fn logins_are_saved_only_when_asked() {
    let answer = AuthAnswer {
        username: Some("evan".into()),
        secret: "pw-1".into(),
        remember: false,
    };
    let (t, saved, _, _, _dir) = with_saved(Some(answer)).await;
    let op = t
        .register("repo", "op1", CancellationToken::new(), None)
        .unwrap();
    let tok = token_of(&op);
    // A store from the user's own helper chain (no Tenajlo dialog) isn't saved either.
    let worked = [
        ("protocol", "https"),
        ("host", "h"),
        ("username", "evan"),
        ("password", "from-gcm"),
    ];
    ask_credential(&t, &tok, CredentialOp::Store, &worked).await;
    ask(&t, &tok, "Username for 'https://h': ").await;
    ask_credential(&t, &tok, CredentialOp::Store, &worked).await;
    assert_eq!(saved.list(), vec![]);
}

#[tokio::test]
async fn remembered_passphrases_answer_without_a_dialog() {
    let key = "/home/evan/.ssh/id_ed25519";
    let prompt = format!("Enter passphrase for key '{key}': ");
    let answer = AuthAnswer {
        username: None,
        secret: "right".into(),
        remember: true,
    };
    let (t, saved, count, seen, _dir) = with_saved(Some(answer)).await;
    let op = t
        .register("repo", "op1", CancellationToken::new(), None)
        .unwrap();
    assert_eq!(
        ask(&t, &token_of(&op), &prompt).await.as_deref(),
        Some("right")
    );
    assert_eq!(count.load(Ordering::SeqCst), 1);
    drop(op);

    // Next time, even in the background, no dialog.
    let op = t
        .register("repo", "op2", CancellationToken::new(), None)
        .unwrap();
    op.set_interactive(false);
    let tok = token_of(&op);
    assert_eq!(ask(&t, &tok, &prompt).await.as_deref(), Some("right"));
    assert_eq!(count.load(Ordering::SeqCst), 1);

    // OpenSSH asks again: the saved one was wrong. It's forgotten, and a background
    // operation doesn't show a dialog.
    assert_eq!(ask(&t, &tok, &prompt).await, None);
    assert!(saved.passphrase(key).await.is_none());
    assert_eq!(count.load(Ordering::SeqCst), 1);
    assert_eq!(
        seen.lock().unwrap().as_slice(),
        [PromptKind::Passphrase {
            key: key.into(),
            retry: false
        }]
    );
}

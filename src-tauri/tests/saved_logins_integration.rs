//! Remembered HTTPS logins with real git, `tenajlo-askpass` and Forgejo (spec §6.2 step 3):
//! a login the user asks to remember is saved once git reports it worked, used without a
//! dialog next time, and forgotten when the server rejects it. See `dev/forgejo.yml`.
#![cfg(feature = "integration")]

use std::ffi::OsString;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tenajlo_lib::auth::accounts::AccountManager;
use tenajlo_lib::auth::prompt::{AuthAnswer, PromptKind};
use tenajlo_lib::auth::remote_auth::{self, AuthRequest, RemoteAuth};
use tenajlo_lib::auth::saved_secrets::SavedSecrets;
use tenajlo_lib::auth::secrets::Secret;
use tenajlo_lib::auth::trampoline::{PromptFn, Trampoline};
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::remote::{self, RemoteRun};
use tokio_util::sync::CancellationToken;

mod common;
use common::*;

/// Hides ~/.gitconfig and the system config, so the developer's own credential helper
/// (e.g. osxkeychain) neither answers nor stores anything.
fn isolated_env(dir: &Path) -> Vec<(OsString, OsString)> {
    let empty = dir.join("empty-gitconfig");
    std::fs::write(&empty, "").unwrap();
    vec![
        ("GIT_CONFIG_GLOBAL".into(), empty.into_os_string()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
    ]
}

#[tokio::test]
async fn remembered_login_is_saved_reused_and_forgotten_when_rejected() {
    let (mine, _) = private_repos().await;
    let url = format!("{BASE}/{mine}.git");
    let password = token(&["read:repository", "write:repository"]).await;
    let dir = tempfile::tempdir().unwrap();
    let env = isolated_env(dir.path());

    // No Tenajlo account for the server; the user answers the dialog and ticks Remember.
    let accounts = AccountManager::load(dir.path(), Arc::new(Keychain::default()));
    let saved = Arc::new(SavedSecrets::load(
        dir.path(),
        Arc::new(Keychain::default()),
    ));
    let prompts = Arc::new(AtomicUsize::new(0));
    let (count, pw) = (prompts.clone(), password.expose().to_owned());
    let prompt: PromptFn = Arc::new(move |p| {
        count.fetch_add(1, Ordering::SeqCst);
        assert!(matches!(p.kind, PromptKind::Credentials { .. }), "{p:?}");
        let answer = AuthAnswer {
            username: Some(USER.into()),
            secret: pw.clone(),
            remember: true,
        };
        Box::pin(async move { Some(answer) })
    });
    let trampoline = Trampoline::start_with_saved(prompt, saved.clone())
        .await
        .unwrap();
    let askpass = askpass();
    let auth = || async {
        let cancel = CancellationToken::new();
        let auth: RemoteAuth = remote_auth::prepare(
            &accounts,
            Some(&trampoline),
            Some(&askpass),
            AuthRequest {
                repo_id: "repo",
                op_id: &uuid::Uuid::new_v4().to_string(),
                cancel: cancel.clone(),
                remote_url: Some(&url),
                interactive: true,
            },
        )
        .await
        .unwrap();
        let mut run_env = auth.env.clone();
        run_env.extend(env.iter().cloned());
        let run = RemoteRun {
            env: run_env,
            config: auth.config.clone(),
            cancel,
            on_progress: Box::new(|_| {}),
        };
        (auth, run)
    };
    let git = resolve(None, None).unwrap();

    // 1. First clone: one dialog, and git's `store` saves the login.
    let work = dir.path().join("work");
    let (_a, run) = auth().await;
    clone(&git, &url, &work, run).await.unwrap();
    assert_eq!(prompts.load(Ordering::SeqCst), 1);
    let (user, secret) = saved.login("http", "localhost:3000", None).await.unwrap();
    assert_eq!((user.as_str(), secret.expose()), (USER, password.expose()));

    // 2. Fetch: answered by Tenajlo's helper, no dialog.
    let (_a, run) = auth().await;
    remote::fetch(&git, &work, "origin", run).await.unwrap();
    assert_eq!(prompts.load(Ordering::SeqCst), 1);

    // 3. The saved password stops working: git's `erase` makes Tenajlo forget it.
    saved
        .save_login(
            "http",
            "localhost:3000",
            USER,
            Secret::new("expired".into()),
        )
        .await
        .unwrap();
    let (_a, run) = auth().await;
    assert!(remote::fetch(&git, &work, "origin", run).await.is_err());
    assert!(saved.login("http", "localhost:3000", None).await.is_none());
}

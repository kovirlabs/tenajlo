//! HTTPS remotes against a real Forgejo with an account (spec §6.2, M4): repository listing,
//! clone of a private repo and push using the account's token through the real trampoline
//! and `tenajlo-askpass`, and diagnosis of rejected tokens. See `dev/forgejo.yml`.
#![cfg(feature = "integration")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tenajlo_lib::auth::accounts::AccountManager;
use tenajlo_lib::auth::remote_auth::{self, AuthDiagnosis, RemoteAuth};
use tenajlo_lib::auth::secrets::Secret;
use tenajlo_lib::auth::trampoline::{PromptFn, Trampoline};
use tenajlo_lib::forgejo::address::normalize_base_url;
use tenajlo_lib::forgejo::{repos, ForgejoClient};
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::error::{GitError, GitErrorKind};
use tenajlo_lib::git::exec::{Access, GitBinary, GitCommand};
use tenajlo_lib::git::remote::{self, RemoteRun};
use tokio_util::sync::CancellationToken;

mod common;
use common::*;

struct Env {
    _dir: tempfile::TempDir,
    work: PathBuf,
    accounts: AccountManager,
    trampoline: Trampoline,
    askpass: PathBuf,
    prompts: Arc<Mutex<Vec<String>>>,
    git: GitBinary,
}

async fn env_with(token: Secret) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let accounts = AccountManager::load(dir.path(), Arc::new(Keychain::default()));
    let user = ForgejoClient::new()
        .unwrap()
        .current_user(&normalize_base_url(BASE).unwrap(), &token)
        .await;
    let user = user.unwrap_or(tenajlo_lib::forgejo::ForgejoUser {
        login: USER.into(),
        full_name: String::new(),
        avatar_url: None,
        email: String::new(),
    });
    accounts.sign_in(BASE, user, token).await.unwrap();
    // Any prompt means the account credential wasn't used; record it and cancel.
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let seen = prompts.clone();
    let prompt: PromptFn = Arc::new(move |p| {
        seen.lock().unwrap().push(format!("{:?}", p.kind));
        Box::pin(async { None })
    });
    Env {
        work: dir.path().join("work"),
        _dir: dir,
        accounts,
        trampoline: Trampoline::start(prompt).await.unwrap(),
        askpass: askpass(),
        prompts,
        git: resolve(None, None).unwrap(),
    }
}

impl Env {
    async fn auth(&self, url: &str) -> (RemoteAuth, RemoteRun) {
        let cancel = CancellationToken::new();
        let auth = remote_auth::prepare(
            &self.accounts,
            Some(&self.trampoline),
            Some(&self.askpass),
            "repo",
            &uuid::Uuid::new_v4().to_string(),
            cancel.clone(),
            Some(url),
            true,
        )
        .await
        .unwrap();
        let run = RemoteRun {
            env: auth.env.clone(),
            config: auth.config.clone(),
            cancel,
            on_progress: Box::new(|_| {}),
        };
        (auth, run)
    }

    async fn commit(&self, repo: &Path, file: &str) {
        std::fs::write(repo.join(file), "change\n").unwrap();
        for args in [
            vec!["add", "-A"],
            vec![
                "-c",
                "user.name=T",
                "-c",
                "user.email=t@x",
                "commit",
                "-q",
                "-m",
                file,
            ],
        ] {
            GitCommand::new(args, Access::Mutating)
                .cwd(repo)
                .run(&self.git)
                .await
                .unwrap();
        }
    }
}

const FULL: [&str; 3] = ["read:user", "read:repository", "write:repository"];

#[tokio::test]
async fn account_token_lists_clones_and_pushes_private_repos() {
    let (mine, org) = private_repos().await;
    let tok = token(&FULL).await;
    let env = env_with(tok.clone()).await;

    let listed = repos::list_repositories(
        &ForgejoClient::new().unwrap(),
        &normalize_base_url(BASE).unwrap(),
        &tok,
    )
    .await
    .unwrap();
    let names: Vec<&str> = listed.iter().map(|r| r.full_name.as_str()).collect();
    assert!(
        names.contains(&mine.as_str()) && names.contains(&org.as_str()),
        "{names:?}"
    );

    let url = format!("{BASE}/{org}.git");
    let dest = env.work.join("Program Files").join("clone");
    let (auth, run) = env.auth(&url).await;
    assert_eq!(auth.config.len(), 2, "account helper configured");
    clone(&env.git, &url, &dest, run).await.unwrap();
    assert!(auth.account_used(), "git used the account token");
    assert!(dest.join("README.md").exists());

    env.commit(&dest, "pushed.txt").await;
    let (auth, run) = env.auth(&url).await;
    remote::push(&env.git, &dest, "origin", "main", "main", false, run)
        .await
        .unwrap();
    assert!(auth.account_used());
    assert!(
        env.prompts.lock().unwrap().is_empty(),
        "no prompts: {:?}",
        env.prompts
    );
}

#[tokio::test]
async fn rejected_and_read_only_tokens_are_diagnosed() {
    let (_, org) = private_repos().await;
    let url = format!("{BASE}/{org}.git");
    let forgejo = ForgejoClient::new().unwrap();

    // Read-only token: clone works, push is refused, and the token is still valid.
    let env = env_with(token(&["read:user", "read:repository"]).await).await;
    let dest = env.work.join("ro");
    let (_auth, run) = env.auth(&url).await; // keep alive: dropping revokes the token
    clone(&env.git, &url, &dest, run).await.unwrap();
    env.commit(&dest, "x.txt").await;
    let (auth, run) = env.auth(&url).await;
    let err = remote::push(&env.git, &dest, "origin", "main", "main", false, run)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            GitError::Failed {
                kind: GitErrorKind::AuthFailed,
                ..
            }
        ),
        "{err:?}"
    );
    assert!(auth.account_used());
    let account = auth.account.clone().unwrap();
    assert_eq!(
        remote_auth::diagnose(&env.accounts, &forgejo, &account).await,
        AuthDiagnosis::NoAccess
    );

    // Dead token: fetch fails, diagnosis marks the account.
    let dead = env_with(Secret::new("0".repeat(40))).await;
    let (auth, run) = dead.auth(&url).await;
    let err = remote::fetch(&dead.git, &dest, "origin", run)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            GitError::Failed {
                kind: GitErrorKind::AuthFailed,
                ..
            }
        ),
        "{err:?}"
    );
    let account = auth.account.clone().unwrap();
    assert_eq!(
        remote_auth::diagnose(&dead.accounts, &forgejo, &account).await,
        AuthDiagnosis::TokenRejected
    );
    assert!(dead.accounts.list()[0].needs_sign_in);
}

#[tokio::test]
async fn diverged_branches_merge_and_push_with_the_account() {
    let (mine, _) = private_repos().await;
    let env = env_with(token(&FULL).await).await;
    let url = format!("{BASE}/{mine}.git");
    let (a, b) = (env.work.join("a"), env.work.join("b"));
    for dest in [&a, &b] {
        let (_auth, run) = env.auth(&url).await;
        clone(&env.git, &url, dest, run).await.unwrap();
        for kv in [["user.name", "Test"], ["user.email", "t@example.com"]] {
            GitCommand::new(["config", kv[0], kv[1]], Access::Mutating)
                .cwd(dest)
                .run(&env.git)
                .await
                .unwrap();
        }
    }
    env.commit(&b, "teammate.txt").await;
    let (_auth, run) = env.auth(&url).await;
    remote::push(&env.git, &b, "origin", "main", "main", false, run)
        .await
        .unwrap();

    env.commit(&a, "mine.txt").await;
    let (_auth, run) = env.auth(&url).await;
    let err = remote::pull(&env.git, &a, run).await.unwrap_err();
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
    let (_auth, run) = env.auth(&url).await;
    remote::pull_merge(&env.git, &a, run).await.unwrap();
    assert!(a.join("teammate.txt").exists());
    let (_auth, run) = env.auth(&url).await;
    remote::push(&env.git, &a, "origin", "main", "main", false, run)
        .await
        .unwrap();
    assert!(env.prompts.lock().unwrap().is_empty());
}

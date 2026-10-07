//! HTTPS remotes against a real Forgejo with an account (spec §6.2, M4): repository listing,
//! clone of a private repo and push using the account's token through the real trampoline
//! and `tenajlo-askpass`, and diagnosis of rejected tokens. See `dev/forgejo.yml`.
#![cfg(feature = "integration")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tenajlo_lib::auth::accounts::AccountManager;
use tenajlo_lib::auth::remote_auth::{self, AuthDiagnosis, RemoteAuth};
use tenajlo_lib::auth::secrets::{Secret, SecretError, SecretStore};
use tenajlo_lib::auth::trampoline::{PromptFn, Trampoline};
use tenajlo_lib::forgejo::address::normalize_base_url;
use tenajlo_lib::forgejo::{repos, ForgejoClient};
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::error::{GitError, GitErrorKind};
use tenajlo_lib::git::exec::{Access, GitBinary, GitCommand};
use tenajlo_lib::git::remote::{self, RemoteRun};
use tokio_util::sync::CancellationToken;

const BASE: &str = "http://localhost:3000";
const USER: &str = "tenajlo";
const PASSWORD: &str = "tenajlo-dev-password";

#[derive(Default)]
struct Keychain(Mutex<HashMap<String, Secret>>);

impl SecretStore for Keychain {
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError> {
        Ok(self.0.lock().unwrap().get(key).cloned())
    }
    fn set(&self, key: &str, secret: &Secret) -> Result<(), SecretError> {
        self.0.lock().unwrap().insert(key.into(), secret.clone());
        Ok(())
    }
    fn delete(&self, key: &str) -> Result<(), SecretError> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

/// Admin-side setup with basic auth (not what the app does).
async fn api(method: reqwest::Method, path: &str, body: serde_json::Value) -> serde_json::Value {
    let res = reqwest::Client::new()
        .request(method, format!("{BASE}/api/v1/{path}"))
        .basic_auth(USER, Some(PASSWORD))
        .json(&body)
        .send()
        .await
        .unwrap();
    let status = res.status();
    let json = res.json().await.unwrap_or(serde_json::Value::Null);
    assert!(
        status.is_success() || status.as_u16() == 422 || status.as_u16() == 409,
        "{path}: {status} {json}"
    );
    json
}

async fn token(scopes: &[&str]) -> Secret {
    let name = format!("it-{}", uuid::Uuid::new_v4());
    let created = api(
        reqwest::Method::POST,
        &format!("users/{USER}/tokens"),
        serde_json::json!({ "name": name, "scopes": scopes }),
    )
    .await;
    Secret::new(created["sha1"].as_str().unwrap().to_owned())
}

/// Builds `tenajlo-askpass` (it's a separate binary crate) and returns its path.
fn askpass() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let status = std::process::Command::new(env!("CARGO"))
        .args(["build", "-q", "-p", "tenajlo-askpass"])
        .current_dir(&root)
        .status()
        .unwrap();
    assert!(status.success());
    let exe = if cfg!(windows) {
        "tenajlo-askpass.exe"
    } else {
        "tenajlo-askpass"
    };
    root.join("target/debug").join(exe)
}

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

async fn private_repos() -> (String, String) {
    let name = format!("it-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let org = format!("org-{name}");
    api(
        reqwest::Method::POST,
        "user/repos",
        serde_json::json!({ "name": name, "private": true, "auto_init": true }),
    )
    .await;
    api(
        reqwest::Method::POST,
        "orgs",
        serde_json::json!({ "username": org }),
    )
    .await;
    api(
        reqwest::Method::POST,
        &format!("orgs/{org}/repos"),
        serde_json::json!({ "name": name, "private": true, "auto_init": true }),
    )
    .await;
    (format!("{USER}/{name}"), format!("{org}/{name}"))
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

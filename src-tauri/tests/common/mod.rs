//! Shared helpers for tests against the dev Forgejo (`dev/forgejo.yml`) or Gitea
//! (`dev/gitea.yml`). The server comes from `TENAJLO_FORGEJO_URL` (default
//! `http://localhost:3000`) and its SSH port from `TENAJLO_FORGEJO_SSH_PORT` (default 2222).
#![allow(dead_code)] // each test binary uses a different subset

use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use tenajlo_lib::auth::secrets::{Secret, SecretError, SecretStore};

/// The server's base URL without a trailing slash. Formats as a string (`"{BASE}/x"`) and
/// derefs to `&str` (`&BASE`).
pub const BASE: Base = Base;

pub struct Base;

impl Base {
    pub fn as_str(&self) -> &'static str {
        static URL: OnceLock<String> = OnceLock::new();
        URL.get_or_init(|| {
            std::env::var("TENAJLO_FORGEJO_URL")
                .unwrap_or_else(|_| "http://localhost:3000".into())
                .trim_end_matches('/')
                .to_owned()
        })
    }
}

impl std::ops::Deref for Base {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Base {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `host:port` of the server, as git's credential protocol names it.
pub fn host() -> String {
    let url = reqwest::Url::parse(&BASE).unwrap();
    format!(
        "{}:{}",
        url.host_str().unwrap(),
        url.port_or_known_default().unwrap()
    )
}

/// The server's SSH port on localhost.
pub fn ssh_port() -> u16 {
    std::env::var("TENAJLO_FORGEJO_SSH_PORT").map_or(2222, |p| p.parse().unwrap())
}

/// `{BASE}/{repo}.git` with a username and password in it (test setup only).
pub fn authed_url(user: &str, password: &str, repo: &str) -> String {
    let mut url = reqwest::Url::parse(&format!("{BASE}/{repo}.git")).unwrap();
    url.set_username(user).unwrap();
    url.set_password(Some(password)).unwrap();
    url.to_string()
}
pub const USER: &str = "tenajlo";
pub const PASSWORD: &str = "tenajlo-dev-password";

#[derive(Default)]
pub struct Keychain(Mutex<HashMap<String, Secret>>);

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
pub async fn api(
    method: reqwest::Method,
    path: &str,
    body: serde_json::Value,
) -> serde_json::Value {
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

pub async fn token(scopes: &[&str]) -> Secret {
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
pub fn askpass() -> PathBuf {
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

/// A private repo owned by the test user and one in a fresh org: `(user/name, org/name)`.
pub async fn private_repos() -> (String, String) {
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

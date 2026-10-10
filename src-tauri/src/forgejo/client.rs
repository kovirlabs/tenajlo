//! HTTP client for the Forgejo REST API.
//!
//! TLS goes through `rustls-platform-verifier`, so the OS trust store (including a
//! domain-pushed internal CA) decides. Verification is never weakened (CLAUDE.md rule 4).

use std::time::Duration;

use reqwest::{redirect, StatusCode, Url};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::address::{base_string, is_loopback, join};
use super::ForgejoError;
use crate::auth::secrets::Secret;

/// Spec §7: every API request times out after 15 s.
const TIMEOUT: Duration = Duration::from_secs(15);
const MAX_REDIRECTS: usize = 5;
/// Error bodies are truncated before they reach error details.
const MAX_ERROR_BODY: usize = 500;

/// Scopes Tenajlo asks for when creating a token (spec §6.4 step 3).
pub const REQUIRED_SCOPES: [&str; 3] = ["read:user", "read:repository", "write:repository"];

/// A reachable Forgejo (or Gitea-compatible) server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ServerInfo {
    /// Normalized address, e.g. `https://git.example.com`.
    pub base_url: String,
    pub version: String,
    /// Where the user creates a personal access token.
    pub token_settings_url: String,
    pub required_scopes: Vec<String>,
}

/// The signed-in user (`GET /api/v1/user`).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ForgejoUser {
    pub login: String,
    #[serde(default)]
    pub full_name: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    /// May be a placeholder (`…@noreply…`) when the user keeps their email private.
    #[serde(default)]
    pub email: String,
}

#[derive(Deserialize)]
struct VersionResponse {
    version: String,
}

/// Shared HTTP client. Cheap to clone.
#[derive(Clone)]
pub struct ForgejoClient {
    http: reqwest::Client,
}

impl ForgejoClient {
    /// Builds the client: 15 s timeout, no redirects that downgrade to plain http.
    pub fn new() -> Result<Self, ForgejoError> {
        let policy = redirect::Policy::custom(|attempt| {
            let url = attempt.url();
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.stop()
            } else if url.scheme() == "https" || is_loopback(url) {
                // reqwest drops the Authorization header on cross-host redirects.
                attempt.follow()
            } else {
                attempt.stop()
            }
        });
        let http = reqwest::Client::builder()
            .timeout(TIMEOUT)
            .redirect(policy)
            .user_agent(concat!("Tenajlo/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| ForgejoError::Other(error_chain(&e)))?;
        Ok(Self { http })
    }

    /// `GET /api/v1/version`: checks the server is reachable, trusted and Forgejo-compatible.
    pub async fn server_info(&self, base: &Url) -> Result<ServerInfo, ForgejoError> {
        let res: VersionResponse = match self.get_json(base, "api/v1/version", None).await {
            Ok(v) => v,
            Err(ForgejoError::Http { status: 404, .. }) => {
                return Err(ForgejoError::NotForgejo("no /api/v1/version".into()))
            }
            Err(e) => return Err(e),
        };
        Ok(ServerInfo {
            base_url: base_string(base),
            version: res.version,
            token_settings_url: join(base, "user/settings/applications").into(),
            required_scopes: REQUIRED_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
        })
    }

    /// `GET /api/v1/user` with the token: who it belongs to.
    pub async fn current_user(
        &self,
        base: &Url,
        token: &Secret,
    ) -> Result<ForgejoUser, ForgejoError> {
        self.get_json(base, "api/v1/user", Some(token)).await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        base: &Url,
        path: &str,
        token: Option<&Secret>,
    ) -> Result<T, ForgejoError> {
        self.get_url(join(base, path), token).await
    }

    /// `GET url` with an optional token, parsing JSON. `url` must be on the server.
    pub(crate) async fn get_url<T: DeserializeOwned>(
        &self,
        url: Url,
        token: Option<&Secret>,
    ) -> Result<T, ForgejoError> {
        self.send(self.http.get(url.clone()), &url, token).await
    }

    /// `POST url` with a JSON `body` and the token, parsing the JSON reply.
    pub(crate) async fn post_url<B: Serialize + ?Sized, T: DeserializeOwned>(
        &self,
        url: Url,
        token: &Secret,
        body: &B,
    ) -> Result<T, ForgejoError> {
        let req = self.http.post(url.clone()).json(body);
        self.send(req, &url, Some(token)).await
    }

    async fn send<T: DeserializeOwned>(
        &self,
        req: reqwest::RequestBuilder,
        url: &Url,
        token: Option<&Secret>,
    ) -> Result<T, ForgejoError> {
        let path = url.path().to_owned();
        let mut req = req.header(reqwest::header::ACCEPT, "application/json");
        if let Some(token) = token {
            let mut value =
                reqwest::header::HeaderValue::from_str(&format!("token {}", token.expose()))
                    .map_err(|_| ForgejoError::Unauthorized)?;
            value.set_sensitive(true);
            req = req.header(reqwest::header::AUTHORIZATION, value);
        }
        let res = req.send().await.map_err(classify)?;
        let status = res.status();
        if !status.is_success() {
            let body = res.text().await.unwrap_or_default();
            return Err(status_error(status, &body));
        }
        let bytes = res.bytes().await.map_err(classify)?;
        serde_json::from_slice(&bytes)
            .map_err(|e| ForgejoError::NotForgejo(format!("unexpected response from {path}: {e}")))
    }
}

fn status_error(status: StatusCode, body: &str) -> ForgejoError {
    let body: String = body.chars().take(MAX_ERROR_BODY).collect();
    match status {
        StatusCode::UNAUTHORIZED => ForgejoError::Unauthorized,
        // Forgejo: "token does not have at least one of required scope(s): [read:user]"
        StatusCode::FORBIDDEN if body.contains("scope") => ForgejoError::MissingScope(body),
        _ => ForgejoError::Http {
            status: status.as_u16(),
            body,
        },
    }
}

fn classify(err: reqwest::Error) -> ForgejoError {
    let chain = error_chain(&err);
    if err.is_timeout() {
        ForgejoError::TimedOut
    } else if is_tls_failure(&chain) {
        ForgejoError::TlsUntrusted(chain)
    } else if err.is_connect() {
        ForgejoError::Unreachable(chain)
    } else {
        ForgejoError::Other(chain)
    }
}

/// rustls doesn't surface a typed error through reqwest; match the certificate wording.
fn is_tls_failure(chain: &str) -> bool {
    let c = chain.to_ascii_lowercase();
    ["certificate", "unknownissuer"]
        .iter()
        .any(|needle| c.contains(needle))
}

/// The error and all its sources, joined with `: `.
fn error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut out = err.to_string();
    let mut source = err.source();
    while let Some(s) = source {
        let text = s.to_string();
        if !out.contains(&text) {
            out.push_str(": ");
            out.push_str(&text);
        }
        source = s.source();
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::forgejo::address::normalize_base_url;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// A canned response: (status line, JSON body).
    pub(crate) type Route = (&'static str, &'static str, String);

    /// Minimal HTTP/1.1 server for tests. Answers `routes` by path; records request heads.
    pub(crate) async fn fake_server(routes: Vec<Route>) -> (Url, Arc<Mutex<Vec<String>>>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = normalize_base_url(&format!(
            "http://127.0.0.1:{}",
            listener.local_addr().unwrap().port()
        ))
        .unwrap();
        let seen = Arc::new(Mutex::new(Vec::new()));
        let log = seen.clone();
        tokio::spawn(async move {
            loop {
                let Ok((mut s, _)) = listener.accept().await else {
                    return;
                };
                let mut buf = vec![0u8; 8192];
                let n = s.read(&mut buf).await.unwrap_or(0);
                let head = String::from_utf8_lossy(&buf[..n]).to_string();
                let path = head.split_whitespace().nth(1).unwrap_or("").to_owned();
                log.lock().unwrap().push(head);
                let (status, body) = routes
                    .iter()
                    .find(|(p, _, _)| *p == path)
                    .map(|(_, st, b)| (*st, b.clone()))
                    .unwrap_or(("404 Not Found", "{}".into()));
                let reply = format!(
                    "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = s.write_all(reply.as_bytes()).await;
            }
        });
        (base, seen)
    }

    #[tokio::test]
    async fn server_info_reads_version() {
        let (base, _) = fake_server(vec![(
            "/api/v1/version",
            "200 OK",
            r#"{"version":"11.0.1+gitea-1.22.0"}"#.into(),
        )])
        .await;
        let info = ForgejoClient::new()
            .unwrap()
            .server_info(&base)
            .await
            .unwrap();
        assert_eq!(info.version, "11.0.1+gitea-1.22.0");
        assert_eq!(
            info.token_settings_url,
            format!("{}/user/settings/applications", base_string(&base))
        );
    }

    #[tokio::test]
    async fn non_forgejo_servers_are_detected() {
        let (base, _) = fake_server(vec![("/api/v1/version", "200 OK", "<html>".into())]).await;
        let client = ForgejoClient::new().unwrap();
        assert!(matches!(
            client.server_info(&base).await,
            Err(ForgejoError::NotForgejo(_))
        ));
        let (base, _) = fake_server(vec![]).await;
        assert!(matches!(
            client.server_info(&base).await,
            Err(ForgejoError::NotForgejo(_))
        ));
    }

    #[tokio::test]
    async fn current_user_sends_token_header() {
        let (base, seen) = fake_server(vec![(
            "/api/v1/user",
            "200 OK",
            r#"{"login":"evan","full_name":"Évan G","avatar_url":"https://h/a.png","email":"e@x"}"#
                .into(),
        )])
        .await;
        let user = ForgejoClient::new()
            .unwrap()
            .current_user(&base, &Secret::new("pat-123".into()))
            .await
            .unwrap();
        assert_eq!(user.login, "evan");
        assert_eq!(user.full_name, "Évan G");
        assert_eq!(user.email, "e@x");
        let head = seen.lock().unwrap()[0].to_ascii_lowercase();
        assert!(head.contains("authorization: token pat-123"), "{head}");
    }

    #[tokio::test]
    async fn auth_failures_are_classified() {
        let (base, _) = fake_server(vec![(
            "/api/v1/user",
            "401 Unauthorized",
            r#"{"message":"token is required"}"#.into(),
        )])
        .await;
        let client = ForgejoClient::new().unwrap();
        let tok = Secret::new("x".into());
        assert!(matches!(
            client.current_user(&base, &tok).await,
            Err(ForgejoError::Unauthorized)
        ));

        let (base, _) = fake_server(vec![(
            "/api/v1/user",
            "403 Forbidden",
            r#"{"message":"token does not have at least one of required scope(s): [read:user]"}"#
                .into(),
        )])
        .await;
        assert!(matches!(
            client.current_user(&base, &tok).await,
            Err(ForgejoError::MissingScope(_))
        ));
    }

    #[tokio::test]
    async fn connection_refused_is_unreachable() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let base = normalize_base_url(&format!("http://127.0.0.1:{port}")).unwrap();
        assert!(matches!(
            ForgejoClient::new().unwrap().server_info(&base).await,
            Err(ForgejoError::Unreachable(_))
        ));
    }

    #[test]
    fn tls_wording_is_recognized() {
        assert!(is_tls_failure(
            "error sending request: client error (Connect): invalid peer certificate: UnknownIssuer"
        ));
        assert!(!is_tls_failure("dns error: failed to lookup address"));
    }
}

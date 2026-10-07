//! Credentials for one remote git operation (spec §6.2), and what an auth failure meant.
//!
//! When an account's server hosts the remote URL, git gets the account's login and PAT
//! through the trampoline's credential helper. Otherwise the user's own helper chain runs
//! untouched, with the askpass prompt as the fallback.

use std::ffi::OsString;
use std::path::Path;

use reqwest::Url;
use tokio_util::sync::CancellationToken;

use super::accounts::AccountManager;
use super::credential_helper::AccountCredential;
use super::secrets::Secret;
use super::trampoline::{OpToken, Trampoline};
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::{ForgejoClient, ForgejoError};
use crate::store::accounts::AccountEntry;

/// Auth wiring for one operation. Keep it alive until git exits: dropping it revokes the
/// trampoline token.
#[derive(Debug, Default)]
pub struct RemoteAuth {
    token: Option<OpToken>,
    /// The account whose credential git may use, if any.
    pub account: Option<AccountEntry>,
    /// Environment for git (askpass trampoline).
    pub env: Vec<(OsString, OsString)>,
    /// `-c` flags for git (credential helper override, account hosts only).
    pub config: Vec<String>,
}

impl RemoteAuth {
    /// True if git used the account credential, so an auth failure is about that token.
    pub fn account_used(&self) -> bool {
        self.token.as_ref().is_some_and(OpToken::account_used)
    }
}

/// Why credentials couldn't be prepared.
#[derive(Debug, thiserror::Error)]
pub enum PrepareError {
    /// The account's token was rejected earlier or is missing from the keychain.
    #[error("sign-in required for {}", .0.base_url)]
    SignInRequired(Box<AccountEntry>),
    #[error("could not register with the askpass trampoline: {0}")]
    Trampoline(#[from] std::io::Error),
}

/// What an `AuthFailed` git error meant for an account's token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthDiagnosis {
    /// The server rejects the token (401). The account is now marked "needs sign-in".
    TokenRejected,
    /// The token works, so the account lacks access to this repository (or a scope).
    NoAccess,
    /// The check itself failed (network, keychain); report the original error.
    Unknown,
}

/// Registers the operation with the trampoline and, if an account hosts `remote_url`,
/// arranges for git to use its token.
#[allow(clippy::too_many_arguments)]
pub async fn prepare(
    accounts: &AccountManager,
    trampoline: Option<&Trampoline>,
    askpass: Option<&Path>,
    repo_id: &str,
    op_id: &str,
    cancel: CancellationToken,
    remote_url: Option<&str>,
    interactive: bool,
) -> Result<RemoteAuth, PrepareError> {
    let account = remote_url.and_then(|u| accounts.for_remote(u));
    let (Some(trampoline), Some(askpass)) = (trampoline, askpass) else {
        // Already logged at startup; git falls back to the user's own helpers.
        return Ok(RemoteAuth::default());
    };

    let credential = match &account {
        Some(a) if a.needs_sign_in => {
            return Err(PrepareError::SignInRequired(Box::new(a.clone())))
        }
        Some(a) => match accounts.token(a).await {
            Ok(Some(token)) => account_credential(a, token),
            Ok(None) => {
                if let Err(e) = accounts.mark_needs_sign_in(a.id) {
                    tracing::warn!(error = %e, "could not record that sign-in is required");
                }
                return Err(PrepareError::SignInRequired(Box::new(a.clone())));
            }
            Err(e) => {
                // Don't block the operation: the user's helpers or a prompt can still work.
                tracing::warn!(error = %e, "keychain unavailable; not using the account token");
                None
            }
        },
        None => None,
    };
    let has_credential = credential.is_some();
    let token = trampoline.register(repo_id, op_id, cancel, credential)?;
    token.set_interactive(interactive);
    Ok(RemoteAuth {
        env: token.env(askpass),
        config: token.config(askpass),
        account: account.filter(|_| has_credential),
        token: Some(token),
    })
}

/// Checks the account's token against `GET /api/v1/user` after git reported `AuthFailed`
/// (spec §6.2: verify, then re-auth; never delete the token, never loop).
pub async fn diagnose(
    accounts: &AccountManager,
    forgejo: &ForgejoClient,
    account: &AccountEntry,
) -> AuthDiagnosis {
    let (Ok(Some(token)), Ok(base)) = (
        accounts.token(account).await,
        normalize_base_url(&account.base_url),
    ) else {
        return AuthDiagnosis::Unknown;
    };
    match forgejo.current_user(&base, &token).await {
        Err(ForgejoError::Unauthorized) => {
            if let Err(e) = accounts.mark_needs_sign_in(account.id) {
                tracing::warn!(error = %e, "could not record that sign-in is required");
            }
            AuthDiagnosis::TokenRejected
        }
        // A token without read:user is still a valid token.
        Ok(_) | Err(ForgejoError::MissingScope(_)) => AuthDiagnosis::NoAccess,
        Err(_) => AuthDiagnosis::Unknown,
    }
}

fn account_credential(account: &AccountEntry, token: Secret) -> Option<AccountCredential> {
    let url = Url::parse(&account.base_url).ok()?;
    let host = url.host_str()?;
    let host = match url.port() {
        Some(port) => format!("{host}:{port}"),
        None => host.to_owned(),
    };
    Some(AccountCredential {
        protocol: url.scheme().to_owned(),
        host,
        login: account.login.clone(),
        token,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::secrets::{MemoryStore, SecretStore};
    use crate::auth::trampoline::PromptFn;
    use crate::forgejo::client::tests::fake_server;
    use crate::forgejo::ForgejoUser;
    use std::sync::Arc;

    const REMOTE: &str = "https://git.example.com/team/plc.git";

    async fn setup(
        base: &str,
    ) -> (
        tempfile::TempDir,
        Arc<MemoryStore>,
        AccountManager,
        Trampoline,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemoryStore::default());
        let accounts = AccountManager::load(dir.path(), secrets.clone());
        let user = ForgejoUser {
            login: "evan".into(),
            full_name: String::new(),
            avatar_url: None,
            email: String::new(),
        };
        accounts
            .sign_in(base, user, Secret::new("pat".into()))
            .await
            .unwrap();
        let never: PromptFn = Arc::new(|_| Box::pin(async { None }));
        let t = Trampoline::start(never).await.unwrap();
        (dir, secrets, accounts, t)
    }

    async fn prep(
        accounts: &AccountManager,
        t: &Trampoline,
        url: &str,
    ) -> Result<RemoteAuth, PrepareError> {
        let askpass = Path::new("/opt/tenajlo-askpass");
        prepare(
            accounts,
            Some(t),
            Some(askpass),
            "r",
            "op",
            CancellationToken::new(),
            Some(url),
            true,
        )
        .await
    }

    #[tokio::test]
    async fn account_hosts_get_the_helper_override() {
        let (_d, _s, accounts, t) = setup("https://git.example.com").await;
        let auth = prep(&accounts, &t, REMOTE).await.unwrap();
        assert_eq!(auth.config.len(), 2);
        assert!(auth.account.is_some());
        assert!(!auth.env.is_empty());

        let other = prep(&accounts, &t, "https://example.com/x.git")
            .await
            .unwrap();
        assert!(other.config.is_empty(), "user's helpers left alone");
        assert!(other.account.is_none());
        assert!(!other.env.is_empty(), "askpass prompt still available");
    }

    #[tokio::test]
    async fn missing_or_rejected_token_requires_sign_in() {
        let (_d, secrets, accounts, t) = setup("https://git.example.com").await;
        secrets.delete("https://git.example.com|evan").unwrap();
        assert!(matches!(
            prep(&accounts, &t, REMOTE).await,
            Err(PrepareError::SignInRequired(_))
        ));
        assert!(accounts.list()[0].needs_sign_in);
        // Stays required (no network attempt) until the user signs in again.
        secrets
            .set("https://git.example.com|evan", &Secret::new("x".into()))
            .unwrap();
        assert!(matches!(
            prep(&accounts, &t, REMOTE).await,
            Err(PrepareError::SignInRequired(_))
        ));
    }

    #[tokio::test]
    async fn locked_keychain_falls_back_to_user_helpers() {
        let (_d, secrets, accounts, t) = setup("https://git.example.com").await;
        secrets
            .fail
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let auth = prep(&accounts, &t, REMOTE).await.unwrap();
        assert!(auth.config.is_empty());
        assert!(auth.account.is_none());
    }

    #[test]
    fn credential_host_includes_explicit_port() {
        let entry = |base: &str| AccountEntry {
            id: uuid::Uuid::new_v4(),
            kind: crate::store::accounts::AccountKind::Forgejo,
            base_url: base.into(),
            login: "evan".into(),
            display_name: "Evan".into(),
            avatar_url: None,
            token_scopes: None,
            ssh_host: None,
            needs_sign_in: false,
        };
        let c =
            account_credential(&entry("https://h:3000/forgejo"), Secret::new("t".into())).unwrap();
        assert_eq!((c.protocol.as_str(), c.host.as_str()), ("https", "h:3000"));
        let c = account_credential(&entry("https://h"), Secret::new("t".into())).unwrap();
        assert_eq!(c.host, "h");
    }

    #[tokio::test]
    async fn diagnosis_distinguishes_dead_tokens_from_missing_access() {
        let (base, _) = fake_server(vec![("/api/v1/user", "401 Unauthorized", "{}".into())]).await;
        let base = crate::forgejo::address::base_string(&base);
        let (_d, _s, accounts, _t) = setup(&base).await;
        let entry = accounts.for_remote(&format!("{base}/team/x.git")).unwrap();
        let client = ForgejoClient::new().unwrap();
        assert_eq!(
            diagnose(&accounts, &client, &entry).await,
            AuthDiagnosis::TokenRejected
        );
        assert!(accounts.list()[0].needs_sign_in);

        let (base, _) = fake_server(vec![(
            "/api/v1/user",
            "200 OK",
            r#"{"login":"evan"}"#.into(),
        )])
        .await;
        let base = crate::forgejo::address::base_string(&base);
        let (_d, _s, accounts, _t) = setup(&base).await;
        let entry = accounts.for_remote(&format!("{base}/team/x.git")).unwrap();
        assert_eq!(
            diagnose(&accounts, &client, &entry).await,
            AuthDiagnosis::NoAccess
        );
        assert!(!accounts.list()[0].needs_sign_in);
    }
}

//! The signed-in user's SSH keys on Forgejo (spec §6.3 key management, §7).
//!
//! Adding a key needs the `write:user` token scope, which Tenajlo didn't ask for before v1.1.
//! [`can_add_keys`] finds out whether a token has it without changing anything.

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::address::join;
use super::repos::all_pages;
use super::{ForgejoClient, ForgejoError};
use crate::auth::secrets::Secret;

/// Token scopes needed to add SSH keys, plus the ones every Tenajlo account needs for
/// repositories. Shown when the user creates a replacement token.
pub const KEY_UPLOAD_SCOPES: [&str; 3] = ["write:user", "read:repository", "write:repository"];

/// An SSH key on the user's Forgejo account.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ServerKey {
    pub id: i64,
    #[serde(default)]
    pub title: String,
    /// `SHA256:…`, the same form as [`crate::auth::ssh_keys::LocalSshKey::fingerprint`].
    #[serde(default)]
    pub fingerprint: String,
}

#[derive(Serialize)]
struct NewKey<'a> {
    title: &'a str,
    key: &'a str,
    read_only: bool,
}

/// Why adding a key failed.
#[derive(Debug, thiserror::Error)]
pub enum AddKeyError {
    /// The token lacks `write:user`.
    #[error("the token can't add SSH keys")]
    NotAllowed,
    /// This exact key is already on a Forgejo account (this user's or someone else's).
    #[error("the key is already in use on the server")]
    AlreadyUsed,
    #[error(transparent)]
    Api(#[from] ForgejoError),
}

/// The user's SSH keys (`GET /api/v1/user/keys`, needs `read:user`).
pub async fn list_keys(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
) -> Result<Vec<ServerKey>, ForgejoError> {
    all_pages(client, base, &["user", "keys"], token).await
}

/// Whether `token` may add SSH keys. Posts an empty key: Forgejo checks scopes before it
/// validates the body, so this gets 403 "required scope" without the scope and 422
/// "Title required" with it. Nothing is created either way.
pub async fn can_add_keys(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
) -> Result<bool, ForgejoError> {
    let probe = client
        .post_url::<_, serde_json::Value>(keys_url(base), token, &serde_json::json!({}))
        .await;
    match probe {
        Err(ForgejoError::MissingScope(_)) => Ok(false),
        Err(ForgejoError::Http { status: 422, .. }) => Ok(true),
        Err(e) => Err(e),
        // A server that accepted an empty key is too unusual to trust with a real one.
        Ok(_) => Err(ForgejoError::Other(
            "the server accepted an empty SSH key".into(),
        )),
    }
}

/// Adds `public_key` (one OpenSSH line) to the account as `title`.
pub async fn add_key(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    title: &str,
    public_key: &str,
) -> Result<ServerKey, AddKeyError> {
    let body = NewKey {
        title,
        key: public_key,
        read_only: false,
    };
    client
        .post_url(keys_url(base), token, &body)
        .await
        .map_err(|e| match e {
            ForgejoError::MissingScope(_) => AddKeyError::NotAllowed,
            // "Key content has been used as non-deploy key"
            ForgejoError::Http { status: 422, body } if body.contains("has been used") => {
                AddKeyError::AlreadyUsed
            }
            other => AddKeyError::Api(other),
        })
}

fn keys_url(base: &Url) -> Url {
    join(base, "api/v1/user/keys")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forgejo::client::tests::fake_server;

    const SCOPE_403: &str =
        r#"{"message":"token does not have at least one of required scope(s): [write:user]"}"#;

    fn tok() -> Secret {
        Secret::new("t".into())
    }

    #[tokio::test]
    async fn probe_tells_scopes_apart() {
        let client = ForgejoClient::new().unwrap();
        let (base, seen) = fake_server(vec![(
            "/api/v1/user/keys",
            "422 Unprocessable Entity",
            r#"{"message":"[Title]: Required"}"#.into(),
        )])
        .await;
        assert!(can_add_keys(&client, &base, &tok()).await.unwrap());
        assert!(seen.lock().unwrap()[0].starts_with("POST "));

        let (base, _) = fake_server(vec![(
            "/api/v1/user/keys",
            "403 Forbidden",
            SCOPE_403.into(),
        )])
        .await;
        assert!(!can_add_keys(&client, &base, &tok()).await.unwrap());

        let (base, _) =
            fake_server(vec![("/api/v1/user/keys", "401 Unauthorized", "{}".into())]).await;
        assert!(matches!(
            can_add_keys(&client, &base, &tok()).await,
            Err(ForgejoError::Unauthorized)
        ));
    }

    #[tokio::test]
    async fn add_key_maps_refusals() {
        let client = ForgejoClient::new().unwrap();
        let (base, seen) = fake_server(vec![(
            "/api/v1/user/keys",
            "201 Created",
            r#"{"id":20,"title":"laptop","fingerprint":"SHA256:abc","key":"ssh-ed25519 AAAA"}"#
                .into(),
        )])
        .await;
        let key = add_key(&client, &base, &tok(), "laptop", "ssh-ed25519 AAAA x")
            .await
            .unwrap();
        assert_eq!((key.id, key.fingerprint.as_str()), (20, "SHA256:abc"));
        assert!(seen.lock().unwrap()[0].starts_with("POST /api/v1/user/keys "));

        let (base, _) = fake_server(vec![(
            "/api/v1/user/keys",
            "422 Unprocessable Entity",
            r#"{"message":"Key content has been used as non-deploy key"}"#.into(),
        )])
        .await;
        assert!(matches!(
            add_key(&client, &base, &tok(), "t", "k").await,
            Err(AddKeyError::AlreadyUsed)
        ));

        let (base, _) = fake_server(vec![(
            "/api/v1/user/keys",
            "403 Forbidden",
            SCOPE_403.into(),
        )])
        .await;
        assert!(matches!(
            add_key(&client, &base, &tok(), "t", "k").await,
            Err(AddKeyError::NotAllowed)
        ));
    }

    #[tokio::test]
    async fn lists_keys() {
        let (base, _) = fake_server(vec![(
            "/api/v1/user/keys?limit=50&page=1",
            "200 OK",
            r#"[{"id":1,"title":"a","fingerprint":"SHA256:x","key":"k","user":{}}]"#.into(),
        )])
        .await;
        let keys = list_keys(&ForgejoClient::new().unwrap(), &base, &tok())
            .await
            .unwrap();
        assert_eq!(keys[0].fingerprint, "SHA256:x");
    }
}

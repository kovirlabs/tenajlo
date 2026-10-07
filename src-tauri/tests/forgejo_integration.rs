//! Forgejo client against a real server (spec §11). See `dev/forgejo.yml` for setup.
//! Override the server with `TENAJLO_FORGEJO_URL` (default `http://localhost:3000`).
#![cfg(feature = "integration")]

use tenajlo_lib::auth::secrets::Secret;
use tenajlo_lib::forgejo::address::normalize_base_url;
use tenajlo_lib::forgejo::{ForgejoClient, ForgejoError};

const USER: &str = "tenajlo";
const PASSWORD: &str = "tenajlo-dev-password";

fn base() -> reqwest::Url {
    let url = std::env::var("TENAJLO_FORGEJO_URL").unwrap_or("http://localhost:3000".into());
    normalize_base_url(&url).unwrap()
}

/// Creates a fresh PAT for the seeded user with Tenajlo's required scopes.
async fn new_token(scopes: &[&str]) -> Secret {
    #[derive(serde::Deserialize)]
    struct Created {
        sha1: String,
    }
    let name = format!("tenajlo-test-{}", uuid::Uuid::new_v4());
    let url = format!(
        "{}/api/v1/users/{USER}/tokens",
        base().as_str().trim_end_matches('/')
    );
    let created: Created = reqwest::Client::new()
        .post(url)
        .basic_auth(USER, Some(PASSWORD))
        .json(&serde_json::json!({ "name": name, "scopes": scopes }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .expect("token creation failed: did you run dev/seed-forgejo.sh?")
        .json()
        .await
        .unwrap();
    Secret::new(created.sha1)
}

#[tokio::test]
async fn sign_in_flow_against_forgejo() {
    let client = ForgejoClient::new().unwrap();
    let info = client.server_info(&base()).await.unwrap();
    assert!(!info.version.is_empty());

    let token = new_token(&["read:user", "read:repository", "write:repository"]).await;
    let user = client.current_user(&base(), &token).await.unwrap();
    assert_eq!(user.login, USER);
}

#[tokio::test]
async fn bad_and_underscoped_tokens_are_classified() {
    let client = ForgejoClient::new().unwrap();
    let bogus = Secret::new("0000000000000000000000000000000000000000".into());
    assert!(matches!(
        client.current_user(&base(), &bogus).await,
        Err(ForgejoError::Unauthorized)
    ));

    let repo_only = new_token(&["read:repository"]).await;
    assert!(matches!(
        client.current_user(&base(), &repo_only).await,
        Err(ForgejoError::MissingScope(_))
    ));
}

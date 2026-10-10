//! SSH key upload against a real Forgejo (spec §6.3 key management): the `write:user`
//! permission check, adding a generated key, and refusing a duplicate. See `dev/forgejo.yml`.
#![cfg(feature = "integration")]

use tenajlo_lib::auth::ssh_keys;
use tenajlo_lib::forgejo::address::normalize_base_url;
use tenajlo_lib::forgejo::keys::{self, AddKeyError};
use tenajlo_lib::forgejo::ForgejoClient;

mod common;
use common::*;

#[tokio::test]
async fn checks_permission_then_adds_a_generated_key() {
    let client = ForgejoClient::new().unwrap();
    let base = normalize_base_url(BASE).unwrap();

    let read_only = token(&["read:user", "read:repository", "write:repository"]).await;
    assert!(!keys::can_add_keys(&client, &base, &read_only)
        .await
        .unwrap());
    let before = keys::list_keys(&client, &base, &read_only).await.unwrap();

    let dir = tempfile::tempdir().unwrap();
    let local = ssh_keys::generate(dir.path(), "integration@test", None).unwrap();
    let (_, line) = ssh_keys::read_public(dir.path(), &local.file_name).unwrap();
    assert!(matches!(
        keys::add_key(&client, &base, &read_only, "it", &line).await,
        Err(AddKeyError::NotAllowed)
    ));
    assert_eq!(
        keys::list_keys(&client, &base, &read_only).await.unwrap(),
        before,
        "the permission check must not create anything"
    );

    let writer = token(&["write:user", "read:repository", "write:repository"]).await;
    assert!(keys::can_add_keys(&client, &base, &writer).await.unwrap());
    let added = keys::add_key(&client, &base, &writer, "it", &line)
        .await
        .unwrap();
    assert_eq!(added.fingerprint, local.fingerprint);
    let listed = keys::list_keys(&client, &base, &writer).await.unwrap();
    assert!(listed.iter().any(|k| k.fingerprint == local.fingerprint));
    assert!(matches!(
        keys::add_key(&client, &base, &writer, "again", &line).await,
        Err(AddKeyError::AlreadyUsed)
    ));

    api(
        reqwest::Method::DELETE,
        &format!("user/keys/{}", added.id),
        serde_json::Value::Null,
    )
    .await;
}

//! "Publish repository" against a real Forgejo (spec §8.1, v1.2): which token scopes allow
//! creating repositories for the user and in organizations, and how refusals map. See
//! `dev/forgejo.yml`.
#![cfg(feature = "integration")]

use tenajlo_lib::forgejo::address::normalize_base_url;
use tenajlo_lib::forgejo::publish::{self, CreateRepoError, NewRepository, Owner, PUBLISH_SCOPES};
use tenajlo_lib::forgejo::repos::list_repositories;
use tenajlo_lib::forgejo::ForgejoClient;

mod common;
use common::*;

#[tokio::test]
async fn sign_in_scopes_cannot_publish_and_probes_create_nothing() {
    let client = ForgejoClient::new().unwrap();
    let base = normalize_base_url(BASE).unwrap();
    let signed_in = token(&["read:user", "read:repository", "write:repository"]).await;
    let before = list_repositories(&client, &base, &signed_in).await.unwrap();

    assert!(!publish::can_create_for_user(&client, &base, &signed_in)
        .await
        .unwrap());
    assert_eq!(
        publish::creatable_orgs(&client, &base, &signed_in, USER)
            .await
            .unwrap(),
        None
    );
    let repo = NewRepository {
        name: "never-created",
        description: "",
        private: true,
    };
    assert!(matches!(
        publish::create_repository(&client, &base, &signed_in, Owner::User, &repo).await,
        Err(CreateRepoError::NotAllowed)
    ));
    assert_eq!(
        list_repositories(&client, &base, &signed_in).await.unwrap(),
        before,
        "permission checks must not create anything"
    );
}

#[tokio::test]
async fn publish_scopes_create_for_user_and_org() {
    let client = ForgejoClient::new().unwrap();
    let base = normalize_base_url(BASE).unwrap();
    let full = token(&PUBLISH_SCOPES).await;
    let name = format!("pub-{}", &uuid::Uuid::new_v4().to_string()[..8]);
    let org = format!("org-{name}");
    api(
        reqwest::Method::POST,
        "orgs",
        serde_json::json!({ "username": org }),
    )
    .await;

    assert!(publish::can_create_for_user(&client, &base, &full)
        .await
        .unwrap());
    assert!(publish::can_create_in_orgs(&client, &base, &full, &org)
        .await
        .unwrap());
    let orgs = publish::creatable_orgs(&client, &base, &full, USER)
        .await
        .unwrap()
        .unwrap();
    assert!(orgs.contains(&org), "{orgs:?}");

    let repo = NewRepository {
        name: &name,
        description: "published by the integration test",
        private: true,
    };
    let mine = publish::create_repository(&client, &base, &full, Owner::User, &repo)
        .await
        .unwrap();
    assert_eq!(mine.full_name, format!("{USER}/{name}"));
    assert!(mine.private);
    assert!(mine.clone_url.ends_with(&format!("/{USER}/{name}.git")));
    let theirs = publish::create_repository(&client, &base, &full, Owner::Org(&org), &repo)
        .await
        .unwrap();
    assert_eq!(theirs.full_name, format!("{org}/{name}"));

    assert!(matches!(
        publish::create_repository(&client, &base, &full, Owner::User, &repo).await,
        Err(CreateRepoError::NameTaken)
    ));
    let bad = NewRepository {
        name: "not allowed!",
        ..repo
    };
    assert!(matches!(
        publish::create_repository(&client, &base, &full, Owner::User, &bad).await,
        Err(CreateRepoError::InvalidName)
    ));

    for full_name in [&mine.full_name, &theirs.full_name] {
        api(
            reqwest::Method::DELETE,
            &format!("repos/{full_name}"),
            serde_json::Value::Null,
        )
        .await;
    }
    api(
        reqwest::Method::DELETE,
        &format!("orgs/{org}"),
        serde_json::Value::Null,
    )
    .await;
}

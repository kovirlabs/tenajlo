//! Creating a repository on Forgejo for "Publish repository" (spec §8.1, v1.2).
//!
//! Sign-in doesn't ask for the scopes this needs: `write:user` + `write:repository` for the
//! user's own repositories, `write:organization` for organizations (verified on Forgejo 11).
//! The `can_create_*` probes find out whether a token has them without creating anything.

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::address::join;
use super::repos::{all_pages, ApiRepo, RemoteRepository};
use super::{ForgejoClient, ForgejoError};
use crate::auth::secrets::Secret;

/// Token scopes for publishing anywhere, plus the ones every Tenajlo account needs. Shown
/// when the user creates a replacement token.
pub const PUBLISH_SCOPES: [&str; 5] = [
    "read:user",
    "write:user",
    "read:repository",
    "write:repository",
    "write:organization",
];

/// Where the new repository goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Owner<'a> {
    /// The signed-in user.
    User,
    /// An organization, by name.
    Org(&'a str),
}

/// What the user typed in the publish dialog.
#[derive(Debug, Serialize)]
pub struct NewRepository<'a> {
    pub name: &'a str,
    pub description: &'a str,
    pub private: bool,
}

/// Why creating the repository failed.
#[derive(Debug, thiserror::Error)]
pub enum CreateRepoError {
    /// The token lacks the scope for this owner.
    #[error("the token can't create repositories here")]
    NotAllowed,
    /// The user isn't allowed to create repositories in the organization.
    #[error("no permission to create repositories in this organization")]
    NoPermission,
    #[error("a repository with this name already exists")]
    NameTaken,
    /// Forgejo's name rules: letters, digits, `-`, `_` and `.`.
    #[error("the repository name isn't allowed")]
    InvalidName,
    #[error(transparent)]
    Api(#[from] ForgejoError),
}

#[derive(Deserialize)]
struct ApiOrg {
    name: String,
}

#[derive(Deserialize)]
struct OrgPermissions {
    #[serde(default)]
    can_create_repository: bool,
}

/// Whether `token` may create repositories for the signed-in user.
pub async fn can_create_for_user(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
) -> Result<bool, ForgejoError> {
    probe(client, create_url(base, Owner::User)?, token).await
}

/// Whether `token` may create repositories in organizations, checked against `org`.
pub async fn can_create_in_orgs(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    org: &str,
) -> Result<bool, ForgejoError> {
    probe(client, create_url(base, Owner::Org(org))?, token).await
}

/// The organizations where `login` may create repositories, sorted by name. `None` when the
/// token can't list organizations (no `read:organization`).
pub async fn creatable_orgs(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    login: &str,
) -> Result<Option<Vec<String>>, ForgejoError> {
    let orgs: Vec<ApiOrg> = match all_pages(client, base, &["user", "orgs"], token).await {
        Ok(orgs) => orgs,
        Err(ForgejoError::MissingScope(_)) => return Ok(None),
        Err(e) => return Err(e),
    };
    let mut out = Vec::new();
    // Usually a handful of organizations, so one request at a time is fine.
    for org in orgs {
        let url = api_url(base, &["users", login, "orgs", &org.name, "permissions"])?;
        let perms: OrgPermissions = client.get_url(url, Some(token)).await?;
        if perms.can_create_repository {
            out.push(org.name);
        }
    }
    out.sort_by_key(|o| o.to_lowercase());
    Ok(Some(out))
}

/// Creates an empty repository (no README, so the first push needs no merge).
pub async fn create_repository(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    owner: Owner<'_>,
    repo: &NewRepository<'_>,
) -> Result<RemoteRepository, CreateRepoError> {
    let created: ApiRepo = client
        .post_url(create_url(base, owner)?, token, repo)
        .await
        .map_err(|e| match e {
            ForgejoError::MissingScope(_) => CreateRepoError::NotAllowed,
            ForgejoError::Http { status: 403, .. } => CreateRepoError::NoPermission,
            ForgejoError::Http { status: 409, .. } => CreateRepoError::NameTaken,
            // "[Name]: AlphaDashDot", "[Name]: MaxSize", or a reserved name.
            ForgejoError::Http { status: 422, .. } => CreateRepoError::InvalidName,
            other => CreateRepoError::Api(other),
        })?;
    Ok(created.into())
}

/// Posts an empty repository. Forgejo checks scopes before it validates the body, so this
/// gets 403 "required scope" without the scope and 422 "[Name]: Required" with it (or a
/// plain 403 for an organization the user can't create in, which still means the scope is
/// there). Nothing is created either way.
async fn probe(client: &ForgejoClient, url: Url, token: &Secret) -> Result<bool, ForgejoError> {
    match client
        .post_url::<_, serde_json::Value>(url, token, &serde_json::json!({}))
        .await
    {
        Err(ForgejoError::MissingScope(_)) => Ok(false),
        Err(ForgejoError::Http {
            status: 403 | 422, ..
        }) => Ok(true),
        Err(e) => Err(e),
        Ok(_) => Err(ForgejoError::Other(
            "the server accepted a repository without a name".into(),
        )),
    }
}

fn create_url(base: &Url, owner: Owner<'_>) -> Result<Url, ForgejoError> {
    match owner {
        Owner::User => Ok(join(base, "api/v1/user/repos")),
        Owner::Org(org) => api_url(base, &["orgs", org, "repos"]),
    }
}

/// `{base}/api/v1/<segments>`, each segment percent-encoded.
fn api_url(base: &Url, segments: &[&str]) -> Result<Url, ForgejoError> {
    let mut url = join(base, "api/v1");
    url.path_segments_mut()
        .map_err(|()| ForgejoError::InvalidUrl("cannot be a base".into()))?
        .extend(segments);
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forgejo::client::tests::fake_server;

    const SCOPE_403: &str = r#"{"message":"token does not have at least one of required scope(s): [write:organization]"}"#;

    fn tok() -> Secret {
        Secret::new("t".into())
    }

    fn created() -> String {
        r#"{"full_name":"Zenith Controls/plc","name":"plc","owner":{"login":"Zenith Controls"},"private":true,"clone_url":"https://h/Zenith%20Controls/plc.git","ssh_url":"ssh://git@h:2222/Zenith%20Controls/plc.git","empty":true}"#.into()
    }

    #[tokio::test]
    async fn probes_tell_scopes_apart() {
        let client = ForgejoClient::new().unwrap();
        let (base, seen) = fake_server(vec![(
            "/api/v1/user/repos",
            "422 Unprocessable Entity",
            r#"{"message":"[Name]: Required"}"#.into(),
        )])
        .await;
        assert!(can_create_for_user(&client, &base, &tok()).await.unwrap());
        assert!(seen.lock().unwrap()[0].starts_with("POST /api/v1/user/repos "));

        let (base, _) = fake_server(vec![(
            "/api/v1/orgs/Zenith%20Controls/repos",
            "403 Forbidden",
            SCOPE_403.into(),
        )])
        .await;
        assert!(
            !can_create_in_orgs(&client, &base, &tok(), "Zenith Controls")
                .await
                .unwrap()
        );

        // Scope present, but not allowed in this organization: still "the token can".
        let (base, _) = fake_server(vec![(
            "/api/v1/orgs/team/repos",
            "403 Forbidden",
            r#"{"message":"Given user is not allowed to create repository in organization."}"#
                .into(),
        )])
        .await;
        assert!(can_create_in_orgs(&client, &base, &tok(), "team")
            .await
            .unwrap());
    }

    #[tokio::test]
    async fn lists_orgs_the_user_can_create_in() {
        let (base, _) = fake_server(vec![
            (
                "/api/v1/user/orgs?limit=50&page=1",
                "200 OK",
                r#"[{"name":"zeta"},{"name":"Alpha"},{"name":"readers"}]"#.into(),
            ),
            (
                "/api/v1/users/evan/orgs/zeta/permissions",
                "200 OK",
                r#"{"can_create_repository":true}"#.into(),
            ),
            (
                "/api/v1/users/evan/orgs/Alpha/permissions",
                "200 OK",
                r#"{"can_create_repository":true}"#.into(),
            ),
            (
                "/api/v1/users/evan/orgs/readers/permissions",
                "200 OK",
                r#"{"can_create_repository":false}"#.into(),
            ),
        ])
        .await;
        let orgs = creatable_orgs(&ForgejoClient::new().unwrap(), &base, &tok(), "evan")
            .await
            .unwrap();
        assert_eq!(orgs, Some(vec!["Alpha".to_owned(), "zeta".to_owned()]));
    }

    #[tokio::test]
    async fn org_listing_without_scope_is_none() {
        let (base, _) = fake_server(vec![(
            "/api/v1/user/orgs?limit=50&page=1",
            "403 Forbidden",
            SCOPE_403.replace("write", "read"),
        )])
        .await;
        let orgs = creatable_orgs(&ForgejoClient::new().unwrap(), &base, &tok(), "evan")
            .await
            .unwrap();
        assert_eq!(orgs, None);
    }

    #[tokio::test]
    async fn creates_in_an_org() {
        let (base, seen) = fake_server(vec![(
            "/api/v1/orgs/Zenith%20Controls/repos",
            "201 Created",
            created(),
        )])
        .await;
        let repo = NewRepository {
            name: "plc",
            description: "",
            private: true,
        };
        let made = create_repository(
            &ForgejoClient::new().unwrap(),
            &base,
            &tok(),
            Owner::Org("Zenith Controls"),
            &repo,
        )
        .await
        .unwrap();
        assert_eq!(made.full_name, "Zenith Controls/plc");
        assert_eq!(made.clone_url, "https://h/Zenith%20Controls/plc.git");
        assert!(seen.lock().unwrap()[0].starts_with("POST /api/v1/orgs/Zenith%20Controls/repos "));
    }

    #[tokio::test]
    async fn create_maps_refusals() {
        let client = ForgejoClient::new().unwrap();
        let repo = NewRepository {
            name: "x",
            description: "",
            private: false,
        };
        let cases = [
            ("403 Forbidden", SCOPE_403, "NotAllowed"),
            (
                "403 Forbidden",
                r#"{"message":"not allowed"}"#,
                "NoPermission",
            ),
            (
                "409 Conflict",
                r#"{"message":"The repository with the same name already exists."}"#,
                "NameTaken",
            ),
            (
                "422 Unprocessable Entity",
                r#"{"message":"[Name]: AlphaDashDot"}"#,
                "InvalidName",
            ),
        ];
        for (status, body, expected) in cases {
            let (base, _) = fake_server(vec![("/api/v1/user/repos", status, body.into())]).await;
            let err = create_repository(&client, &base, &tok(), Owner::User, &repo)
                .await
                .unwrap_err();
            assert_eq!(format!("{err:?}"), expected, "{status} {body}");
        }
    }
}

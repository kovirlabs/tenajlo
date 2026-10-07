//! Repositories the signed-in user can clone (spec §7): their own and their organizations'.

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::address::join;
use super::{ForgejoClient, ForgejoError};
use crate::auth::secrets::Secret;

/// Forgejo's maximum page size (`MAX_RESPONSE_ITEMS` default).
const PAGE_SIZE: usize = 50;
/// Stops runaway pagination: 40 pages × 50 = 2000 repositories per listing.
const MAX_PAGES: usize = 40;

/// A repository shown in the clone dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteRepository {
    /// `owner/name`, unique per server.
    pub full_name: String,
    pub owner: String,
    pub name: String,
    pub description: String,
    pub private: bool,
    pub archived: bool,
    /// HTTPS clone URL.
    pub clone_url: String,
}

#[derive(Deserialize)]
struct ApiRepo {
    full_name: String,
    name: String,
    owner: ApiOwner,
    #[serde(default)]
    description: String,
    #[serde(default)]
    private: bool,
    #[serde(default)]
    archived: bool,
    clone_url: String,
}

#[derive(Deserialize)]
struct ApiOwner {
    login: String,
}

#[derive(Deserialize)]
struct ApiOrg {
    #[serde(alias = "username")]
    name: String,
}

/// The user's repositories plus those of every organization they belong to, sorted by
/// `owner/name` and de-duplicated.
pub async fn list_repositories(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
) -> Result<Vec<RemoteRepository>, ForgejoError> {
    let mut repos: Vec<ApiRepo> = all_pages(client, base, &["user", "repos"], token).await?;
    let orgs: Vec<ApiOrg> = all_pages(client, base, &["user", "orgs"], token).await?;
    for org in orgs {
        repos.extend(
            all_pages::<ApiRepo>(client, base, &["orgs", &org.name, "repos"], token).await?,
        );
    }
    let mut out: Vec<RemoteRepository> = repos
        .into_iter()
        .map(|r| RemoteRepository {
            full_name: r.full_name,
            owner: r.owner.login,
            name: r.name,
            description: r.description,
            private: r.private,
            archived: r.archived,
            clone_url: r.clone_url,
        })
        .collect();
    out.sort_by_key(|r| r.full_name.to_lowercase());
    out.dedup_by(|a, b| a.full_name.eq_ignore_ascii_case(&b.full_name));
    Ok(out)
}

/// Follows `?page=N&limit=50` until a short page. `segments` are percent-encoded.
async fn all_pages<T: serde::de::DeserializeOwned>(
    client: &ForgejoClient,
    base: &Url,
    segments: &[&str],
    token: &Secret,
) -> Result<Vec<T>, ForgejoError> {
    let mut out = Vec::new();
    for page in 1..=MAX_PAGES {
        let mut url = join(base, "api/v1");
        url.path_segments_mut()
            .map_err(|()| ForgejoError::InvalidUrl("cannot be a base".into()))?
            .extend(segments);
        url.query_pairs_mut()
            .append_pair("limit", &PAGE_SIZE.to_string())
            .append_pair("page", &page.to_string());
        let items: Vec<T> = client.get_url(url, Some(token)).await?;
        let short = items.len() < PAGE_SIZE;
        out.extend(items);
        if short {
            break;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::forgejo::client::tests::fake_server;

    fn repo(owner: &str, name: &str) -> String {
        format!(
            r#"{{"full_name":"{owner}/{name}","name":"{name}","owner":{{"login":"{owner}"}},"description":"d","private":true,"clone_url":"https://h/{owner}/{name}.git"}}"#
        )
    }

    #[tokio::test]
    async fn lists_user_and_org_repos_across_pages() {
        let page1: Vec<String> = (0..50).map(|i| repo("evan", &format!("r{i:02}"))).collect();
        let (base, seen) = fake_server(vec![
            (
                "/api/v1/user/repos?limit=50&page=1",
                "200 OK",
                format!("[{}]", page1.join(",")),
            ),
            (
                "/api/v1/user/repos?limit=50&page=2",
                "200 OK",
                format!("[{}]", repo("evan", "zz")),
            ),
            (
                "/api/v1/user/orgs?limit=50&page=1",
                "200 OK",
                r#"[{"name":"TMC Controls"}]"#.into(),
            ),
            (
                "/api/v1/orgs/TMC%20Controls/repos?limit=50&page=1",
                "200 OK",
                format!("[{},{}]", repo("TMC Controls", "plc"), repo("evan", "zz")),
            ),
        ])
        .await;
        let repos = list_repositories(
            &ForgejoClient::new().unwrap(),
            &base,
            &Secret::new("t".into()),
        )
        .await
        .unwrap();
        assert_eq!(repos.len(), 52, "50 + 1 + org repo; duplicate dropped");
        assert_eq!(repos[0].full_name, "evan/r00");
        assert_eq!(repos[51].full_name, "TMC Controls/plc");
        assert!(repos.iter().all(|r| r.private));
        assert_eq!(seen.lock().unwrap().len(), 4);
    }

    #[tokio::test]
    async fn unauthorized_is_reported() {
        let (base, _) = fake_server(vec![(
            "/api/v1/user/repos?limit=50&page=1",
            "401 Unauthorized",
            "{}".into(),
        )])
        .await;
        assert!(matches!(
            list_repositories(
                &ForgejoClient::new().unwrap(),
                &base,
                &Secret::new("t".into())
            )
            .await,
            Err(ForgejoError::Unauthorized)
        ));
    }
}

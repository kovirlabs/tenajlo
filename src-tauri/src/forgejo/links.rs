//! Read-only links to Forgejo for the open repository (spec §7–8.1, v1.4): its web address,
//! the branch's open pull request, and a commit's checks.

use reqwest::Url;
use serde::{Deserialize, Serialize};

use super::address::join;
use super::remote_repo::RepoRef;
use super::repos::all_pages_with;
use super::{ForgejoClient, ForgejoError};
use crate::auth::secrets::Secret;

/// What Tenajlo needs about the remote's repository.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct RepoInfo {
    pub full_name: String,
    /// The repository's web page. Only this and paths below it are ever opened.
    pub html_url: String,
    #[serde(default)]
    pub default_branch: String,
}

/// An open pull request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct PullRequest {
    pub number: u32,
    pub title: String,
}

#[derive(Deserialize)]
struct ApiPull {
    number: u32,
    #[serde(default)]
    title: String,
    head: ApiHead,
}

#[derive(Deserialize)]
struct ApiHead {
    #[serde(rename = "ref")]
    branch: String,
    /// `None` when the fork was deleted.
    #[serde(default)]
    repo: Option<ApiHeadRepo>,
}

#[derive(Deserialize)]
struct ApiHeadRepo {
    full_name: String,
}

/// The combined result of a commit's checks (Forgejo Actions and other CI).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum CheckState {
    Pending,
    Success,
    Warning,
    Failure,
}

/// One check of a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Check {
    /// e.g. `build / test (push)`.
    pub context: String,
    pub state: CheckState,
    pub description: String,
}

/// A commit's checks. `state` is `None` when the commit has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitChecks {
    pub state: Option<CheckState>,
    pub checks: Vec<Check>,
}

#[derive(Deserialize)]
struct ApiCombinedStatus {
    #[serde(default)]
    state: String,
    #[serde(default)]
    statuses: Option<Vec<ApiStatus>>,
}

#[derive(Deserialize)]
struct ApiStatus {
    #[serde(default)]
    context: String,
    #[serde(default, alias = "state")]
    status: String,
    #[serde(default)]
    description: String,
}

fn check_state(s: &str) -> Option<CheckState> {
    match s {
        "pending" => Some(CheckState::Pending),
        "success" => Some(CheckState::Success),
        "warning" => Some(CheckState::Warning),
        "failure" | "error" => Some(CheckState::Failure),
        _ => None,
    }
}

/// `/api/v1/repos/{owner}/{name}/…` with each segment percent-encoded.
fn repo_url(base: &Url, repo: &RepoRef, rest: &[&str]) -> Result<Url, ForgejoError> {
    let mut url = join(base, "api/v1");
    url.path_segments_mut()
        .map_err(|()| ForgejoError::InvalidUrl("cannot be a base".into()))?
        .extend(["repos", &repo.owner, &repo.name])
        .extend(rest);
    Ok(url)
}

/// `GET /repos/{owner}/{name}`: confirms the repository and gives its web address.
pub async fn repo_info(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    repo: &RepoRef,
) -> Result<RepoInfo, ForgejoError> {
    client
        .get_url(repo_url(base, repo, &[])?, Some(token))
        .await
}

/// The open pull request whose head is `branch` of this repository (not a fork's), if any.
pub async fn pull_for_branch(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    repo: &RepoRef,
    branch: &str,
) -> Result<Option<PullRequest>, ForgejoError> {
    let segments = ["repos", repo.owner.as_str(), repo.name.as_str(), "pulls"];
    match all_pages_with::<ApiPull>(client, base, &segments, &[("state", "open")], token).await {
        Ok(pulls) => Ok(pick_pull(pulls, &repo.full_name(), branch)),
        // Pull requests are turned off for this repository: there's no pull request, and the
        // rest of the links (checks, View on Forgejo) still work.
        Err(ForgejoError::Http { status: 404, .. }) => Ok(None),
        Err(e) => Err(e),
    }
}

fn pick_pull(pulls: Vec<ApiPull>, full_name: &str, branch: &str) -> Option<PullRequest> {
    pulls
        .into_iter()
        .filter(|p| p.head.branch == branch)
        .filter(|p| {
            p.head
                .repo
                .as_ref()
                .is_some_and(|r| r.full_name.eq_ignore_ascii_case(full_name))
        })
        .min_by_key(|p| p.number)
        .map(|p| PullRequest {
            number: p.number,
            title: p.title,
        })
}

/// `GET /repos/{owner}/{name}/commits/{sha}/status`. `sha` must be a validated hash.
pub async fn commit_checks(
    client: &ForgejoClient,
    base: &Url,
    token: &Secret,
    repo: &RepoRef,
    sha: &str,
) -> Result<CommitChecks, ForgejoError> {
    let url = repo_url(base, repo, &["commits", sha, "status"])?;
    let raw: ApiCombinedStatus = client.get_url(url, Some(token)).await?;
    Ok(to_checks(raw))
}

fn to_checks(raw: ApiCombinedStatus) -> CommitChecks {
    let mut checks: Vec<Check> = raw
        .statuses
        .unwrap_or_default()
        .into_iter()
        .filter_map(|s| {
            Some(Check {
                state: check_state(&s.status)?,
                context: s.context,
                description: s.description,
            })
        })
        .collect();
    checks.sort_by(|a, b| a.context.cmp(&b.context));
    let state = if checks.is_empty() {
        None
    } else {
        check_state(&raw.state)
    };
    CommitChecks { state, checks }
}

/// A page on the repository's website, built only from its `html_url`.
#[derive(Debug, Clone, serde::Deserialize, specta::Type)]
#[serde(tag = "type", rename_all_fields = "camelCase")]
pub enum ForgejoPage {
    Repository,
    PullRequest {
        number: u32,
    },
    /// Compare `branch` with the default branch, where Forgejo offers "New pull request".
    NewPullRequest {
        branch: String,
    },
    /// `sha` must be a validated hash.
    Commit {
        sha: String,
    },
}

/// The web address of `page`. Every URL is `html_url` plus percent-encoded segments.
pub fn page_url(info: &RepoInfo, page: &ForgejoPage) -> Result<Url, ForgejoError> {
    let mut url =
        Url::parse(&info.html_url).map_err(|e| ForgejoError::InvalidUrl(e.to_string()))?;
    if !matches!(url.scheme(), "https" | "http") {
        return Err(ForgejoError::InvalidUrl("not a web address".into()));
    }
    {
        let mut segs = url
            .path_segments_mut()
            .map_err(|()| ForgejoError::InvalidUrl("cannot be a base".into()))?;
        segs.pop_if_empty();
        match page {
            ForgejoPage::Repository => {}
            ForgejoPage::PullRequest { number } => {
                segs.extend(["pulls", &number.to_string()]);
            }
            ForgejoPage::NewPullRequest { branch } => {
                // Forgejo reads `compare/<base>...<head>` with the branch's `/` as separators.
                let compare = format!("{}...{}", info.default_branch, branch);
                segs.push("compare");
                segs.extend(compare.split('/'));
            }
            ForgejoPage::Commit { sha } => {
                segs.extend(["commit", sha]);
            }
        }
    }
    Ok(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn info() -> RepoInfo {
        RepoInfo {
            full_name: "team/pump".into(),
            html_url: "https://git.example.com/forgejo/team/pump".into(),
            default_branch: "main".into(),
        }
    }

    #[test]
    fn page_urls_stay_under_html_url() {
        let i = info();
        let url = |p| page_url(&i, &p).unwrap().to_string();
        assert_eq!(
            url(ForgejoPage::Repository),
            "https://git.example.com/forgejo/team/pump"
        );
        assert_eq!(
            url(ForgejoPage::PullRequest { number: 12 }),
            "https://git.example.com/forgejo/team/pump/pulls/12"
        );
        assert_eq!(
            url(ForgejoPage::NewPullRequest {
                branch: "feature/low pressure#1".into()
            }),
            "https://git.example.com/forgejo/team/pump/compare/main...feature/low%20pressure%231"
        );
        assert_eq!(
            url(ForgejoPage::Commit {
                sha: "abc123".into()
            }),
            "https://git.example.com/forgejo/team/pump/commit/abc123"
        );
        let mut bad = info();
        bad.html_url = "javascript:alert(1)".into();
        assert!(page_url(&bad, &ForgejoPage::Repository).is_err());
    }

    #[test]
    fn picks_this_repositorys_pull_for_the_branch() {
        let json = r#"[
          {"number": 9, "title": "From a fork", "head": {"ref": "feature/x", "repo": {"full_name": "sam/pump"}}},
          {"number": 12, "title": "Add low pressure alarm", "head": {"ref": "feature/x", "repo": {"full_name": "Team/Pump"}}},
          {"number": 3, "title": "Other", "head": {"ref": "main", "repo": {"full_name": "team/pump"}}},
          {"number": 4, "title": "Deleted fork", "head": {"ref": "feature/x", "repo": null}}
        ]"#;
        let pulls: Vec<ApiPull> = serde_json::from_str(json).unwrap();
        assert_eq!(
            pick_pull(pulls, "team/pump", "feature/x"),
            Some(PullRequest {
                number: 12,
                title: "Add low pressure alarm".into()
            })
        );
        let none: Vec<ApiPull> = serde_json::from_str("[]").unwrap();
        assert_eq!(pick_pull(none, "team/pump", "feature/x"), None);
    }

    #[test]
    fn combined_status_maps_to_checks() {
        // Shape of Forgejo 11's /commits/{sha}/status with Forgejo Actions.
        let json = r#"{"state": "failure", "total_count": 2, "statuses": [
          {"context": "test / unit (push)", "status": "failure", "description": "Failing after 31s"},
          {"context": "build (push)", "status": "success", "description": "Successful in 1m"}
        ]}"#;
        let checks = to_checks(serde_json::from_str(json).unwrap());
        assert_eq!(checks.state, Some(CheckState::Failure));
        assert_eq!(checks.checks[0].context, "build (push)");
        assert_eq!(checks.checks[1].state, CheckState::Failure);

        let none = to_checks(serde_json::from_str(r#"{"state": "", "statuses": null}"#).unwrap());
        assert_eq!(
            none,
            CommitChecks {
                state: None,
                checks: vec![]
            }
        );
        let pending =
            r#"{"state": "pending", "statuses": [{"context": "ci", "state": "pending"}]}"#;
        assert_eq!(
            to_checks(serde_json::from_str(pending).unwrap()).state,
            Some(CheckState::Pending)
        );
    }
}

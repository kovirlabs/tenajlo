//! Forgejo links against the dev Forgejo (spec §8.1, v1.4): resolve a clone's remote to its
//! repository through the signed-in account, find the branch's open pull request, and read a
//! commit's checks.
#![cfg(feature = "integration")]

use std::path::Path;
use std::sync::Arc;

use tenajlo_lib::auth::accounts::AccountManager;
use tenajlo_lib::forgejo::links::{self, CheckState, ForgejoPage};
use tenajlo_lib::forgejo::{ForgejoClient, ForgejoUser};
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::exec::{Access, GitBinary, GitCommand};
use tenajlo_lib::git::remote::{self, RemoteRun};
use tenajlo_lib::repo_links::RepoLinks;
use uuid::Uuid;

mod common;
use common::*;

async fn run(git: &GitBinary, work: &Path, args: &[&str]) -> Vec<u8> {
    let mut full = vec!["-c", "user.name=T", "-c", "user.email=t@x"];
    full.extend_from_slice(args);
    GitCommand::new(full, Access::Mutating)
        .cwd(work)
        .run(git)
        .await
        .unwrap()
        .stdout
}

#[tokio::test]
async fn branch_pull_request_and_commit_checks() {
    let dir = tempfile::tempdir().unwrap();
    let git = resolve(None, None).unwrap();
    let (repo, _) = private_repos().await;

    // A clone whose remote has no credentials in it: the account is found by host.
    let work = dir.path().join("work");
    let authed = authed_url(USER, PASSWORD, &repo);
    clone(&git, &authed, &work, RemoteRun::quiet())
        .await
        .unwrap();
    run(&git, &work, &["switch", "-q", "-c", "feature/low-pressure"]).await;
    std::fs::write(work.join("alarm.st"), "bLowPressure : BOOL;\n").unwrap();
    run(&git, &work, &["add", "-A"]).await;
    run(&git, &work, &["commit", "-q", "-m", "Add alarm"]).await;
    remote::push(
        &git,
        &work,
        "origin",
        "feature/low-pressure",
        "feature/low-pressure",
        true,
        RemoteRun::quiet(),
    )
    .await
    .unwrap();
    let sha = String::from_utf8(run(&git, &work, &["rev-parse", "HEAD"]).await)
        .unwrap()
        .trim()
        .to_owned();
    run(
        &git,
        &work,
        &["remote", "set-url", "origin", &format!("{BASE}/{repo}.git")],
    )
    .await;

    let pr = api(
        reqwest::Method::POST,
        &format!("repos/{repo}/pulls"),
        serde_json::json!({ "head": "feature/low-pressure", "base": "main", "title": "Low pressure alarm" }),
    )
    .await;
    api(
        reqwest::Method::POST,
        &format!("repos/{repo}/statuses/{sha}"),
        serde_json::json!({ "state": "failure", "context": "test / unit (push)", "description": "Failing after 31s" }),
    )
    .await;
    api(
        reqwest::Method::POST,
        &format!("repos/{repo}/statuses/{sha}"),
        serde_json::json!({ "state": "success", "context": "build (push)" }),
    )
    .await;

    let accounts = AccountManager::load(dir.path(), Arc::new(Keychain::default()));
    let user = ForgejoUser {
        login: USER.into(),
        full_name: String::new(),
        avatar_url: None,
        email: String::new(),
    };
    accounts
        .sign_in(&BASE, user, token(&["read:user", "read:repository"]).await)
        .await
        .unwrap();
    let client = ForgejoClient::new().unwrap();
    let linked = RepoLinks::default()
        .resolve(&accounts, &client, &git, Uuid::new_v4(), &work)
        .await
        .unwrap()
        .expect("the remote is on the account's server");
    assert_eq!(linked.info.full_name, repo);
    assert_eq!(linked.info.default_branch, "main");
    assert_eq!(
        links::page_url(&linked.info, &ForgejoPage::Commit { sha: sha.clone() })
            .unwrap()
            .as_str(),
        format!("{BASE}/{repo}/commit/{sha}")
    );

    let token = accounts.usable_token(&linked.account).await.unwrap();
    let pull = links::pull_for_branch(
        &client,
        &linked.base,
        &token,
        &linked.repo,
        "feature/low-pressure",
    )
    .await
    .unwrap()
    .expect("open pull request");
    assert_eq!(u64::from(pull.number), pr["number"].as_u64().unwrap());
    assert_eq!(pull.title, "Low pressure alarm");
    assert!(
        links::pull_for_branch(&client, &linked.base, &token, &linked.repo, "main")
            .await
            .unwrap()
            .is_none()
    );

    let checks = links::commit_checks(&client, &linked.base, &token, &linked.repo, &sha)
        .await
        .unwrap();
    assert_eq!(checks.state, Some(CheckState::Failure));
    assert_eq!(checks.checks.len(), 2);
    assert_eq!(checks.checks[0].context, "build (push)");
    assert_eq!(checks.checks[1].description, "Failing after 31s");

    // With pull requests turned off the server answers 404 for them: that's "no pull
    // request", not a failure that would hide the checks and View on Forgejo too.
    api(
        reqwest::Method::PATCH,
        &format!("repos/{repo}"),
        serde_json::json!({ "has_pull_requests": false }),
    )
    .await;
    assert_eq!(
        links::pull_for_branch(
            &client,
            &linked.base,
            &token,
            &linked.repo,
            "feature/low-pressure"
        )
        .await
        .unwrap(),
        None
    );
}

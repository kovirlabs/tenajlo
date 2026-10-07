//! Git LFS against the dev Forgejo's LFS server (spec §13.1, M6): commit a large binary as
//! an LFS pointer, upload it with the account token, and get the real content back on a
//! fresh clone. The user's global and system git config are hidden, proving Tenajlo's
//! per-invocation filter flags are enough (no `git lfs install`).
#![cfg(feature = "integration")]

use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;

use tenajlo_lib::auth::accounts::AccountManager;
use tenajlo_lib::auth::remote_auth::{self, RemoteAuth};
use tenajlo_lib::auth::trampoline::{PromptFn, Trampoline};
use tenajlo_lib::forgejo::ForgejoUser;
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::exec::{Access, GitBinary, GitCommand};
use tenajlo_lib::git::lfs;
use tenajlo_lib::git::remote::{self, RemoteRun};
use tokio_util::sync::CancellationToken;

mod common;
use common::*;

/// Hides ~/.gitconfig and /etc/gitconfig (where `git lfs install` would have written).
fn isolated_env(dir: &Path) -> Vec<(OsString, OsString)> {
    let empty = dir.join("empty-gitconfig");
    std::fs::write(&empty, "").unwrap();
    vec![
        ("GIT_CONFIG_GLOBAL".into(), empty.into_os_string()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
    ]
}

async fn git(git: &GitBinary, dir: &Path, env: &[(OsString, OsString)], args: &[&str]) -> Vec<u8> {
    let mut cmd = GitCommand::new(args.to_vec(), Access::Mutating).cwd(dir);
    for (k, v) in env {
        cmd = cmd.env(k.clone(), v.clone());
    }
    cmd.run(git).await.unwrap().stdout
}

#[tokio::test]
async fn large_files_round_trip_through_lfs_with_the_account() {
    let dir = tempfile::tempdir().unwrap();
    let env = isolated_env(dir.path());
    let lfs_git = resolve(None, None).unwrap().with_lfs(true);
    let (repo, _) = private_repos().await;
    let url = format!("{BASE}/{repo}.git");

    let accounts = AccountManager::load(dir.path(), Arc::new(Keychain::default()));
    let user = ForgejoUser {
        login: USER.into(),
        full_name: String::new(),
        avatar_url: None,
    };
    let full = ["read:user", "read:repository", "write:repository"];
    accounts
        .sign_in(BASE, user, token(&full).await)
        .await
        .unwrap();
    let no_prompts: PromptFn = Arc::new(|p| panic!("unexpected prompt {:?}", p.kind));
    let trampoline = Trampoline::start(no_prompts).await.unwrap();
    let askpass = askpass();
    let auth = || async {
        let cancel = CancellationToken::new();
        let auth: RemoteAuth = remote_auth::prepare(
            &accounts,
            Some(&trampoline),
            Some(&askpass),
            "repo",
            &uuid::Uuid::new_v4().to_string(),
            cancel.clone(),
            Some(&url),
            true,
        )
        .await
        .unwrap();
        let mut run_env = auth.env.clone();
        run_env.extend(env.iter().cloned());
        let run = RemoteRun {
            env: run_env,
            config: auth.config.clone(),
            cancel,
            on_progress: Box::new(|_| {}),
        };
        (auth, run)
    };

    // Commit a 3 MB binary through the LFS filter.
    let work = dir.path().join("work");
    let (_a, run) = auth().await;
    clone(&lfs_git, &url, &work, run).await.unwrap();
    std::fs::write(
        work.join(".gitattributes"),
        "*.SLDPRT filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    let content: Vec<u8> = (0..3_000_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    std::fs::write(work.join("pump housing.SLDPRT"), &content).unwrap();
    assert!(lfs::status(&lfs_git, &work).used);
    git(&lfs_git, &work, &env, &["add", "-A"]).await;
    git(
        &lfs_git,
        &work,
        &env,
        &[
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@x",
            "commit",
            "-q",
            "-m",
            "CAD",
        ],
    )
    .await;
    let blob = git(
        &lfs_git,
        &work,
        &env,
        &["cat-file", "-p", "HEAD:pump housing.SLDPRT"],
    )
    .await;
    assert!(
        blob.starts_with(b"version https://git-lfs.github.com/spec/v1"),
        "committed as a pointer, not 3 MB of content"
    );

    // Upload LFS objects, then push (what `sync` does).
    let (_a, run) = auth().await;
    lfs::push_objects(&lfs_git, &work, "origin", "main", run)
        .await
        .unwrap();
    let (a, run) = auth().await;
    remote::push(&lfs_git, &work, "origin", "main", "main", false, run)
        .await
        .unwrap();
    assert!(a.account_used());

    // The server has the real content…
    let media = reqwest::Client::new()
        .get(format!(
            "{BASE}/api/v1/repos/{repo}/media/pump%20housing.SLDPRT"
        ))
        .basic_auth(USER, Some(PASSWORD))
        .send()
        .await
        .unwrap();
    assert!(media.status().is_success(), "{}", media.status());
    assert_eq!(media.bytes().await.unwrap().as_ref(), content.as_slice());

    // …and a fresh clone gets the file, not the pointer.
    let fresh = dir.path().join("fresh");
    let (_a, run) = auth().await;
    clone(&lfs_git, &url, &fresh, run).await.unwrap();
    assert_eq!(
        std::fs::read(fresh.join("pump housing.SLDPRT")).unwrap(),
        content
    );
    let status = git(&lfs_git, &fresh, &env, &["status", "--porcelain"]).await;
    assert!(
        status.is_empty(),
        "clean after clone: {}",
        String::from_utf8_lossy(&status)
    );
}

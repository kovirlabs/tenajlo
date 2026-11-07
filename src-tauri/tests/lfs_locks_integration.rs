//! Git LFS file locks against the dev Forgejo (spec §8.1, v1.4): lock as one user, see the
//! lock as "theirs" as another, refuse that user's push of the locked file, then unlock.
//! Credentials go in the URL here; the account helper path is covered by lfs_integration.
#![cfg(feature = "integration")]

use std::ffi::OsString;
use std::path::Path;

use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::error::{GitError, GitErrorKind};
use tenajlo_lib::git::exec::{Access, GitBinary, GitCommand};
use tenajlo_lib::git::remote::{self, RemoteRun};
use tenajlo_lib::git::{lfs, lfs_locks};

mod common;
use common::*;

const OTHER: &str = "alice";
const OTHER_PASSWORD: &str = "alice-dev-password";

/// Hides the user's global config, so only Tenajlo's per-invocation flags apply.
fn isolated(dir: &Path) -> Vec<(OsString, OsString)> {
    let empty = dir.join("empty-gitconfig");
    std::fs::write(&empty, "").unwrap();
    vec![
        ("GIT_CONFIG_GLOBAL".into(), empty.into_os_string()),
        ("GIT_CONFIG_NOSYSTEM".into(), "1".into()),
    ]
}

fn run(env: &[(OsString, OsString)]) -> RemoteRun {
    RemoteRun {
        env: env.to_vec(),
        ..RemoteRun::quiet()
    }
}

async fn git(git: &GitBinary, dir: &Path, env: &[(OsString, OsString)], args: &[&str]) {
    let mut cmd = GitCommand::new(args.to_vec(), Access::Mutating).cwd(dir);
    for (k, v) in env {
        cmd = cmd.env(k.clone(), v.clone());
    }
    cmd.run(git).await.unwrap();
}

async fn commit_all(g: &GitBinary, dir: &Path, env: &[(OsString, OsString)], msg: &str) {
    git(g, dir, env, &["add", "-A"]).await;
    let args = [
        "-c",
        "user.name=T",
        "-c",
        "user.email=t@x",
        "commit",
        "-q",
        "-m",
        msg,
    ];
    git(g, dir, env, &args).await;
}

#[tokio::test]
async fn locks_are_shared_and_block_other_peoples_pushes() {
    let dir = tempfile::tempdir().unwrap();
    let env = isolated(dir.path());
    let g = resolve(None, None).unwrap().with_lfs(true);
    let (repo, _) = private_repos().await;
    api(
        reqwest::Method::PUT,
        &format!("repos/{repo}/collaborators/{OTHER}"),
        serde_json::json!({ "permission": "write" }),
    )
    .await;
    let url_for =
        |user: &str, pass: &str| format!("http://{user}:{pass}@localhost:3000/{repo}.git");

    // The owner adds a CAD file through LFS.
    let mine = dir.path().join("mine");
    clone(&g, &url_for(USER, PASSWORD), &mine, run(&env))
        .await
        .unwrap();
    std::fs::write(
        mine.join(".gitattributes"),
        "*.SLDPRT filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    std::fs::write(mine.join("Pumpe Gehäuse.SLDPRT"), [1u8; 2048]).unwrap();
    commit_all(&g, &mine, &env, "CAD").await;
    lfs::push_objects(&g, &mine, "origin", "main", false, run(&env))
        .await
        .unwrap();
    remote::push(&g, &mine, "origin", "main", "main", false, run(&env))
        .await
        .unwrap();

    let (name, _url) = lfs_locks::lock_remote(&g, &mine).await.unwrap().unwrap();
    assert_eq!(name, "origin");
    let lock = lfs_locks::lock(&g, &mine, "origin", "Pumpe Gehäuse.SLDPRT", run(&env))
        .await
        .unwrap();
    assert_eq!(
        (lock.owner.as_str(), lock.path.as_str()),
        (USER, "Pumpe Gehäuse.SLDPRT")
    );
    let again = lfs_locks::lock(&g, &mine, "origin", "Pumpe Gehäuse.SLDPRT", run(&env)).await;
    assert!(
        matches!(
            again,
            Err(GitError::Failed {
                kind: GitErrorKind::LfsAlreadyLocked,
                ..
            })
        ),
        "{again:?}"
    );
    let listed = lfs_locks::list(&g, &mine, "origin", || run(&env))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(listed[0].ours);

    // The other user sees it as theirs, can't unlock it without forcing, and can't push it.
    let theirs = dir.path().join("theirs");
    clone(&g, &url_for(OTHER, OTHER_PASSWORD), &theirs, run(&env))
        .await
        .unwrap();
    let listed = lfs_locks::list(&g, &theirs, "origin", || run(&env))
        .await
        .unwrap();
    assert_eq!((listed[0].owner.as_str(), listed[0].ours), (USER, false));
    let denied = lfs_locks::unlock(&g, &theirs, "origin", &lock.id, false, run(&env)).await;
    assert!(
        matches!(
            denied,
            Err(GitError::Failed {
                kind: GitErrorKind::LfsLockNotYours,
                ..
            })
        ),
        "{denied:?}"
    );
    std::fs::write(theirs.join("Pumpe Gehäuse.SLDPRT"), [2u8; 2048]).unwrap();
    commit_all(&g, &theirs, &env, "edit").await;
    let refused = lfs::push_objects(&g, &theirs, "origin", "main", true, run(&env)).await;
    assert!(
        matches!(
            refused,
            Err(GitError::Failed {
                kind: GitErrorKind::LfsFileLocked,
                ..
            })
        ),
        "{refused:?}"
    );

    // Refused too before Tenajlo has listed this server's locks (it verifies when unsure)...
    let (unsure, learned) =
        lfs::push_objects_verified(&g, &theirs, "origin", "main", None, || run(&env)).await;
    assert!(
        matches!(
            unsure,
            Err(GitError::Failed {
                kind: GitErrorKind::LfsFileLocked,
                ..
            })
        ),
        "{unsure:?}"
    );
    assert_eq!(learned, None);

    // ...and when the repository's config turns verification off for this server's LFS URL,
    // as git-lfs itself does after a server error: that key outranks lfs.locksverify.
    let lfs_url = format!("http://localhost:3000/{repo}.git/info/lfs");
    git(
        &g,
        &theirs,
        &env,
        &["config", &format!("lfs.{lfs_url}.locksverify"), "false"],
    )
    .await;
    let overridden = lfs::push_objects(&g, &theirs, "origin", "main", true, run(&env)).await;
    assert!(
        matches!(
            overridden,
            Err(GitError::Failed {
                kind: GitErrorKind::LfsFileLocked,
                ..
            })
        ),
        "{overridden:?}"
    );

    // The owner unlocks; the other user's push now goes through.
    lfs_locks::unlock(&g, &mine, "origin", &lock.id, false, run(&env))
        .await
        .unwrap();
    assert!(lfs_locks::list(&g, &mine, "origin", || run(&env))
        .await
        .unwrap()
        .is_empty());
    lfs::push_objects(&g, &theirs, "origin", "main", true, run(&env))
        .await
        .unwrap();
}

#[tokio::test]
async fn read_only_collaborators_still_see_locks() {
    let dir = tempfile::tempdir().unwrap();
    let env = isolated(dir.path());
    let g = resolve(None, None).unwrap().with_lfs(true);
    let (repo, _) = private_repos().await;
    api(
        reqwest::Method::PUT,
        &format!("repos/{repo}/collaborators/{OTHER}"),
        serde_json::json!({ "permission": "read" }),
    )
    .await;
    let url_for =
        |user: &str, pass: &str| format!("http://{user}:{pass}@localhost:3000/{repo}.git");

    let mine = dir.path().join("mine");
    clone(&g, &url_for(USER, PASSWORD), &mine, run(&env))
        .await
        .unwrap();
    std::fs::write(
        mine.join(".gitattributes"),
        "*.SLDPRT filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    std::fs::write(mine.join("Pump.SLDPRT"), [1u8; 512]).unwrap();
    commit_all(&g, &mine, &env, "CAD").await;
    lfs::push_objects(&g, &mine, "origin", "main", false, run(&env))
        .await
        .unwrap();
    remote::push(&g, &mine, "origin", "main", "main", false, run(&env))
        .await
        .unwrap();
    lfs_locks::lock(&g, &mine, "origin", "Pump.SLDPRT", run(&env))
        .await
        .unwrap();

    // A reader can't use the verify endpoint (it needs write access), but must still see
    // who has the file locked before asking them for it.
    let reader = dir.path().join("reader");
    clone(&g, &url_for(OTHER, OTHER_PASSWORD), &reader, run(&env))
        .await
        .unwrap();
    let listed = lfs_locks::list(&g, &reader, "origin", || run(&env))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (
            listed[0].path.as_str(),
            listed[0].owner.as_str(),
            listed[0].ours
        ),
        ("Pump.SLDPRT", USER, false)
    );
}

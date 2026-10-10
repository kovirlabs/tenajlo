//! SSH remotes against the dev Forgejo on port 2222 (spec §6.3, M5): the real OpenSSH asks
//! for host-key confirmation and the key passphrase through `SSH_ASKPASS` → the real
//! `tenajlo-askpass` → the trampoline. Uses a throwaway key and a temporary known_hosts, so
//! the user's `~/.ssh` is never touched.
#![cfg(feature = "integration")]

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tenajlo_lib::auth::prompt::{AuthAnswer, PromptKind};
use tenajlo_lib::auth::trampoline::{PromptFn, Trampoline};
use tenajlo_lib::git::binary::resolve;
use tenajlo_lib::git::clone::clone;
use tenajlo_lib::git::error::GitError;
use tenajlo_lib::git::exec::{Access, GitCommand};
use tenajlo_lib::git::remote::{self, RemoteRun};
use tokio_util::sync::CancellationToken;

mod common;
use common::*;

const PASSPHRASE: &str = "correct horse";

/// A passphrase-protected ed25519 key registered with the test user.
async fn ssh_key(dir: &Path) -> PathBuf {
    let key = dir.join("id_ed25519");
    let status = std::process::Command::new("ssh-keygen")
        .args([
            "-q",
            "-t",
            "ed25519",
            "-C",
            "tenajlo-it",
            "-N",
            PASSPHRASE,
            "-f",
        ])
        .arg(&key)
        .status()
        .unwrap();
    assert!(status.success());
    let public = std::fs::read_to_string(key.with_extension("pub")).unwrap();
    api(
        reqwest::Method::POST,
        "user/keys",
        serde_json::json!({ "title": format!("it-{}", uuid::Uuid::new_v4()), "key": public.trim() }),
    )
    .await;
    key
}

/// What a scripted user does with each prompt.
#[derive(Clone, Copy)]
struct User {
    trust_host: bool,
    wrong_passphrase_first: bool,
}

struct Ssh {
    trampoline: Trampoline,
    prompts: Arc<Mutex<Vec<PromptKind>>>,
    askpass: PathBuf,
    ssh_command: String,
    known_hosts: PathBuf,
}

async fn ssh_env(dir: &Path, key: &Path, user: User) -> Ssh {
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let seen = prompts.clone();
    let prompt: PromptFn = Arc::new(move |p| {
        let mut seen = seen.lock().unwrap();
        let answer = match &p.kind {
            PromptKind::HostKey { .. } if user.trust_host => Some(String::new()),
            PromptKind::HostKey { .. } => None,
            PromptKind::Passphrase { retry: false, .. } if user.wrong_passphrase_first => {
                Some("wrong".to_owned())
            }
            PromptKind::Passphrase { .. } => Some(PASSPHRASE.to_owned()),
            _ => None,
        };
        seen.push(p.kind);
        Box::pin(async move {
            answer.map(|secret| AuthAnswer {
                username: None,
                secret,
                remember: false,
            })
        })
    });
    let known_hosts = dir.join("known_hosts");
    // Isolate from ~/.ssh/config, the user's agent keys and known_hosts.
    let ssh_command = format!(
        "ssh -F /dev/null -o IdentitiesOnly=yes -o IdentityAgent=none -i '{}' -o UserKnownHostsFile='{}'",
        key.display(),
        known_hosts.display()
    );
    Ssh {
        trampoline: Trampoline::start(prompt).await.unwrap(),
        prompts,
        askpass: askpass(),
        ssh_command,
        known_hosts,
    }
}

impl Ssh {
    /// Keep the returned token alive while git runs.
    fn run(&self, cancel: CancellationToken) -> (impl Drop, RemoteRun) {
        let token = self
            .trampoline
            .register(
                "repo",
                &uuid::Uuid::new_v4().to_string(),
                cancel.clone(),
                None,
            )
            .unwrap();
        let run = RemoteRun {
            env: token.env(&self.askpass),
            config: vec![format!("core.sshCommand={}", self.ssh_command)],
            cancel,
            on_progress: Box::new(|_| {}),
        };
        (token, run)
    }

    fn kinds(&self) -> Vec<String> {
        self.prompts
            .lock()
            .unwrap()
            .iter()
            .map(|k| match k {
                PromptKind::HostKey { host, key_type, .. } => format!("host {host} {key_type}"),
                PromptKind::Passphrase { retry, .. } => format!("passphrase retry={retry}"),
                other => format!("{other:?}"),
            })
            .collect()
    }
}

fn ssh_url(full_name: &str) -> String {
    format!("ssh://git@localhost:2222/{full_name}.git")
}

#[tokio::test]
async fn clone_and_push_over_ssh_with_host_key_and_passphrase_prompts() {
    let dir = tempfile::tempdir().unwrap();
    let key = ssh_key(dir.path()).await;
    let (_, org) = private_repos().await;
    let user = User {
        trust_host: true,
        wrong_passphrase_first: true,
    };
    let ssh = ssh_env(dir.path(), &key, user).await;
    let git = resolve(None, None).unwrap();

    let dest = dir.path().join("work").join("plc");
    let (_token, run) = ssh.run(CancellationToken::new());
    clone(&git, &ssh_url(&org), &dest, run).await.unwrap();
    assert!(dest.join("README.md").exists());
    assert_eq!(
        ssh.kinds(),
        [
            "host [localhost]:2222 ED25519",
            "passphrase retry=false",
            "passphrase retry=true"
        ]
    );
    let known = std::fs::read_to_string(&ssh.known_hosts).unwrap();
    assert!(
        known.starts_with("[localhost]:2222 ssh-ed25519 "),
        "OpenSSH recorded the key"
    );

    // Second operation: host now known, so only the passphrase is asked.
    std::fs::write(dest.join("ssh.txt"), "over ssh\n").unwrap();
    for args in [
        vec!["add", "-A"],
        vec![
            "-c",
            "user.name=T",
            "-c",
            "user.email=t@x",
            "commit",
            "-q",
            "-m",
            "ssh",
        ],
    ] {
        GitCommand::new(args, Access::Mutating)
            .cwd(&dest)
            .run(&git)
            .await
            .unwrap();
    }
    ssh.prompts.lock().unwrap().clear();
    let (_token, run) = ssh.run(CancellationToken::new());
    remote::push(&git, &dest, "origin", "main", "main", false, run)
        .await
        .unwrap();
    // A new operation, so the first passphrase prompt isn't a retry; the scripted user
    // still answers it wrongly once.
    assert_eq!(
        ssh.kinds(),
        ["passphrase retry=false", "passphrase retry=true"]
    );
}

#[tokio::test]
async fn rejecting_the_host_key_cancels_without_trusting_it() {
    let dir = tempfile::tempdir().unwrap();
    let key = ssh_key(dir.path()).await;
    let (_, org) = private_repos().await;
    let user = User {
        trust_host: false,
        wrong_passphrase_first: false,
    };
    let ssh = ssh_env(dir.path(), &key, user).await;
    let dest = dir.path().join("rejected");
    let cancel = CancellationToken::new();
    let (_token, run) = ssh.run(cancel.clone());
    let err = clone(&resolve(None, None).unwrap(), &ssh_url(&org), &dest, run)
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            tenajlo_lib::git::clone::CloneError::Git(GitError::Cancelled)
        ),
        "{err:?}"
    );
    assert!(cancel.is_cancelled());
    assert_eq!(ssh.kinds(), ["host [localhost]:2222 ED25519"]);
    assert!(!dest.exists(), "partial clone removed");
    let known = std::fs::read_to_string(&ssh.known_hosts).unwrap_or_default();
    assert!(known.is_empty(), "host key not recorded: {known}");
}

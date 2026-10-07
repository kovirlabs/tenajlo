//! Drives real git through the real `tenajlo-askpass` binary. `git credential fill` asks
//! for a username and password via GIT_ASKPASS exactly as a fetch would, without a server.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use tenajlo_askpass::{
    credential_helper_config, CredentialOp, Mode, Request, Response, ENV_PORT, ENV_TOKEN,
};

/// Minimal stand-in for the app's trampoline: answers prompts for `token` only.
/// Askpass prompts are recorded as text; credential requests as `op key=value…`.
fn fake_app(token: &'static str) -> (u16, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let prompts = Arc::new(Mutex::new(Vec::new()));
    let seen = prompts.clone();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut line = String::new();
            BufReader::new(&stream).read_line(&mut line).unwrap();
            let req: Request = serde_json::from_str(&line).unwrap();
            if req.mode == Mode::Credential {
                let fields: Vec<String> =
                    req.fields.iter().map(|(k, v)| format!("{k}={v}")).collect();
                seen.lock()
                    .unwrap()
                    .push(format!("{:?} {}", req.op.unwrap(), fields.join(" ")));
                let fields = if req.token == token && req.op == Some(CredentialOp::Get) {
                    vec![
                        ("username".into(), "evan".into()),
                        ("password".into(), "pat-from-keychain".into()),
                    ]
                } else {
                    Vec::new()
                };
                let mut out = serde_json::to_vec(&Response {
                    answer: None,
                    fields,
                })
                .unwrap();
                out.push(b'\n');
                stream.write_all(&out).unwrap();
                continue;
            }
            let prompt = req.prompt.unwrap_or_default();
            seen.lock().unwrap().push(prompt.clone());
            let answer = if req.token != token {
                None
            } else if prompt.starts_with("Username") {
                Some("ëvan".to_owned())
            } else {
                Some("s3cr3t token".to_owned())
            };
            let mut out = serde_json::to_vec(&Response {
                answer,
                fields: Vec::new(),
            })
            .unwrap();
            out.push(b'\n');
            stream.write_all(&out).unwrap();
        }
    });
    (port, prompts)
}

fn credential_fill(port: u16, token: &str) -> std::process::Output {
    let mut child = Command::new("git")
        .args(["-c", "credential.helper=", "credential", "fill"])
        .env("GIT_ASKPASS", env!("CARGO_BIN_EXE_tenajlo-askpass"))
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("LC_ALL", "C")
        .env(ENV_PORT, port.to_string())
        .env(ENV_TOKEN, token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"protocol=https\nhost=git.example.com\n\n")
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn git_gets_credentials_through_the_trampoline() {
    let (port, prompts) = fake_app("good-token");
    let out = credential_fill(port, "good-token");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("username=ëvan\n"), "{stdout}");
    assert!(stdout.contains("password=s3cr3t token\n"), "{stdout}");
    let prompts = prompts.lock().unwrap();
    assert_eq!(prompts[0], "Username for 'https://git.example.com': ");
    // git percent-encodes the username inside the URL.
    assert_eq!(
        prompts[1],
        "Password for 'https://%C3%ABvan@git.example.com': "
    );
}

#[test]
fn rejected_token_fails_git_without_hanging() {
    let (port, _) = fake_app("good-token");
    let out = credential_fill(port, "wrong-token");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stdout).is_empty());
}

#[test]
fn refuses_to_run_outside_tenajlo() {
    let out = Command::new(env!("CARGO_BIN_EXE_tenajlo-askpass"))
        .arg("Password: ")
        .env_remove(ENV_PORT)
        .env_remove(ENV_TOKEN)
        .output()
        .unwrap();
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("not started by Tenajlo"));
}

/// Copies the helper into a path with a space and an apostrophe, as installs may have.
fn awkward_helper_path(dir: &std::path::Path) -> std::path::PathBuf {
    let sub = dir.join("Program Files").join("it's");
    std::fs::create_dir_all(&sub).unwrap();
    let dest = sub.join(if cfg!(windows) {
        "tenajlo-askpass.exe"
    } else {
        "tenajlo-askpass"
    });
    std::fs::copy(env!("CARGO_BIN_EXE_tenajlo-askpass"), &dest).unwrap();
    dest
}

fn git_credential(
    port: u16,
    token: &str,
    helper: &str,
    global: &std::path::Path,
    action: &str,
    input: &str,
) -> std::process::Output {
    let mut child = Command::new("git")
        .args(["-c", "credential.helper=", "-c"])
        .arg(format!("credential.helper={helper}"))
        .args(["credential", action])
        .env("GIT_ASKPASS", env!("CARGO_BIN_EXE_tenajlo-askpass"))
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", global)
        .env("LC_ALL", "C")
        .env(ENV_PORT, port.to_string())
        .env(ENV_TOKEN, token)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn credential_helper_mode_answers_from_the_app_and_overrides_user_helpers() {
    let dir = tempfile_dir();
    // A user's URL-specific helper must not run once Tenajlo resets the helper list.
    let global = dir.join("gitconfig");
    std::fs::write(
        &global,
        "[credential \"https://git.example.com\"]\n\thelper = \"!f() { echo password=FROM-USER-HELPER; }; f\"\n",
    )
    .unwrap();
    let helper = credential_helper_config(&awkward_helper_path(&dir));
    let (port, seen) = fake_app("good-token");

    let input = "protocol=https\nhost=git.example.com\n\n";
    let out = git_credential(port, "good-token", &helper, &global, "fill", input);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("username=evan\n"), "{stdout}");
    assert!(stdout.contains("password=pat-from-keychain\n"), "{stdout}");
    assert!(!stdout.contains("FROM-USER-HELPER"), "{stdout}");

    let reject =
        "protocol=https\nhost=git.example.com\nusername=evan\npassword=pat-from-keychain\n\n";
    let out = git_credential(port, "good-token", &helper, &global, "reject", reject);
    assert!(out.status.success());

    let seen = seen.lock().unwrap();
    assert_eq!(seen[0], "Get protocol=https host=git.example.com");
    assert!(
        seen[1].starts_with("Erase protocol=https host=git.example.com username=evan"),
        "{}",
        seen[1]
    );
    assert_eq!(seen.len(), 2, "no askpass prompt was needed: {seen:?}");
}

#[test]
fn credential_helper_with_no_answer_falls_back_to_askpass() {
    let dir = tempfile_dir();
    let global = dir.join("gitconfig");
    std::fs::write(&global, "").unwrap();
    let helper = credential_helper_config(&awkward_helper_path(&dir));
    let (port, seen) = fake_app("good-token");
    // Wrong token: the helper gets no fields and stays silent; askpass is then rejected too.
    let out = git_credential(
        port,
        "bad-token",
        &helper,
        &global,
        "fill",
        "protocol=https\nhost=h\n\n",
    );
    assert!(!out.status.success());
    let seen = seen.lock().unwrap();
    assert!(seen[0].starts_with("Get "), "{seen:?}");
    assert_eq!(
        seen[1], "Username for 'https://h': ",
        "fell back to GIT_ASKPASS"
    );
}

/// A fresh directory under the target dir (no tempfile dev-dependency in this crate).
fn tempfile_dir() -> std::path::PathBuf {
    let base = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let n = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir =
        base.join(format!("askpass-{n}-{:?}", thread::current().id()).replace(['(', ')'], ""));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

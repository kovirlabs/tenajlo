//! Drives real git through the real `tenajlo-askpass` binary. `git credential fill` asks
//! for a username and password via GIT_ASKPASS exactly as a fetch would, without a server.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;

use tenajlo_askpass::{Request, Response, ENV_PORT, ENV_TOKEN};

/// Minimal stand-in for the app's trampoline: answers prompts for `token` only.
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
            let prompt = req.prompt.unwrap_or_default();
            seen.lock().unwrap().push(prompt.clone());
            let answer = if req.token != token {
                None
            } else if prompt.starts_with("Username") {
                Some("ëvan".to_owned())
            } else {
                Some("s3cr3t token".to_owned())
            };
            let mut out = serde_json::to_vec(&Response { answer }).unwrap();
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
        .write_all(b"protocol=https\nhost=tmc-git01.tmus.local\n\n")
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
    assert_eq!(prompts[0], "Username for 'https://tmc-git01.tmus.local': ");
    // git percent-encodes the username inside the URL.
    assert_eq!(
        prompts[1],
        "Password for 'https://%C3%ABvan@tmc-git01.tmus.local': "
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

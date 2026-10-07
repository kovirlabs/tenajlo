//! `tenajlo-askpass`: run by git either as `GIT_ASKPASS <prompt>` or as a credential helper
//! (`credential <get|store|erase>`, fields on stdin). Forwards the request to the Tenajlo
//! app over loopback and prints the answer. Stores nothing.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

use tenajlo_askpass::{
    format_fields, parse_fields, CredentialOp, Mode, Request, Response, CREDENTIAL_ARG, ENV_PORT,
    ENV_TOKEN, MAX_LINE,
};

/// Longer than the app's own 5-minute prompt timeout, so the app always decides.
const READ_TIMEOUT: Duration = Duration::from_secs(6 * 60);

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [mode, op] if mode == CREDENTIAL_ARG => credential(op),
        _ => askpass(args.into_iter().next().unwrap_or_default()),
    }
}

fn askpass(prompt: String) -> ExitCode {
    let request = |token| Request {
        token,
        mode: Mode::Askpass,
        prompt: Some(prompt),
        op: None,
        fields: Vec::new(),
    };
    match send(request) {
        Ok(Response {
            answer: Some(answer),
            ..
        }) => {
            println!("{answer}");
            ExitCode::SUCCESS
        }
        Ok(_) => ExitCode::FAILURE,
        Err(msg) => {
            eprintln!("tenajlo-askpass: {msg}");
            ExitCode::FAILURE
        }
    }
}

/// Credential-helper mode. Always exits 0: printing nothing tells git "no answer here",
/// and git then falls back to its next helper or GIT_ASKPASS.
fn credential(op: &str) -> ExitCode {
    let Some(op) = CredentialOp::parse(op) else {
        return ExitCode::SUCCESS;
    };
    let mut input = String::new();
    let _ = std::io::stdin()
        .take(MAX_LINE as u64)
        .read_to_string(&mut input);
    let fields = parse_fields(&input);
    let request = |token| Request {
        token,
        mode: Mode::Credential,
        prompt: None,
        op: Some(op),
        fields,
    };
    match send(request) {
        Ok(res) => print!("{}", format_fields(&res.fields)),
        Err(msg) => eprintln!("tenajlo-askpass: {msg}"),
    }
    ExitCode::SUCCESS
}

fn send(request: impl FnOnce(String) -> Request) -> Result<Response, String> {
    let port: u16 = std::env::var(ENV_PORT)
        .ok()
        .and_then(|p| p.parse().ok())
        .ok_or("not started by Tenajlo")?;
    let token = std::env::var(ENV_TOKEN).map_err(|_| "not started by Tenajlo")?;

    // Loopback only, never a hostname lookup.
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("cannot reach Tenajlo: {e}"))?;
    stream
        .set_read_timeout(Some(READ_TIMEOUT))
        .map_err(|e| e.to_string())?;

    let mut line = serde_json::to_vec(&request(token)).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream.take(MAX_LINE as u64))
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&reply).map_err(|_| "unexpected reply from Tenajlo".into())
}

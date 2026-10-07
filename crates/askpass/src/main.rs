//! `tenajlo-askpass`: run by git as `GIT_ASKPASS <prompt>`. Forwards the prompt to the
//! Tenajlo app over loopback and prints the user's answer. Stores nothing.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpStream};
use std::process::ExitCode;
use std::time::Duration;

use tenajlo_askpass::{Mode, Request, Response, ENV_PORT, ENV_TOKEN, MAX_LINE};

/// Longer than the app's own 5-minute prompt timeout, so the app always decides.
const READ_TIMEOUT: Duration = Duration::from_secs(6 * 60);

fn main() -> ExitCode {
    match run() {
        Ok(Some(answer)) => {
            println!("{answer}");
            ExitCode::SUCCESS
        }
        Ok(None) => ExitCode::FAILURE,
        Err(msg) => {
            eprintln!("tenajlo-askpass: {msg}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<Option<String>, String> {
    let prompt = std::env::args().nth(1).unwrap_or_default();
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

    let request = Request {
        token,
        mode: Mode::Askpass,
        prompt: Some(prompt),
    };
    let mut line = serde_json::to_vec(&request).map_err(|e| e.to_string())?;
    line.push(b'\n');
    stream.write_all(&line).map_err(|e| e.to_string())?;

    let mut reply = String::new();
    BufReader::new(stream.take(MAX_LINE as u64))
        .read_line(&mut reply)
        .map_err(|e| e.to_string())?;
    let response: Response =
        serde_json::from_str(&reply).map_err(|_| "unexpected reply from Tenajlo")?;
    Ok(response.answer)
}

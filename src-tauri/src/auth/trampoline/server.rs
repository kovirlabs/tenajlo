//! The loopback listener: accepts `tenajlo-askpass` connections and answers each request.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tenajlo_askpass::{Mode, Request, Response, MAX_LINE};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};

use super::handlers::{answer, credential};
use super::{Inner, PromptFn, Trampoline};
use crate::auth::saved_secrets::SavedSecrets;

/// How long a connected helper has to send its request line.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

impl Trampoline {
    pub(super) async fn spawn(
        prompt: PromptFn,
        prompt_timeout: Duration,
        saved: Option<Arc<SavedSecrets>>,
    ) -> std::io::Result<Self> {
        let listener = TcpListener::bind(SocketAddr::from((Ipv4Addr::LOCALHOST, 0))).await?;
        let port = listener.local_addr()?.port();
        tracing::info!(port, "askpass trampoline listening on loopback");
        let inner = Arc::new(Inner {
            port,
            ops: Mutex::default(),
            prompt,
            prompt_timeout,
            saved,
        });
        let server = inner.clone();
        tokio::spawn(async move {
            loop {
                match listener.accept().await {
                    Ok((stream, _)) => {
                        let inner = server.clone();
                        tokio::spawn(async move { handle(&inner, stream).await });
                    }
                    Err(e) => {
                        // Errors like EMFILE persist; back off instead of spinning the loop.
                        tracing::warn!(error = %e, "trampoline accept failed");
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                }
            }
        });
        Ok(Self { inner })
    }
}

async fn handle(inner: &Inner, mut stream: TcpStream) {
    let response = match read_request(&mut stream).await {
        Some(req) if req.mode == Mode::Credential => Response {
            answer: None,
            fields: credential(inner, &req).await,
        },
        Some(req) => Response {
            answer: answer(inner, req).await,
            fields: Vec::new(),
        },
        None => Response {
            answer: None,
            fields: Vec::new(),
        },
    };
    let mut line = serde_json::to_vec(&response).unwrap_or_else(|_| b"{\"answer\":null}".to_vec());
    line.push(b'\n');
    let _ = stream.write_all(&line).await;
}

async fn read_request(stream: &mut TcpStream) -> Option<Request> {
    let mut line = String::new();
    let mut reader = BufReader::new(stream.take(MAX_LINE as u64));
    let read = tokio::time::timeout(REQUEST_TIMEOUT, reader.read_line(&mut line)).await;
    match read {
        Ok(Ok(n)) if n > 0 => serde_json::from_str(&line).ok(),
        _ => None,
    }
}

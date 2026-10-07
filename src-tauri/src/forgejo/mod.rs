//! Forgejo REST API (spec §7): server checks, sign-in, and repository listing.

pub mod address;
pub mod client;
pub mod repos;

pub use client::{ForgejoClient, ForgejoUser, ServerInfo};

/// Why a Forgejo request failed. Messages never include the token.
#[derive(Debug, thiserror::Error)]
pub enum ForgejoError {
    #[error("invalid server address: {0}")]
    InvalidUrl(String),
    #[error("certificate not trusted: {0}")]
    TlsUntrusted(String),
    #[error("server unreachable: {0}")]
    Unreachable(String),
    #[error("request timed out")]
    TimedOut,
    #[error("not a Forgejo server: {0}")]
    NotForgejo(String),
    #[error("token rejected (HTTP 401)")]
    Unauthorized,
    #[error("token lacks a required scope: {0}")]
    MissingScope(String),
    #[error("unexpected response (HTTP {status}): {body}")]
    Http { status: u16, body: String },
    #[error("request failed: {0}")]
    Other(String),
}

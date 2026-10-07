//! Wire protocol between `tenajlo-askpass` and the Tenajlo app (spec §6.5).
//!
//! One request per TCP connection: a single line of JSON from the helper, then a single
//! line of JSON back. Kept dependency-light because it ships as a sidecar binary.

use std::fmt;

use serde::{Deserialize, Serialize};

/// Loopback port of the app's trampoline listener.
pub const ENV_PORT: &str = "TENAJLO_TRAMPOLINE_PORT";
/// Per-operation token proving the request comes from a git process Tenajlo started.
pub const ENV_TOKEN: &str = "TENAJLO_TRAMPOLINE_TOKEN";
/// Upper bound for one protocol line, in bytes.
pub const MAX_LINE: usize = 64 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Invoked as `GIT_ASKPASS` / `SSH_ASKPASS` with the prompt as the only argument.
    Askpass,
}

/// Helper → app.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub token: String,
    pub mode: Mode,
    pub prompt: Option<String>,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Request")
            .field("token", &"***")
            .field("mode", &self.mode)
            .field("prompt", &self.prompt)
            .finish()
    }
}

/// App → helper. `answer: None` means the user cancelled or the request was rejected.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub answer: Option<String>,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let answer = self.answer.as_ref().map(|_| "***");
        f.debug_struct("Response").field("answer", &answer).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_secrets() {
        let req = Request {
            token: "tok-123".into(),
            mode: Mode::Askpass,
            prompt: Some("Password:".into()),
        };
        let res = Response {
            answer: Some("hunter2".into()),
        };
        let shown = format!("{req:?} {res:?}");
        assert!(
            !shown.contains("tok-123") && !shown.contains("hunter2"),
            "{shown}"
        );
    }

    #[test]
    fn wire_format() {
        let req = Request {
            token: "t".into(),
            mode: Mode::Askpass,
            prompt: Some("p".into()),
        };
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"token":"t","mode":"askpass","prompt":"p"}"#
        );
    }
}

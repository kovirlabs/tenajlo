//! Wire protocol between `tenajlo-askpass` and the Tenajlo app (spec §6.5).
//!
//! One request per TCP connection: a single line of JSON from the helper, then a single
//! line of JSON back. Kept dependency-light because it ships as a sidecar binary.

use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Loopback port of the app's trampoline listener.
pub const ENV_PORT: &str = "TENAJLO_TRAMPOLINE_PORT";
/// Per-operation token proving the request comes from a git process Tenajlo started.
pub const ENV_TOKEN: &str = "TENAJLO_TRAMPOLINE_TOKEN";
/// Upper bound for one protocol line, in bytes.
pub const MAX_LINE: usize = 64 * 1024;

/// First argument that selects credential-helper mode: `tenajlo-askpass credential <op>`.
pub const CREDENTIAL_ARG: &str = "credential";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Invoked as `GIT_ASKPASS` / `SSH_ASKPASS` with the prompt as the only argument.
    Askpass,
    /// Invoked as a git credential helper: `credential <op>`, fields on stdin.
    Credential,
}

/// git credential-helper operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialOp {
    Get,
    Store,
    Erase,
}

impl CredentialOp {
    /// Parses git's operation argument. Unknown operations (future git) are `None`.
    pub fn parse(arg: &str) -> Option<Self> {
        match arg {
            "get" => Some(Self::Get),
            "store" => Some(Self::Store),
            "erase" => Some(Self::Erase),
            _ => None,
        }
    }
}

/// `key=value` pairs of the git credential protocol, in order (keys may repeat).
pub type Fields = Vec<(String, String)>;

/// Helper → app.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Request {
    pub token: String,
    pub mode: Mode,
    pub prompt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op: Option<CredentialOp>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Fields,
}

impl fmt::Debug for Request {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let keys: Vec<&str> = self.fields.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("Request")
            .field("token", &"***")
            .field("mode", &self.mode)
            .field("prompt", &self.prompt)
            .field("op", &self.op)
            .field("fields", &keys)
            .finish()
    }
}

/// App → helper. `answer: None` means the user cancelled or the request was rejected.
/// In credential mode the reply is `fields` instead (empty = "I don't know").
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Response {
    pub answer: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fields: Fields,
}

impl fmt::Debug for Response {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let answer = self.answer.as_ref().map(|_| "***");
        let keys: Vec<&str> = self.fields.iter().map(|(k, _)| k.as_str()).collect();
        f.debug_struct("Response")
            .field("answer", &answer)
            .field("fields", &keys)
            .finish()
    }
}

/// Parses credential-protocol input: `key=value` lines up to a blank line or EOF.
/// Lines without `=` are skipped.
pub fn parse_fields(input: &str) -> Fields {
    input
        .lines() // also strips `\r\n`
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_once('='))
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
}

/// Formats fields for git. Pairs whose key or value could break the line protocol
/// (newline, NUL, `=` in the key) are dropped rather than injected.
pub fn format_fields(fields: &Fields) -> String {
    let mut out = String::new();
    for (k, v) in fields {
        let bad = |s: &str| s.contains(['\n', '\r', '\0']);
        if k.is_empty() || k.contains('=') || bad(k) || bad(v) {
            continue;
        }
        out.push_str(k);
        out.push('=');
        out.push_str(v);
        out.push('\n');
    }
    out
}

/// The `credential.helper` value that runs `askpass` in credential mode.
///
/// The `!` form makes git run the rest through its shell (MinGit's `sh` on Windows) with the
/// operation appended; without it, a quoted path would get `git credential-` prepended. The
/// path is single-quoted for paths with spaces. Backslashes become forward slashes, which
/// Windows and MinGit both accept.
pub fn credential_helper_config(askpass: &Path) -> String {
    let path = askpass.to_string_lossy();
    let path = if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    };
    format!("!'{}' {CREDENTIAL_ARG}", path.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_never_shows_secrets() {
        let req = Request {
            token: "tok-123".into(),
            mode: Mode::Credential,
            prompt: None,
            op: Some(CredentialOp::Get),
            fields: vec![("password".into(), "pw-in-req".into())],
        };
        let res = Response {
            answer: Some("hunter2".into()),
            fields: vec![("password".into(), "pw-in-res".into())],
        };
        let shown = format!("{req:?} {res:?}");
        assert!(
            !shown.contains("tok-123") && !shown.contains("hunter2") && !shown.contains("pw-in-"),
            "{shown}"
        );
    }

    #[test]
    fn wire_format() {
        let req = Request {
            token: "t".into(),
            mode: Mode::Askpass,
            prompt: Some("p".into()),
            op: None,
            fields: Vec::new(),
        };
        assert_eq!(
            serde_json::to_string(&req).unwrap(),
            r#"{"token":"t","mode":"askpass","prompt":"p"}"#
        );
        let res: Response = serde_json::from_str(r#"{"answer":null}"#).unwrap();
        assert!(res.fields.is_empty(), "old replies still parse");
    }

    #[test]
    fn credential_fields_round_trip() {
        let f = parse_fields(
            "protocol=https\r\nhost=h:3000\nwwwauth[]=Basic realm=\"x\"\njunk\n\nafter=blank\n",
        );
        assert_eq!(
            f,
            vec![
                ("protocol".into(), "https".into()),
                ("host".into(), "h:3000".into()),
                ("wwwauth[]".into(), "Basic realm=\"x\"".into()),
            ]
        );
        assert!(parse_fields("").is_empty());
        let out = format_fields(&vec![
            ("username".into(), "ëvan".into()),
            ("password".into(), "a\nprotocol=http".into()),
            ("a=b".into(), "c".into()),
            ("password".into(), "p=q".into()),
        ]);
        assert_eq!(out, "username=ëvan\npassword=p=q\n");
    }

    #[test]
    fn helper_config_quotes_the_path() {
        assert_eq!(
            credential_helper_config(Path::new("/Apps/Ten ajlo/it's/tenajlo-askpass")),
            "!'/Apps/Ten ajlo/it'\\''s/tenajlo-askpass' credential"
        );
    }
}

//! What git is asking for, parsed from askpass prompts, and the user's answer.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A prompt shown to the user. Never carries secrets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(tag = "type")]
pub enum PromptKind {
    /// Username and password/token for an HTTPS host (one dialog for git's two prompts).
    Credentials { host: String },
    /// Password only; the username is already known (e.g. `https://evan@host/…`).
    Password { host: String, username: String },
    /// Anything else git asks; shown verbatim.
    Other { prompt: String },
}

/// The user's answer. Flows UI → Rust only (CLAUDE.md rule 2). Debug output is redacted.
#[derive(Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct AuthAnswer {
    pub username: Option<String>,
    pub secret: String,
}

impl fmt::Debug for AuthAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthAnswer")
            .field("username", &self.username)
            .field("secret", &"***")
            .finish()
    }
}

/// Parses git's askpass prompt. Exec forces `LC_ALL=C`, so the wording is stable:
/// `Username for 'https://host': ` and `Password for 'https://user@host': `.
pub fn parse_prompt(prompt: &str) -> PromptKind {
    let quoted = |prefix: &str| {
        prompt
            .strip_prefix(prefix)
            .and_then(|r| r.trim_end().strip_suffix("':"))
    };
    if let Some(url) = quoted("Username for '") {
        return PromptKind::Credentials {
            host: strip_scheme(url).to_owned(),
        };
    }
    if let Some(url) = quoted("Password for '") {
        let authority = strip_scheme(url);
        if let Some((user, host)) = authority.rsplit_once('@') {
            return PromptKind::Password {
                host: host.to_owned(),
                username: percent_decode(user),
            };
        }
    }
    PromptKind::Other {
        prompt: prompt.trim().to_owned(),
    }
}

fn strip_scheme(url: &str) -> &str {
    url.split_once("://").map_or(url, |(_, rest)| rest)
}

/// Decodes `%XX` escapes (git percent-encodes usernames in prompts). Invalid escapes are kept.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let hex = |b: u8| (b as char).to_digit(16);
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex(bytes[i + 1]), hex(bytes[i + 2])) {
                out.push((h * 16 + l) as u8);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_git_prompts() {
        assert_eq!(
            parse_prompt("Username for 'https://TMC-GIT01.tmus.local': "),
            PromptKind::Credentials {
                host: "TMC-GIT01.tmus.local".into()
            }
        );
        assert_eq!(
            parse_prompt("Password for 'https://%C3%ABvan@TMC-GIT01.tmus.local:3000': "),
            PromptKind::Password {
                host: "TMC-GIT01.tmus.local:3000".into(),
                username: "ëvan".into()
            }
        );
        assert_eq!(
            parse_prompt("Enter passphrase for key '/home/e/.ssh/id_ed25519': "),
            PromptKind::Other {
                prompt: "Enter passphrase for key '/home/e/.ssh/id_ed25519':".into()
            }
        );
    }

    #[test]
    fn percent_decoding_edge_cases() {
        assert_eq!(percent_decode("a%20b"), "a b");
        assert_eq!(percent_decode("100%"), "100%");
        assert_eq!(percent_decode("%zz%4"), "%zz%4");
    }

    #[test]
    fn answer_debug_is_redacted() {
        let a = AuthAnswer {
            username: Some("evan".into()),
            secret: "hunter2".into(),
        };
        assert!(!format!("{a:?}").contains("hunter2"));
    }
}

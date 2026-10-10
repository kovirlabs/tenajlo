//! What git or OpenSSH is asking for, parsed from askpass prompts, and the user's answer.

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
    /// OpenSSH wants the passphrase for a private key. `retry` = an earlier answer was wrong.
    Passphrase { key: String, retry: bool },
    /// OpenSSH hasn't seen this server's host key. Accepting lets OpenSSH record it in
    /// `known_hosts` itself; Tenajlo never edits that file (CLAUDE.md rule 4).
    HostKey {
        host: String,
        key_type: String,
        fingerprint: String,
    },
    /// Anything else git asks; shown verbatim.
    Other { prompt: String },
}

/// The user's answer. Flows UI → Rust only (CLAUDE.md rule 2). Debug output is redacted.
#[derive(Clone, PartialEq, Eq, Deserialize, specta::Type)]
pub struct AuthAnswer {
    pub username: Option<String>,
    pub secret: String,
    /// Save this password or passphrase in the keychain (only once it's known to work, for
    /// HTTPS logins). Ignored for other prompts.
    #[serde(default)]
    pub remember: bool,
}

impl fmt::Debug for AuthAnswer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthAnswer")
            .field("username", &self.username)
            .field("secret", &"***")
            .field("remember", &self.remember)
            .finish()
    }
}

/// Parses git's askpass prompt. Exec forces `LC_ALL=C`, so the wording is stable:
/// `Username for 'https://host': ` and `Password for 'https://user@host': `.
pub fn parse_prompt(prompt: &str) -> PromptKind {
    if let Some(kind) = parse_ssh_prompt(prompt) {
        return kind;
    }
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

/// OpenSSH's askpass prompts (stable English text; OpenSSH isn't localized):
/// `Enter passphrase for key '<path>': ` and the multi-line unknown-host confirmation.
fn parse_ssh_prompt(prompt: &str) -> Option<PromptKind> {
    if let Some(rest) = prompt.strip_prefix("Enter passphrase for key '") {
        let key = rest
            .trim_end()
            .strip_suffix("':")
            .unwrap_or(rest.trim_end());
        return Some(PromptKind::Passphrase {
            key: key.to_owned(),
            retry: false,
        });
    }
    let rest = prompt.split_once("The authenticity of host '")?.1;
    let host = rest.split_once('\'')?.0;
    // OpenSSH ≥ 10: "ED25519 key fingerprint is: SHA256:…"; older: "… is SHA256:….".
    let (key_type, fingerprint) = prompt.lines().find_map(|line| {
        let (key_type, fp) = line.trim().split_once(" key fingerprint is")?;
        let fp = fp.trim_start_matches(':').trim().trim_end_matches('.');
        Some((key_type.to_owned(), fp.to_owned()))
    })?;
    if !prompt.contains("continue connecting") {
        return None;
    }
    // "[localhost]:2222 ([127.0.0.1]:2222)" → "[localhost]:2222"
    let host = host.split(" (").next().unwrap_or(host);
    Some(PromptKind::HostKey {
        host: host.to_owned(),
        key_type,
        fingerprint,
    })
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
            parse_prompt("Username for 'https://GIT.EXAMPLE.COM': "),
            PromptKind::Credentials {
                host: "GIT.EXAMPLE.COM".into()
            }
        );
        assert_eq!(
            parse_prompt("Password for 'https://%C3%ABvan@GIT.EXAMPLE.COM:3000': "),
            PromptKind::Password {
                host: "GIT.EXAMPLE.COM:3000".into(),
                username: "ëvan".into()
            }
        );
        assert_eq!(
            parse_prompt("git@h's password: "),
            PromptKind::Other {
                prompt: "git@h's password:".into()
            }
        );
    }

    #[test]
    fn parses_openssh_prompts() {
        // Captured from OpenSSH 10.3 against Forgejo on port 2222.
        let host_key = "The authenticity of host '[localhost]:2222 ([127.0.0.1]:2222)' can't be established.\nED25519 key fingerprint is: SHA256:O1QkjOlwIL6Lug/2cvCCxPHIfu9OX+W02xC/x3YTBxI\nThis key is not known by any other names.\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert_eq!(
            parse_prompt(host_key),
            PromptKind::HostKey {
                host: "[localhost]:2222".into(),
                key_type: "ED25519".into(),
                fingerprint: "SHA256:O1QkjOlwIL6Lug/2cvCCxPHIfu9OX+W02xC/x3YTBxI".into(),
            }
        );
        // Older OpenSSH (e.g. Windows' 8.x/9.x builds).
        let old = "The authenticity of host 'git.example.com (10.1.2.3)' can't be established.\r\nECDSA key fingerprint is SHA256:abc+/def.\r\nAre you sure you want to continue connecting (yes/no/[fingerprint])? ";
        assert_eq!(
            parse_prompt(old),
            PromptKind::HostKey {
                host: "git.example.com".into(),
                key_type: "ECDSA".into(),
                fingerprint: "SHA256:abc+/def".into(),
            }
        );
        assert_eq!(
            parse_prompt("Enter passphrase for key 'C:\\Users\\Evan G\\.ssh\\id_ed25519': "),
            PromptKind::Passphrase {
                key: "C:\\Users\\Evan G\\.ssh\\id_ed25519".into(),
                retry: false
            }
        );
        // Without a fingerprint line it's not something we can show safely.
        assert!(matches!(
            parse_prompt("The authenticity of host 'h' can't be established.\nAre you sure you want to continue connecting (yes/no)? "),
            PromptKind::Other { .. }
        ));
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
            remember: false,
        };
        assert!(!format!("{a:?}").contains("hunter2"));
    }
}

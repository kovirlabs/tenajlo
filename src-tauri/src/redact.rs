//! Secret redaction for anything that may reach logs or the UI.
//!
//! Every log site or error detail that might contain a remote URL, an HTTP
//! header, or git credential-protocol fields must pass through [`redact`].

const MASK: &str = "***";

/// Redacts credentials from free-form text.
///
/// Handles:
/// - URL userinfo: `https://user:token@host/x` → `https://***@host/x`
/// - HTTP `Authorization:` header values (case-insensitive)
/// - git credential-protocol `password=` lines
pub fn redact(input: &str) -> String {
    let out = redact_url_userinfo(input);
    let out = redact_after_marker(&out, "authorization:");
    redact_after_marker(&out, "password=")
}

/// Replaces `scheme://userinfo@` with `scheme://***@`.
fn redact_url_userinfo(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(idx) = rest.find("://") {
        let authority_start = idx + 3;
        out.push_str(&rest[..authority_start]);
        let after = &rest[authority_start..];
        let authority_end = after
            .find(|c: char| {
                matches!(c, '/' | '?' | '#' | '"' | '\'' | '<' | '>') || c.is_whitespace()
            })
            .unwrap_or(after.len());
        let authority = &after[..authority_end];
        match authority.rfind('@') {
            Some(at) => {
                out.push_str(MASK);
                out.push_str(&authority[at..]);
            }
            None => out.push_str(authority),
        }
        rest = &after[authority_end..];
    }
    out.push_str(rest);
    out
}

/// Masks everything after an ASCII `marker` (case-insensitive) up to end of line.
fn redact_after_marker(input: &str, marker: &str) -> String {
    // ASCII lowercasing preserves byte offsets, so indices map back to `input`.
    let lower = input.to_ascii_lowercase();
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while let Some(found) = lower[pos..].find(marker) {
        let value_start = pos + found + marker.len();
        out.push_str(&input[pos..value_start]);
        let line_end = input[value_start..]
            .find(['\n', '\r'])
            .map_or(input.len(), |n| value_start + n);
        if input[value_start..line_end].trim().is_empty() {
            out.push_str(&input[value_start..line_end]);
        } else {
            if input[value_start..].starts_with(' ') {
                out.push(' ');
            }
            out.push_str(MASK);
        }
        pos = line_end;
    }
    out.push_str(&input[pos..]);
    out
}

#[cfg(test)]
mod tests {
    use super::redact;

    #[test]
    fn masks_url_userinfo() {
        assert_eq!(
            redact("fatal: unable to access 'https://evan:abc123@TMC-GIT01.tmus.local/a/b.git/'"),
            "fatal: unable to access 'https://***@TMC-GIT01.tmus.local/a/b.git/'"
        );
    }

    #[test]
    fn masks_token_only_userinfo_and_keeps_port() {
        assert_eq!(redact("http://tok@host:3000/x"), "http://***@host:3000/x");
    }

    #[test]
    fn leaves_plain_urls_alone() {
        let s = "https://TMC-GIT01.tmus.local/org/repo.git and ssh://git@host:2222/r";
        assert_eq!(
            redact(s),
            "https://TMC-GIT01.tmus.local/org/repo.git and ssh://***@host:2222/r"
        );
    }

    #[test]
    fn masks_authorization_header() {
        assert_eq!(
            redact("> GET /api\n> Authorization: token deadbeef\n> Host: x"),
            "> GET /api\n> Authorization: ***\n> Host: x"
        );
        assert_eq!(redact("authorization:Bearer x"), "authorization:***");
    }

    #[test]
    fn masks_credential_protocol_password() {
        assert_eq!(
            redact("protocol=https\nhost=h\nusername=u\npassword=s3cr3t\n"),
            "protocol=https\nhost=h\nusername=u\npassword=***\n"
        );
    }

    #[test]
    fn handles_unicode_and_empty() {
        assert_eq!(redact(""), "");
        assert_eq!(redact("päth/ünïcode — fine"), "päth/ünïcode — fine");
        assert_eq!(redact("ö https://ü:ß@h/ö"), "ö https://***@h/ö");
    }
}

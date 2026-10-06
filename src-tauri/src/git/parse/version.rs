//! Parser for `git --version` output.

use std::fmt;

use serde::Serialize;

/// A git version triple. Vendor suffixes (`.windows.1`, `(Apple Git-154)`) are dropped.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, specta::Type)]
pub struct GitVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl GitVersion {
    /// Minimum git version Anvil supports (spec §5.1).
    pub const MINIMUM: GitVersion = GitVersion {
        major: 2,
        minor: 40,
        patch: 0,
    };
}

impl fmt::Display for GitVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Parses `git version X.Y.Z[...]`. Returns `None` for unrecognized output.
pub fn parse_version(output: &str) -> Option<GitVersion> {
    let rest = output.trim().strip_prefix("git version ")?;
    let token = rest.split_whitespace().next()?;
    let mut parts = token.split('.');
    let major = leading_number(parts.next()?)?;
    let minor = leading_number(parts.next()?)?;
    let patch = parts.next().and_then(leading_number).unwrap_or(0);
    Some(GitVersion {
        major,
        minor,
        patch,
    })
}

/// Parses the leading ASCII digits of `s` (`"0-rc1"` → 0). `None` if there are none.
fn leading_number(s: &str) -> Option<u32> {
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    s[..end].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(major: u32, minor: u32, patch: u32) -> GitVersion {
        GitVersion {
            major,
            minor,
            patch,
        }
    }

    #[test]
    fn parses_common_formats() {
        assert_eq!(parse_version("git version 2.55.0\n"), Some(v(2, 55, 0)));
        assert_eq!(
            parse_version("git version 2.47.1.windows.2\r\n"),
            Some(v(2, 47, 1))
        );
        assert_eq!(
            parse_version("git version 2.39.5 (Apple Git-154)"),
            Some(v(2, 39, 5))
        );
        assert_eq!(parse_version("git version 2.40.0-rc1"), Some(v(2, 40, 0)));
        assert_eq!(parse_version("git version 2.41"), Some(v(2, 41, 0)));
    }

    #[test]
    fn rejects_garbage() {
        assert_eq!(parse_version(""), None);
        assert_eq!(parse_version("bash: git: command not found"), None);
        assert_eq!(parse_version("git version"), None);
        assert_eq!(parse_version("git version x.y"), None);
    }

    #[test]
    fn minimum_comparison() {
        assert!(v(2, 39, 9) < GitVersion::MINIMUM);
        assert!(v(2, 40, 0) >= GitVersion::MINIMUM);
        assert!(v(3, 0, 0) > GitVersion::MINIMUM);
    }
}

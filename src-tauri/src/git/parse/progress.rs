//! Parser for git `--progress` stderr lines (spec §5.4).

use serde::Serialize;

/// One progress update, e.g. `Receiving objects: 45% (450/1000)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct Progress {
    /// `Receiving objects`, `Resolving deltas`, `Writing objects`, …
    pub phase: String,
    pub percent: Option<u8>,
    /// Remainder after the percentage, e.g. `(450/1000), 1.20 MiB | 2.00 MiB/s`.
    pub detail: Option<String>,
    /// Reported by the server (`remote:` prefix).
    pub remote: bool,
}

/// Parses one stderr line. Returns `None` for lines that aren't progress
/// (errors, hints, ref updates like `To https://…`).
pub fn parse_progress(line: &str) -> Option<Progress> {
    let line = line.trim();
    let (remote, line) = match line.strip_prefix("remote:") {
        Some(rest) => (true, rest.trim_start()),
        None => (false, line),
    };
    let (phase, rest) = line.split_once(": ")?;
    // Phases are short capitalised phrases: "Receiving objects", "Counting objects".
    let mut chars = phase.chars();
    let starts_upper = chars.next().is_some_and(|c| c.is_ascii_uppercase());
    if !starts_upper
        || phase.len() > 40
        || !phase.chars().all(|c| c.is_ascii_alphabetic() || c == ' ')
    {
        return None;
    }
    let rest = rest.trim();
    let (percent, detail) = match rest.split_once('%') {
        Some((num, after)) => match num.trim().parse::<u8>() {
            Ok(p) if p <= 100 => (Some(p), after.trim()),
            _ => (None, rest),
        },
        None => (None, rest),
    };
    let detail = detail
        .trim_end_matches(", done.")
        .trim_end_matches(", done")
        .trim();
    Some(Progress {
        phase: phase.to_owned(),
        percent,
        detail: (!detail.is_empty()).then(|| detail.to_owned()),
        remote,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_common_lines() {
        let p =
            parse_progress("Receiving objects:  45% (450/1000), 1.20 MiB | 2.00 MiB/s").unwrap();
        assert_eq!(p.phase, "Receiving objects");
        assert_eq!(p.percent, Some(45));
        assert_eq!(
            p.detail.as_deref(),
            Some("(450/1000), 1.20 MiB | 2.00 MiB/s")
        );
        assert!(!p.remote);

        let p = parse_progress("remote: Counting objects: 100% (5/5), done.").unwrap();
        assert!(p.remote);
        assert_eq!(
            (p.phase.as_str(), p.percent, p.detail.as_deref()),
            ("Counting objects", Some(100), Some("(5/5)"))
        );

        let p = parse_progress("Enumerating objects: 5, done.").unwrap();
        assert_eq!((p.percent, p.detail.as_deref()), (None, Some("5")));
    }

    #[test]
    fn ignores_non_progress() {
        for line in [
            "",
            "To https://TMC-GIT01.tmus.local/org/repo.git",
            "   abc123..def456  main -> main",
            "fatal: Authentication failed for 'https://***@h/r.git/'",
            "error: failed to push some refs to 'x'",
            "hint: Updates were rejected",
            "From https://h/r",
            " * [new branch]      feature -> origin/feature",
            "remote: ",
        ] {
            assert_eq!(parse_progress(line), None, "{line}");
        }
    }
}

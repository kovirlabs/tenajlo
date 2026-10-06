//! Parsers for `git log` records and `--name-status -z` file lists.

use serde::Serialize;

use super::status::FileStatusKind;

/// `--format` for [`parse_log`]: fields separated by NUL, records terminated by `-z`.
pub const LOG_FORMAT: &str = "--format=%H%x00%h%x00%P%x00%an%x00%ae%x00%aI%x00%s%x00%b";
const FIELDS: usize = 8;

/// One commit in the History list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Commit {
    pub sha: String,
    pub short_sha: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    /// Strict ISO 8601 with offset.
    pub author_date: String,
    pub summary: String,
    pub body: String,
}

/// A file changed by a commit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitFile {
    pub path: String,
    pub old_path: Option<String>,
    pub kind: FileStatusKind,
}

/// Parses output of `git log -z` with [`LOG_FORMAT`]. Incomplete trailing records are dropped.
pub fn parse_log(output: &[u8]) -> Vec<Commit> {
    let text = String::from_utf8_lossy(output);
    let tokens: Vec<&str> = text.split('\0').collect();
    tokens
        .as_chunks::<FIELDS>()
        .0
        .iter()
        .map(|f| Commit {
            // -z terminates each record with NUL; git may still emit a newline between records.
            sha: f[0].trim_start_matches('\n').to_owned(),
            short_sha: f[1].to_owned(),
            parents: f[2]
                .split(' ')
                .filter(|p| !p.is_empty())
                .map(str::to_owned)
                .collect(),
            author_name: f[3].to_owned(),
            author_email: f[4].to_owned(),
            author_date: f[5].to_owned(),
            summary: f[6].to_owned(),
            body: f[7].trim_end().to_owned(),
        })
        .filter(|c| !c.sha.is_empty())
        .collect()
}

/// Parses `--name-status -z`: `M\0path\0`, `R100\0old\0new\0`, `C75\0src\0dst\0`.
pub fn parse_name_status(output: &[u8]) -> Vec<CommitFile> {
    let text = String::from_utf8_lossy(output);
    let mut tokens = text.split('\0').map(|t| t.trim_start_matches('\n'));
    let mut files = Vec::new();
    while let Some(code) = tokens.next() {
        let Some(letter) = code.chars().next() else {
            continue;
        };
        let kind = match letter {
            'A' => FileStatusKind::Added,
            'D' => FileStatusKind::Deleted,
            'R' => FileStatusKind::Renamed,
            'C' => FileStatusKind::Copied,
            _ => FileStatusKind::Modified,
        };
        let entry = if matches!(letter, 'R' | 'C') {
            match (tokens.next(), tokens.next()) {
                (Some(old), Some(new)) => CommitFile {
                    path: new.to_owned(),
                    old_path: Some(old.to_owned()),
                    kind,
                },
                _ => break,
            }
        } else {
            match tokens.next() {
                Some(path) => CommitFile {
                    path: path.to_owned(),
                    old_path: None,
                    kind,
                },
                None => break,
            }
        };
        files.push(entry);
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(sha: &str, parents: &str, summary: &str, body: &str) -> String {
        format!(
            "{sha}\0{}\0{parents}\0Ëvan G\0e@x.io\02026-10-06T19:00:00-04:00\0{summary}\0{body}\0",
            &sha[..7]
        )
    }

    #[test]
    fn parses_log_records() {
        let raw = record("aaaaaaaaaa", "", "Root: ünï", "")
            + &record(
                "bbbbbbbbbb",
                "aaaaaaaaaa cccccccccc",
                "Merge",
                "line1\n\nline2\n",
            );
        let c = parse_log(raw.as_bytes());
        assert_eq!(c.len(), 2);
        assert!(c[0].parents.is_empty());
        assert_eq!(c[0].summary, "Root: ünï");
        assert_eq!(c[0].author_name, "Ëvan G");
        assert_eq!(c[1].parents.len(), 2);
        assert_eq!(c[1].body, "line1\n\nline2");
        assert_eq!(c[1].short_sha, "bbbbbbb");
    }

    #[test]
    fn empty_and_truncated_log() {
        assert!(parse_log(b"").is_empty());
        assert!(parse_log(b"abc\0ab\0").is_empty());
    }

    #[test]
    fn parses_name_status() {
        let raw = "M\0a b.txt\0R087\0old.txt\0new ü.txt\0A\0x\0D\0y\0C100\0s\0t\0T\0link\0";
        let f = parse_name_status(raw.as_bytes());
        let got: Vec<_> = f
            .iter()
            .map(|f| (f.kind, f.path.as_str(), f.old_path.as_deref()))
            .collect();
        use FileStatusKind::*;
        assert_eq!(
            got,
            vec![
                (Modified, "a b.txt", None),
                (Renamed, "new ü.txt", Some("old.txt")),
                (Added, "x", None),
                (Deleted, "y", None),
                (Copied, "t", Some("s")),
                (Modified, "link", None),
            ]
        );
    }

    #[test]
    fn name_status_tolerates_leading_newline_and_truncation() {
        let f = parse_name_status(b"\nM\0a\0R100\0only-old");
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].path, "a");
    }
}

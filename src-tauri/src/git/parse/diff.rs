//! Parser for unified diff output (`git diff` / `git show` patches).

use serde::Serialize;

/// Above this many bytes of patch text, the diff is not rendered (GitHub Desktop's LargeText limit).
pub const MAX_DIFF_BYTES: usize = 4_375_000;
/// A single line longer than this marks the diff as too large (minified files, data dumps).
pub const MAX_LINE_CHARS: usize = 5_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum DiffLineKind {
    Context,
    Add,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffLine {
    pub kind: DiffLineKind,
    /// Line content without the +/-/space marker or line ending.
    pub text: String,
    pub old_line: Option<u32>,
    pub new_line: Option<u32>,
    /// Followed by "\ No newline at end of file".
    pub no_newline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    /// The full `@@ … @@ section` header line.
    pub header: String,
    pub lines: Vec<DiffLine>,
}

/// A file's diff, ready to render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum FileDiff {
    Text {
        hunks: Vec<DiffHunk>,
    },
    Binary,
    TooLarge,
    /// No content changes (e.g. only the file mode changed).
    Unchanged,
}

/// Parses a single-file unified diff.
pub fn parse_diff(output: &[u8]) -> FileDiff {
    if output.len() > MAX_DIFF_BYTES {
        return FileDiff::TooLarge;
    }
    let text = String::from_utf8_lossy(output);
    let mut hunks: Vec<DiffHunk> = Vec::new();
    let mut old_no = 0u32;
    let mut new_no = 0u32;

    for raw in text.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let Some(hunk) = hunks.last_mut() else {
            // File header: everything before the first hunk.
            if line.starts_with("Binary files ") && line.ends_with(" differ") {
                return FileDiff::Binary;
            }
            if let Some((h, o, n)) = parse_hunk_header(line) {
                hunks.push(h);
                (old_no, new_no) = (o, n);
            }
            continue;
        };
        if let Some((h, o, n)) = parse_hunk_header(line) {
            hunks.push(h);
            (old_no, new_no) = (o, n);
            continue;
        }
        let (kind, body) = match line.as_bytes().first() {
            Some(b'+') => (DiffLineKind::Add, &line[1..]),
            Some(b'-') => (DiffLineKind::Delete, &line[1..]),
            Some(b' ') => (DiffLineKind::Context, &line[1..]),
            Some(b'\\') => {
                if let Some(prev) = hunk.lines.last_mut() {
                    prev.no_newline = true;
                }
                continue;
            }
            // Trailing empty string after the final newline, or anything unexpected.
            _ => continue,
        };
        if body.chars().count() > MAX_LINE_CHARS {
            return FileDiff::TooLarge;
        }
        let (old_line, new_line) = match kind {
            DiffLineKind::Context => (Some(old_no), Some(new_no)),
            DiffLineKind::Add => (None, Some(new_no)),
            DiffLineKind::Delete => (Some(old_no), None),
        };
        if kind != DiffLineKind::Add {
            old_no += 1;
        }
        if kind != DiffLineKind::Delete {
            new_no += 1;
        }
        hunk.lines.push(DiffLine {
            kind,
            text: body.to_owned(),
            old_line,
            new_line,
            no_newline: false,
        });
    }

    if hunks.is_empty() {
        FileDiff::Unchanged
    } else {
        FileDiff::Text { hunks }
    }
}

/// Parses `@@ -a[,b] +c[,d] @@ …`, returning the hunk and starting line numbers.
fn parse_hunk_header(line: &str) -> Option<(DiffHunk, u32, u32)> {
    let rest = line.strip_prefix("@@ -")?;
    let (ranges, _) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let start = |r: &str| r.split(',').next()?.parse::<u32>().ok();
    let hunk = DiffHunk {
        header: line.to_owned(),
        lines: Vec::new(),
    };
    Some((hunk, start(old)?, start(new)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hunks(raw: &str) -> Vec<DiffHunk> {
        match parse_diff(raw.as_bytes()) {
            FileDiff::Text { hunks } => hunks,
            other => panic!("expected text, got {other:?}"),
        }
    }

    const MODIFIED: &[&str] = &[
        "diff --git a/a.txt b/a.txt",
        "index 1111111..2222222 100644",
        "--- a/a.txt",
        "+++ b/a.txt",
        "@@ -1,3 +1,3 @@ fn main()",
        " one",
        "-two",
        "+TWO",
        " three",
        "@@ -10 +10,2 @@",
        " ten",
        "+eleven",
        "",
    ];

    #[test]
    fn parses_hunks_and_line_numbers() {
        let h = hunks(&MODIFIED.join("\n"));
        assert_eq!(h.len(), 2);
        assert_eq!(h[0].header, "@@ -1,3 +1,3 @@ fn main()");
        let nums: Vec<_> = h[0]
            .lines
            .iter()
            .map(|l| (l.kind, l.old_line, l.new_line, l.text.as_str()))
            .collect();
        use DiffLineKind::*;
        assert_eq!(
            nums,
            vec![
                (Context, Some(1), Some(1), "one"),
                (Delete, Some(2), None, "two"),
                (Add, None, Some(2), "TWO"),
                (Context, Some(3), Some(3), "three"),
            ]
        );
        assert_eq!(h[1].lines[1].new_line, Some(11));
    }

    #[test]
    fn no_newline_marker_and_crlf() {
        let raw = "--- a/x\n+++ b/x\n@@ -1 +1 @@\r\n-old\r\n\\ No newline at end of file\n+new\r\n";
        let h = hunks(raw);
        assert!(h[0].lines[0].no_newline);
        assert!(!h[0].lines[1].no_newline);
        assert_eq!(h[0].lines[1].text, "new");
    }

    #[test]
    fn header_lines_that_look_like_changes_are_ignored() {
        // "--- a/x" and "+++ b/x" precede the first hunk and must not become lines.
        let h = hunks("--- a/-x\n+++ b/+x\n@@ -0,0 +1 @@\n+ünïcode ✓\n");
        assert_eq!(h[0].lines.len(), 1);
        assert_eq!(h[0].lines[0].text, "ünïcode ✓");
    }

    #[test]
    fn binary_empty_and_mode_only() {
        assert_eq!(
            parse_diff(b"diff --git a/i.png b/i.png\nBinary files a/i.png and b/i.png differ\n"),
            FileDiff::Binary
        );
        assert_eq!(parse_diff(b""), FileDiff::Unchanged);
        assert_eq!(
            parse_diff(b"diff --git a/s b/s\nold mode 100644\nnew mode 100755\n"),
            FileDiff::Unchanged
        );
    }

    #[test]
    fn too_large() {
        let long = format!("@@ -0,0 +1 @@\n+{}\n", "x".repeat(MAX_LINE_CHARS + 1));
        assert_eq!(parse_diff(long.as_bytes()), FileDiff::TooLarge);
        assert_eq!(
            parse_diff(&vec![b' '; MAX_DIFF_BYTES + 1]),
            FileDiff::TooLarge
        );
    }
}

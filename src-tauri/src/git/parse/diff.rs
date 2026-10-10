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

/// A hunk of a unified diff with its raw line bytes, for code that rebuilds file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawHunk<'a> {
    /// The `@@ … @@ section` header line without its line ending.
    pub header: &'a [u8],
    pub old_start: u32,
    pub old_count: u32,
    pub new_start: u32,
    pub lines: Vec<RawLine<'a>>,
}

/// One line of a hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawLine<'a> {
    pub kind: DiffLineKind,
    /// Content without the +/-/space marker or the `\n`. Keeps a trailing `\r` if the file has one.
    pub body: &'a [u8],
    /// Followed by "\ No newline at end of file".
    pub no_newline: bool,
}

/// A single-file patch split into hunks, before any decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RawPatch<'a> {
    Binary,
    /// Empty when the file has no content changes.
    Hunks(Vec<RawHunk<'a>>),
}

/// Splits a single-file unified diff into hunks, keeping each line's exact bytes.
pub fn split_patch(output: &[u8]) -> RawPatch<'_> {
    let mut hunks: Vec<RawHunk> = Vec::new();
    for raw in output.split(|&b| b == b'\n') {
        let line = raw.strip_suffix(b"\r").unwrap_or(raw);
        if let Some(h) = parse_hunk_header(line) {
            hunks.push(h);
            continue;
        }
        let Some(hunk) = hunks.last_mut() else {
            // File header: everything before the first hunk.
            if line.starts_with(b"Binary files ") && line.ends_with(b" differ") {
                return RawPatch::Binary;
            }
            continue;
        };
        let kind = match raw.first() {
            Some(b'+') => DiffLineKind::Add,
            Some(b'-') => DiffLineKind::Delete,
            Some(b' ') => DiffLineKind::Context,
            Some(b'\\') => {
                if let Some(prev) = hunk.lines.last_mut() {
                    prev.no_newline = true;
                }
                continue;
            }
            // Trailing empty string after the final newline, or anything unexpected.
            _ => continue,
        };
        hunk.lines.push(RawLine {
            kind,
            body: &raw[1..],
            no_newline: false,
        });
    }
    RawPatch::Hunks(hunks)
}

/// Parses a single-file unified diff.
pub fn parse_diff(output: &[u8]) -> FileDiff {
    if output.len() > MAX_DIFF_BYTES {
        return FileDiff::TooLarge;
    }
    let raw_hunks = match split_patch(output) {
        RawPatch::Binary => return FileDiff::Binary,
        RawPatch::Hunks(h) if h.is_empty() => return FileDiff::Unchanged,
        RawPatch::Hunks(h) => h,
    };
    let mut hunks = Vec::with_capacity(raw_hunks.len());
    for raw in raw_hunks {
        let (mut old_no, mut new_no) = (raw.old_start, raw.new_start);
        let mut lines = Vec::with_capacity(raw.lines.len());
        for l in raw.lines {
            let body = l.body.strip_suffix(b"\r").unwrap_or(l.body);
            let text = String::from_utf8_lossy(body).into_owned();
            if text.chars().count() > MAX_LINE_CHARS {
                return FileDiff::TooLarge;
            }
            let (old_line, new_line) = match l.kind {
                DiffLineKind::Context => (Some(old_no), Some(new_no)),
                DiffLineKind::Add => (None, Some(new_no)),
                DiffLineKind::Delete => (Some(old_no), None),
            };
            if l.kind != DiffLineKind::Add {
                old_no += 1;
            }
            if l.kind != DiffLineKind::Delete {
                new_no += 1;
            }
            lines.push(DiffLine {
                kind: l.kind,
                text,
                old_line,
                new_line,
                no_newline: l.no_newline,
            });
        }
        hunks.push(DiffHunk {
            header: String::from_utf8_lossy(raw.header).into_owned(),
            lines,
        });
    }
    FileDiff::Text { hunks }
}

/// Parses `@@ -a[,b] +c[,d] @@ …`. A missing count means 1.
fn parse_hunk_header(line: &[u8]) -> Option<RawHunk<'_>> {
    let rest = line.strip_prefix(b"@@ -")?;
    let end = rest.windows(3).position(|w| w == b" @@")?;
    let ranges = std::str::from_utf8(&rest[..end]).ok()?;
    let (old, new) = ranges.split_once(" +")?;
    let range = |r: &str| -> Option<(u32, u32)> {
        match r.split_once(',') {
            Some((s, c)) => Some((s.parse().ok()?, c.parse().ok()?)),
            None => Some((r.parse().ok()?, 1)),
        }
    };
    let (old_start, old_count) = range(old)?;
    let (new_start, _) = range(new)?;
    Some(RawHunk {
        header: line,
        old_start,
        old_count,
        new_start,
        lines: Vec::new(),
    })
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
    fn split_patch_keeps_raw_bytes_and_counts() {
        let raw = b"--- a/x\n+++ b/x\n@@ -3,0 +4 @@\n+a\xff\r\n@@ -7 +8,2 @@ f\n-x\n\\ No newline at end of file\n+y\n+z\n";
        let RawPatch::Hunks(h) = split_patch(raw) else {
            panic!("expected hunks")
        };
        assert_eq!((h[0].old_start, h[0].old_count, h[0].new_start), (3, 0, 4));
        assert_eq!(h[0].lines[0].body, b"a\xff\r");
        assert_eq!((h[1].old_start, h[1].old_count, h[1].new_start), (7, 1, 8));
        assert_eq!(h[1].header, b"@@ -7 +8,2 @@ f");
        assert!(h[1].lines[0].no_newline);
        assert_eq!(h[1].lines.len(), 3);
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

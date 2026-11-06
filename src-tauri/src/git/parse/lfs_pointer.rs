//! Git LFS pointer files (the small text git stores in place of a large file), and spotting
//! a diff that only changes a pointer.

use serde::Serialize;

use super::diff::{DiffHunk, DiffLineKind};

/// Pointer files are at most this long (git-lfs spec).
pub const MAX_POINTER_BYTES: usize = 1024;
const VERSION_LINE: &str = "version https://git-lfs.github.com/spec/v1";

/// What a Git LFS pointer says about the real file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsPointer {
    /// SHA-256 of the content, 64 lowercase hex characters.
    pub oid: String,
    /// Size of the real file in bytes. Exported to TypeScript as `number` (the JSON carries
    /// the full value; exact up to 2^53).
    #[specta(type = u32)]
    pub size: u64,
}

/// Parses pointer file `text`, or `None` if it isn't one.
pub fn parse_pointer(text: &str) -> Option<LfsPointer> {
    if text.len() > MAX_POINTER_BYTES {
        return None;
    }
    pointer_from_lines(text.lines())
}

fn pointer_from_lines<'a>(lines: impl Iterator<Item = &'a str>) -> Option<LfsPointer> {
    let (mut version, mut oid, mut size) = (false, None, None);
    for line in lines {
        let line = line.trim_end_matches('\r');
        if line == VERSION_LINE {
            version = true;
        } else if let Some(hex) = line.strip_prefix("oid sha256:") {
            let ok = hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit());
            oid = ok.then(|| hex.to_ascii_lowercase());
        } else if let Some(n) = line.strip_prefix("size ") {
            size = n.parse::<u64>().ok();
        } else if !line.starts_with("ext-") && !line.is_empty() {
            return None;
        }
    }
    match (version, oid, size) {
        (true, Some(oid), Some(size)) => Some(LfsPointer { oid, size }),
        _ => None,
    }
}

/// The old and new pointer when every line of `hunks` belongs to an LFS pointer (the whole
/// diff of a file stored in LFS). `None` for any other diff.
pub fn pointer_change(hunks: &[DiffHunk]) -> Option<(Option<LfsPointer>, Option<LfsPointer>)> {
    let lines: Vec<_> = hunks.iter().flat_map(|h| &h.lines).collect();
    if lines.is_empty() || !lines.iter().any(|l| l.text == VERSION_LINE) {
        return None;
    }
    let side = |skip: DiffLineKind| {
        let mut texts = lines
            .iter()
            .filter(|l| l.kind != skip)
            .map(|l| l.text.as_str())
            .peekable();
        if texts.peek().is_none() {
            return Ok(None);
        }
        pointer_from_lines(texts).map(Some).ok_or(())
    };
    let old = side(DiffLineKind::Add).ok()?;
    let new = side(DiffLineKind::Delete).ok()?;
    (old.is_some() || new.is_some()).then_some((old, new))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::diff::{parse_diff, FileDiff};

    const OID_A: &str = "c52f0d630543e6f5eda8432f240153a68fa2ed86b89c1436035d57478e74ac5e";

    #[test]
    fn parses_pointers() {
        let p = parse_pointer(&format!("{VERSION_LINE}\noid sha256:{OID_A}\nsize 4096\n")).unwrap();
        assert_eq!((p.oid.as_str(), p.size), (OID_A, 4096));
        let crlf =
            format!("{VERSION_LINE}\r\next-0-foo sha256:00\r\noid sha256:{OID_A}\r\nsize 7\r\n");
        assert_eq!(parse_pointer(&crlf).unwrap().size, 7);
        assert!(parse_pointer("hello").is_none());
        assert!(parse_pointer(&format!("{VERSION_LINE}\noid sha256:xyz\nsize 1\n")).is_none());
        assert!(parse_pointer(&format!("{VERSION_LINE}\noid sha256:{OID_A}\n")).is_none());
        assert!(parse_pointer(&format!("{VERSION_LINE}\nsize 1\nmore text\n")).is_none());
    }

    // Captured from `git diff HEAD` with Tenajlo's LFS filter flags (git-lfs 3.7.1).
    const MODIFIED: &[u8] = b"diff --git a/a.bin b/a.bin
index 20c4663..2e21ac2 100644
--- a/a.bin
+++ b/a.bin
@@ -1,3 +1,3 @@
 version https://git-lfs.github.com/spec/v1
-oid sha256:c52f0d630543e6f5eda8432f240153a68fa2ed86b89c1436035d57478e74ac5e
-size 4096
+oid sha256:3727fc1d4560c5c075139889a8ff24812ca6853081b91e7410c02c20f2c0fda3
+size 6144
";

    #[test]
    fn spots_a_pointer_change() {
        let FileDiff::Lfs { old, new } = parse_diff(MODIFIED) else {
            panic!("LFS diff expected");
        };
        assert_eq!(old.unwrap().size, 4096);
        let new = new.unwrap();
        assert_eq!(new.size, 6144);
        assert!(new.oid.starts_with("3727fc1d"));
    }

    #[test]
    fn added_and_deleted_pointers_have_one_side() {
        let added = format!(
            "diff --git a/x.SLDPRT b/x.SLDPRT\nnew file mode 100644\n--- /dev/null\n+++ b/x.SLDPRT\n@@ -0,0 +1,3 @@\n+{VERSION_LINE}\n+oid sha256:{OID_A}\n+size 12\n"
        );
        let FileDiff::Lfs { old, new } = parse_diff(added.as_bytes()) else {
            panic!()
        };
        assert!(old.is_none() && new.unwrap().size == 12);

        let deleted = added
            .replace("+++ b/x", "+++ /dev/null")
            .replace("\n+", "\n-");
        let deleted = deleted.replace("@@ -0,0 +1,3 @@", "@@ -1,3 +0,0 @@");
        let FileDiff::Lfs { old, new } = parse_diff(deleted.as_bytes()) else {
            panic!()
        };
        assert!(old.unwrap().size == 12 && new.is_none());
    }

    #[test]
    fn ordinary_text_diffs_are_not_pointers() {
        let text =
            b"diff --git a/r.md b/r.md\n--- a/r.md\n+++ b/r.md\n@@ -1 +1 @@\n-size 1\n+size 2\n";
        let FileDiff::Text { hunks } = parse_diff(text) else {
            panic!()
        };
        assert!(pointer_change(&hunks).is_none());
        let doc = format!(
            "diff --git a/r.md b/r.md\n--- a/r.md\n+++ b/r.md\n@@ -1,2 +1,3 @@\n {VERSION_LINE}\n-old\n+new\n+size 3\n"
        );
        let FileDiff::Text { hunks } = parse_diff(doc.as_bytes()) else {
            panic!()
        };
        assert!(pointer_change(&hunks).is_none(), "other lines rule it out");
    }
}

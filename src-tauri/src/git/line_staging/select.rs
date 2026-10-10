//! Pure functions behind line staging: which lines of a diff are staged, and the file content
//! that results from staging a chosen set of them. No I/O in this module.

use std::collections::HashSet;

use crate::git::parse::diff::{DiffLineKind, RawHunk};

/// Per line of the working-directory diff (HEAD → working tree): whether that change is staged.
/// Context lines are always false. Indexed `[hunk][line]` like the diff itself.
pub type LineFlags = Vec<Vec<bool>>;

/// The diff the UI showed no longer matches the file (it changed in between).
#[derive(Debug, PartialEq, Eq)]
pub struct Mismatch;

/// Every change line flagged `value`, context lines false.
pub fn uniform_flags(hunks: &[RawHunk], value: bool) -> LineFlags {
    hunks
        .iter()
        .map(|h| {
            h.lines
                .iter()
                .map(|l| value && l.kind != DiffLineKind::Context)
                .collect()
        })
        .collect()
}

/// Works out which changes of `combined` (HEAD → working tree) are staged, given the staged diff
/// (HEAD → index) and the unstaged diff (index → working tree).
///
/// A removed line is staged when the index no longer has that HEAD line. An added line is staged
/// when the index already has that working-tree line. Both are judged by line number on the side
/// the diffs share, so no content matching is needed.
pub fn staged_flags(combined: &[RawHunk], staged: &[RawHunk], unstaged: &[RawHunk]) -> LineFlags {
    let removed_in_index = old_numbers(staged, DiffLineKind::Delete);
    let missing_from_index = new_numbers(unstaged, DiffLineKind::Add);
    combined
        .iter()
        .map(|h| {
            let (mut old_no, mut new_no) = (h.old_start, h.new_start);
            h.lines
                .iter()
                .map(|l| match l.kind {
                    DiffLineKind::Context => {
                        old_no += 1;
                        new_no += 1;
                        false
                    }
                    DiffLineKind::Delete => {
                        old_no += 1;
                        removed_in_index.contains(&(old_no - 1))
                    }
                    DiffLineKind::Add => {
                        new_no += 1;
                        !missing_from_index.contains(&(new_no - 1))
                    }
                })
                .collect()
        })
        .collect()
}

/// Old-side line numbers of `kind` lines.
fn old_numbers(hunks: &[RawHunk], kind: DiffLineKind) -> HashSet<u32> {
    let mut out = HashSet::new();
    for h in hunks {
        let mut no = h.old_start;
        for l in &h.lines {
            if l.kind == DiffLineKind::Add {
                continue;
            }
            if l.kind == kind {
                out.insert(no);
            }
            no += 1;
        }
    }
    out
}

/// New-side line numbers of `kind` lines.
fn new_numbers(hunks: &[RawHunk], kind: DiffLineKind) -> HashSet<u32> {
    let mut out = HashSet::new();
    for h in hunks {
        let mut no = h.new_start;
        for l in &h.lines {
            if l.kind == DiffLineKind::Delete {
                continue;
            }
            if l.kind == kind {
                out.insert(no);
            }
            no += 1;
        }
    }
    out
}

/// Builds the content of `base` (the diff's old side) with only the `selected` changes applied:
/// a selected removal drops its line, an unselected one keeps it; a selected addition is inserted,
/// an unselected one is left out. Works on bytes, so line endings and encodings are kept as-is.
///
/// Fails with [`Mismatch`] if `base` isn't the content the hunks were made from.
pub fn apply_selection(
    base: &[u8],
    hunks: &[RawHunk],
    selected: impl Fn(usize, usize) -> bool,
) -> Result<Vec<u8>, Mismatch> {
    let lines = split_lines(base);
    let mut out = Vec::with_capacity(base.len());
    let mut pos = 0usize;
    for (hi, h) in hunks.iter().enumerate() {
        // A hunk that removes nothing inserts *after* line `old_start`.
        let start = if h.old_count == 0 {
            h.old_start as usize
        } else {
            (h.old_start as usize).saturating_sub(1)
        };
        if start < pos || start > lines.len() {
            return Err(Mismatch);
        }
        for l in &lines[pos..start] {
            push_line(&mut out, l);
        }
        pos = start;
        for (li, l) in h.lines.iter().enumerate() {
            match l.kind {
                DiffLineKind::Context | DiffLineKind::Delete => {
                    let line = lines.get(pos).ok_or(Mismatch)?;
                    if line.strip_suffix(b"\n").unwrap_or(line) != l.body {
                        return Err(Mismatch);
                    }
                    pos += 1;
                    if l.kind == DiffLineKind::Context || !selected(hi, li) {
                        push_line(&mut out, line);
                    }
                }
                DiffLineKind::Add if selected(hi, li) => {
                    push_line(&mut out, l.body);
                    if !l.no_newline {
                        out.push(b'\n');
                    }
                }
                DiffLineKind::Add => {}
            }
        }
    }
    for l in &lines[pos..] {
        push_line(&mut out, l);
    }
    Ok(out)
}

/// Splits into lines, each keeping its `\n` (the last may have none).
fn split_lines(data: &[u8]) -> Vec<&[u8]> {
    data.split_inclusive(|&b| b == b'\n').collect()
}

/// Appends a line. If the previous line had no newline (it was the old last line, and now
/// something follows it), one is added first.
fn push_line(out: &mut Vec<u8>, line: &[u8]) {
    if out.last().is_some_and(|&b| b != b'\n') {
        out.push(b'\n');
    }
    out.extend_from_slice(line);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::diff::{split_patch, RawPatch};

    fn hunks(raw: &[u8]) -> Vec<RawHunk<'_>> {
        match split_patch(raw) {
            RawPatch::Hunks(h) => h,
            RawPatch::Binary => panic!("binary"),
        }
    }

    const BASE: &[u8] = b"one\ntwo\nthree\nfour\n";
    // two → TWO, and "five" appended.
    const PATCH: &[u8] = b"@@ -1,3 +1,3 @@\n one\n-two\n+TWO\n three\n@@ -4,0 +5 @@ four\n+five\n";

    #[test]
    fn applies_all_none_or_some() {
        let h = hunks(PATCH);
        assert_eq!(
            apply_selection(BASE, &h, |_, _| true).unwrap(),
            b"one\nTWO\nthree\nfour\nfive\n"
        );
        assert_eq!(apply_selection(BASE, &h, |_, _| false).unwrap(), BASE);
        // Only the addition of TWO (not the removal of two), plus five.
        assert_eq!(
            apply_selection(BASE, &h, |hi, li| !(hi == 0 && li == 1)).unwrap(),
            b"one\ntwo\nTWO\nthree\nfour\nfive\n"
        );
    }

    #[test]
    fn new_file_from_empty_base() {
        let h = hunks(b"@@ -0,0 +1,3 @@\n+a\n+b\n+c\n");
        assert_eq!(
            apply_selection(b"", &h, |_, li| li != 1).unwrap(),
            b"a\nc\n"
        );
    }

    #[test]
    fn keeps_crlf_and_non_utf8() {
        let base = b"a\r\n\xe9t\xe9\r\nc\r\n";
        let h = hunks(b"@@ -2 +2 @@\n-\xe9t\xe9\r\n+summer\r\n");
        assert_eq!(
            apply_selection(base, &h, |_, _| true).unwrap(),
            b"a\r\nsummer\r\nc\r\n"
        );
    }

    #[test]
    fn missing_final_newline() {
        // "b" (no newline) → "c" (no newline). Keeping b and adding c must separate them.
        let base = b"a\nb";
        let h = hunks(
            b"@@ -2 +2 @@\n-b\n\\ No newline at end of file\n+c\n\\ No newline at end of file\n",
        );
        assert_eq!(
            apply_selection(base, &h, |_, li| li == 1).unwrap(),
            b"a\nb\nc"
        );
        assert_eq!(apply_selection(base, &h, |_, _| true).unwrap(), b"a\nc");
    }

    #[test]
    fn rejects_a_different_base() {
        let h = hunks(PATCH);
        assert_eq!(
            apply_selection(b"one\nchanged\nthree\nfour\n", &h, |_, _| true),
            Err(Mismatch)
        );
        assert_eq!(apply_selection(b"one\n", &h, |_, _| true), Err(Mismatch));
    }

    #[test]
    fn flags_from_staged_and_unstaged_diffs() {
        let combined = hunks(PATCH);
        // Index has the removal of "two" staged but not "TWO", and "five" staged.
        let staged = hunks(b"@@ -2 +1,0 @@\n-two\n@@ -4,0 +4 @@\n+five\n");
        let unstaged = hunks(b"@@ -1,0 +2 @@\n+TWO\n");
        assert_eq!(
            staged_flags(&combined, &staged, &unstaged),
            vec![vec![false, true, false, false], vec![true]]
        );
        assert_eq!(
            uniform_flags(&combined, true),
            vec![vec![false, true, true, false], vec![true]]
        );
    }
}

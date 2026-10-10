//! Adding entries to the repository's root `.gitignore` (spec §5.3).

use std::path::Path;

/// Anchored `.gitignore` pattern matching exactly `path` (repo-relative, `/`-separated).
pub fn exact_pattern(path: &str) -> String {
    let mut out = String::from("/");
    for c in path.chars() {
        if matches!(c, '\\' | '*' | '?' | '[') {
            out.push('\\');
        }
        out.push(c);
    }
    // Trailing spaces are ignored by git unless escaped.
    let trailing = out.len() - out.trim_end_matches(' ').len();
    out.truncate(out.len() - trailing);
    out.push_str(&"\\ ".repeat(trailing));
    out
}

/// The file's extension (without the dot) if it's simple enough to ignore by: letters,
/// digits, `_` and `-`. Dotfiles like `.env` have none.
pub fn extension(path: &str) -> Option<&str> {
    let name = path.rsplit('/').next()?;
    let (stem, ext) = name.rsplit_once('.')?;
    let simple = !stem.is_empty()
        && !ext.is_empty()
        && ext
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    simple.then_some(ext)
}

/// `*.ext` pattern for the file's [`extension`], if it has one.
pub fn extension_pattern(path: &str) -> Option<String> {
    extension(path).map(|ext| format!("*.{ext}"))
}

/// Appends `pattern` to `<root>/.gitignore` unless an identical line exists.
/// Preserves the file's existing line endings.
pub fn append(root: &Path, pattern: &str) -> std::io::Result<()> {
    let path = root.join(".gitignore");
    let existing = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(e) => return Err(e),
    };
    if existing
        .lines()
        .any(|l| l.trim_end_matches('\r') == pattern)
    {
        return Ok(());
    }
    let eol = if existing.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut out = existing;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push_str(eol);
    }
    out.push_str(pattern);
    out.push_str(eol);
    std::fs::write(&path, out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::status::status;
    use crate::git::test_support::{git, init_repo, write};

    #[test]
    fn escapes_patterns() {
        assert_eq!(exact_pattern("build/out.log"), "/build/out.log");
        assert_eq!(exact_pattern("a*b?[c].txt"), "/a\\*b\\?\\[c].txt");
        assert_eq!(exact_pattern("#notes!"), "/#notes!");
        assert_eq!(exact_pattern("trail  "), "/trail\\ \\ ");
        assert_eq!(extension_pattern("dir/plc.L5X"), Some("*.L5X".into()));
        assert_eq!(extension_pattern(".env"), None);
        assert_eq!(extension_pattern("Makefile"), None);
    }

    #[test]
    fn appends_without_duplicates_and_keeps_crlf() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "a\r\nb").unwrap();
        append(dir.path(), "/c").unwrap();
        append(dir.path(), "/c").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.path().join(".gitignore")).unwrap(),
            "a\r\nb\r\n/c\r\n"
        );
    }

    #[tokio::test]
    async fn git_honours_written_patterns() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        // `[a]` is a glob that would also match xay.txt; the pattern must match only the
        // literal name. (A name with `*`, or ending in a space, isn't valid on Windows.)
        for p in ["x[a]y.txt", "xay.txt", "sub/z.tmp", "other.tmp"] {
            write(&repo, p, "1\n");
        }
        append(&repo, &exact_pattern("x[a]y.txt")).unwrap();
        if cfg!(not(windows)) {
            write(&repo, "trail ", "1\n");
            append(&repo, &exact_pattern("trail ")).unwrap();
        }
        append(&repo, &extension_pattern("sub/z.tmp").unwrap()).unwrap();
        let mut left: Vec<String> = status(&git, &repo)
            .await
            .unwrap()
            .files
            .into_iter()
            .map(|f| f.path)
            .collect();
        left.sort();
        assert_eq!(left, vec![".gitignore", "xay.txt"]);
    }
}

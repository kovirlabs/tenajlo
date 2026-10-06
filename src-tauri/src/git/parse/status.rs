//! Parser for `git status --porcelain=v2 --branch -z`.

use serde::Serialize;

/// What happened to a file, as shown in the Changes list.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum FileStatusKind {
    Untracked,
    Added,
    Modified,
    Deleted,
    Renamed,
    Copied,
    Conflicted,
}

/// How much of a file's change is staged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum StagedState {
    None,
    Partial,
    Full,
}

/// One changed path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FileChange {
    /// Repo-relative path, `/`-separated.
    pub path: String,
    /// Original path for renames and copies.
    pub old_path: Option<String>,
    pub kind: FileStatusKind,
    pub staged: StagedState,
    /// The path is a submodule (detected and shown only; spec §2).
    pub submodule: bool,
}

/// Branch information from `# branch.*` headers.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchState {
    /// Current branch name; `None` when HEAD is detached.
    pub name: Option<String>,
    /// HEAD commit; `None` in a repository with no commits yet.
    pub tip: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    /// An upstream is configured but no longer exists on the remote.
    pub upstream_gone: bool,
}

/// Parsed working directory status.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WorkingDirectoryStatus {
    pub branch: BranchState,
    pub files: Vec<FileChange>,
    pub has_conflicts: bool,
}

/// Parses porcelain v2 output. Unknown records are skipped rather than failing the whole status.
pub fn parse_status(output: &[u8]) -> WorkingDirectoryStatus {
    let text = String::from_utf8_lossy(output);
    let mut tokens = text.split('\0');
    let mut status = WorkingDirectoryStatus::default();
    let mut saw_ab = false;

    while let Some(token) = tokens.next() {
        if let Some(header) = token.strip_prefix("# ") {
            saw_ab |= parse_header(header, &mut status.branch);
            continue;
        }
        let entry = match token.as_bytes().first() {
            Some(b'1') => parse_ordinary(token),
            Some(b'2') => parse_rename(token, tokens.next()),
            Some(b'u') => parse_unmerged(token),
            Some(b'?') => token
                .get(2..)
                .map(|p| change(p, None, FileStatusKind::Untracked, StagedState::None, false)),
            _ => None,
        };
        if let Some(entry) = entry {
            // An untracked file can share a path with a staged deletion; the untracked entry wins.
            status.files.retain(|f| f.path != entry.path);
            status.files.push(entry);
        }
    }

    status.branch.upstream_gone = status.branch.upstream.is_some() && !saw_ab;
    status.has_conflicts = status
        .files
        .iter()
        .any(|f| f.kind == FileStatusKind::Conflicted);
    status
}

/// Applies one `branch.*` header. Returns true for `branch.ab`.
fn parse_header(header: &str, branch: &mut BranchState) -> bool {
    let Some((key, value)) = header.split_once(' ') else {
        return false;
    };
    match key {
        "branch.oid" if value != "(initial)" => branch.tip = Some(value.to_owned()),
        "branch.head" if value != "(detached)" => branch.name = Some(value.to_owned()),
        "branch.upstream" => branch.upstream = Some(value.to_owned()),
        "branch.ab" => {
            for part in value.split(' ') {
                if let Some(n) = part.strip_prefix('+') {
                    branch.ahead = n.parse().unwrap_or(0);
                } else if let Some(n) = part.strip_prefix('-') {
                    branch.behind = n.parse().unwrap_or(0);
                }
            }
            return true;
        }
        _ => {}
    }
    false
}

/// `1 XY sub mH mI mW hH hI path`
fn parse_ordinary(token: &str) -> Option<FileChange> {
    let fields: Vec<&str> = token.splitn(9, ' ').collect();
    let [_, xy, sub, _, _, _, _, _, path] = fields.as_slice() else {
        return None;
    };
    let (x, y) = split_xy(xy)?;
    let kind = match (x, y) {
        // Added then deleted from the working tree: nothing to show (matches GitHub Desktop).
        ('A', 'D') => return None,
        ('A', _) => FileStatusKind::Added,
        ('D', _) | (_, 'D') => FileStatusKind::Deleted,
        _ => FileStatusKind::Modified,
    };
    Some(change(path, None, kind, staged(x, y), is_submodule(sub)))
}

/// `2 XY sub mH mI mW hH hI Xscore path` followed by a NUL-separated original path.
fn parse_rename(token: &str, orig: Option<&str>) -> Option<FileChange> {
    let fields: Vec<&str> = token.splitn(10, ' ').collect();
    let [_, xy, sub, _, _, _, _, _, score, path] = fields.as_slice() else {
        return None;
    };
    let (x, y) = split_xy(xy)?;
    let kind = if score.starts_with('C') {
        FileStatusKind::Copied
    } else {
        FileStatusKind::Renamed
    };
    Some(change(path, orig, kind, staged(x, y), is_submodule(sub)))
}

/// `u XY sub m1 m2 m3 mW h1 h2 h3 path`
fn parse_unmerged(token: &str) -> Option<FileChange> {
    let fields: Vec<&str> = token.splitn(11, ' ').collect();
    let [_, _, sub, _, _, _, _, _, _, _, path] = fields.as_slice() else {
        return None;
    };
    Some(change(
        path,
        None,
        FileStatusKind::Conflicted,
        StagedState::None,
        is_submodule(sub),
    ))
}

fn split_xy(xy: &str) -> Option<(char, char)> {
    let mut chars = xy.chars();
    Some((chars.next()?, chars.next()?))
}

fn staged(x: char, y: char) -> StagedState {
    match (x, y) {
        ('.', _) => StagedState::None,
        (_, '.') => StagedState::Full,
        _ => StagedState::Partial,
    }
}

fn is_submodule(sub: &str) -> bool {
    sub.starts_with('S')
}

fn change(
    path: &str,
    old: Option<&str>,
    kind: FileStatusKind,
    staged: StagedState,
    submodule: bool,
) -> FileChange {
    FileChange {
        path: path.to_owned(),
        old_path: old.map(str::to_owned),
        kind,
        staged,
        submodule,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const H: &str = "0000000000000000000000000000000000000000";

    fn ordinary(xy: &str, path: &str) -> String {
        format!("1 {xy} N... 100644 100644 100644 {H} {H} {path}")
    }

    fn files(raw: &str) -> Vec<FileChange> {
        parse_status(raw.as_bytes()).files
    }

    #[test]
    fn parses_branch_headers() {
        let raw = "# branch.oid abc123\0# branch.head main\0# branch.upstream origin/main\0# branch.ab +2 -3\0";
        let s = parse_status(raw.as_bytes());
        assert_eq!(
            s.branch,
            BranchState {
                name: Some("main".into()),
                tip: Some("abc123".into()),
                upstream: Some("origin/main".into()),
                ahead: 2,
                behind: 3,
                upstream_gone: false,
            }
        );
    }

    #[test]
    fn unborn_detached_and_gone_upstream() {
        let s = parse_status(b"# branch.oid (initial)\0# branch.head main\0");
        assert_eq!(s.branch.tip, None);
        assert_eq!(s.branch.name.as_deref(), Some("main"));

        let s = parse_status(b"# branch.oid abc\0# branch.head (detached)\0");
        assert_eq!(s.branch.name, None);

        let s = parse_status(b"# branch.oid abc\0# branch.head f\0# branch.upstream origin/f\0");
        assert!(s.branch.upstream_gone);
    }

    #[test]
    fn empty_output() {
        assert_eq!(parse_status(b""), WorkingDirectoryStatus::default());
    }

    #[test]
    fn ordinary_entries_and_stage_state() {
        let raw = [
            ordinary(".M", "unstaged.txt"),
            ordinary("M.", "staged.txt"),
            ordinary("MM", "partial.txt"),
            ordinary("A.", "new.txt"),
            ordinary(".D", "gone.txt"),
            ordinary("AD", "dropped.txt"),
            ordinary(".T", "symlink"),
        ]
        .join("\0");
        let got: Vec<(String, FileStatusKind, StagedState)> = files(&raw)
            .into_iter()
            .map(|f| (f.path, f.kind, f.staged))
            .collect();
        use FileStatusKind as K;
        use StagedState as S;
        assert_eq!(
            got,
            vec![
                ("unstaged.txt".into(), K::Modified, S::None),
                ("staged.txt".into(), K::Modified, S::Full),
                ("partial.txt".into(), K::Modified, S::Partial),
                ("new.txt".into(), K::Added, S::Full),
                ("gone.txt".into(), K::Deleted, S::None),
                ("symlink".into(), K::Modified, S::None),
            ]
        );
    }

    #[test]
    fn paths_with_spaces_unicode_and_newlines() {
        let raw = format!(
            "{}\0? dir/ünï cödé.txt\0? weird\nname\0",
            ordinary(".M", "a b/c d.txt")
        );
        let paths: Vec<String> = files(&raw).into_iter().map(|f| f.path).collect();
        assert_eq!(
            paths,
            vec!["a b/c d.txt", "dir/ünï cödé.txt", "weird\nname"]
        );
    }

    #[test]
    fn renames_and_copies_consume_orig_path() {
        let raw = format!(
            "2 R. N... 100644 100644 100644 {H} {H} R100 new name.txt\0old name.txt\0\
             2 C. N... 100644 100644 100644 {H} {H} C75 copy.txt\0src.txt\0? after.txt\0"
        );
        let f = files(&raw);
        assert_eq!(f.len(), 3);
        assert_eq!(
            (f[0].kind, f[0].path.as_str(), f[0].old_path.as_deref()),
            (
                FileStatusKind::Renamed,
                "new name.txt",
                Some("old name.txt")
            )
        );
        assert_eq!(
            (f[1].kind, f[1].old_path.as_deref()),
            (FileStatusKind::Copied, Some("src.txt"))
        );
        assert_eq!(f[2].path, "after.txt");
    }

    #[test]
    fn conflicts_and_submodules() {
        let raw = format!(
            "u UU N... 100644 100644 100644 100644 {H} {H} {H} both.txt\0\
             1 .M SC.. 160000 160000 160000 {H} {H} vendor/lib\0"
        );
        let s = parse_status(raw.as_bytes());
        assert!(s.has_conflicts);
        assert_eq!(s.files[0].kind, FileStatusKind::Conflicted);
        assert!(s.files[1].submodule);
    }

    #[test]
    fn untracked_replaces_staged_delete_of_same_path() {
        let raw = format!("{}\0? same.txt\0", ordinary("D.", "same.txt"));
        let f = files(&raw);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].kind, FileStatusKind::Untracked);
    }

    #[test]
    fn skips_ignored_and_malformed() {
        assert!(files("! ignored.txt\0garbage\0").is_empty());
        assert!(files("1 .M short\0").is_empty());
    }
}

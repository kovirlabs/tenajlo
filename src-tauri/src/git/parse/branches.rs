//! Parser for `git for-each-ref` branch listings.

use serde::Serialize;

/// `--format` for [`parse_branches`]: NUL-separated fields, one ref per line.
/// Ref names cannot contain NUL or newlines (`git check-ref-format`).
pub const BRANCH_FORMAT: &str = "--format=%(refname)%00%(refname:short)%00%(upstream:short)%00%(objectname)%00%(symref)%00%(HEAD)%00%(committerdate:iso-strict)";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, specta::Type)]
pub enum BranchKind {
    Local,
    Remote,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    /// Short name: `main`, or `origin/main` for remote branches.
    pub name: String,
    pub kind: BranchKind,
    /// Remote name for remote branches.
    pub remote: Option<String>,
    /// Upstream (`origin/main`) for local branches that track one.
    pub upstream: Option<String>,
    pub tip: String,
    pub is_current: bool,
    /// ISO 8601 date of the tip commit.
    pub last_commit_date: String,
}

/// Branches grouped the way the branch dropdown shows them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct BranchList {
    pub current: Option<String>,
    /// Local branches, sorted by name (current included).
    pub local: Vec<Branch>,
    /// Remote branches with no local branch tracking them or sharing their name.
    pub remote_only: Vec<Branch>,
}

/// Parses `for-each-ref` output. `remotes` (from `git remote`) resolves remote names
/// that contain `/`.
pub fn parse_branches(output: &[u8], remotes: &[String]) -> BranchList {
    let text = String::from_utf8_lossy(output);
    let mut local = Vec::new();
    let mut remote = Vec::new();

    for line in text.split('\n') {
        let fields: Vec<&str> = line.trim_end_matches('\r').split('\0').collect();
        let [refname, short, upstream, tip, symref, head, date] = fields.as_slice() else {
            continue;
        };
        // Skip symbolic refs such as origin/HEAD.
        if !symref.is_empty() {
            continue;
        }
        let some = |s: &str| (!s.is_empty()).then(|| s.to_owned());
        let mut branch = Branch {
            name: (*short).to_owned(),
            kind: BranchKind::Local,
            remote: None,
            upstream: some(upstream),
            tip: (*tip).to_owned(),
            is_current: *head == "*",
            last_commit_date: (*date).to_owned(),
        };
        if let Some(rest) = refname.strip_prefix("refs/remotes/") {
            branch.kind = BranchKind::Remote;
            branch.remote = remote_name(rest, remotes);
            branch.name = rest.to_owned();
            branch.upstream = None;
            remote.push(branch);
        } else if refname.starts_with("refs/heads/") {
            local.push(branch);
        }
    }

    let tracked = |r: &Branch| {
        local.iter().any(|l| {
            l.upstream.as_deref() == Some(r.name.as_str())
                || r.remote.as_ref().is_some_and(|rem| {
                    r.name
                        .strip_prefix(rem.as_str())
                        .and_then(|n| n.strip_prefix('/'))
                        == Some(l.name.as_str())
                })
        })
    };
    let mut remote_only: Vec<Branch> = remote.iter().filter(|r| !tracked(r)).cloned().collect();
    let key = |b: &Branch| b.name.to_lowercase();
    local.sort_by_key(key);
    remote_only.sort_by_key(key);

    BranchList {
        current: local.iter().find(|b| b.is_current).map(|b| b.name.clone()),
        local,
        remote_only,
    }
}

/// Longest configured remote that prefixes `rest` (`team/origin/feature` → `team/origin`).
pub fn remote_name(rest: &str, remotes: &[String]) -> Option<String> {
    remotes
        .iter()
        .filter(|r| {
            rest.strip_prefix(r.as_str())
                .is_some_and(|tail| tail.starts_with('/'))
        })
        .max_by_key(|r| r.len())
        .cloned()
        .or_else(|| rest.split_once('/').map(|(r, _)| r.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(refname: &str, short: &str, upstream: &str, symref: &str, head: &str) -> String {
        format!(
            "{refname}\0{short}\0{upstream}\0abc123\0{symref}\0{head}\02026-10-01T10:00:00+00:00"
        )
    }

    #[test]
    fn groups_and_filters_branches() {
        let raw = [
            line("refs/heads/main", "main", "origin/main", "", "*"),
            line("refs/heads/Zeta", "Zeta", "", "", " "),
            line("refs/heads/feature/ü", "feature/ü", "", "", " "),
            line(
                "refs/remotes/origin/HEAD",
                "origin",
                "",
                "refs/remotes/origin/main",
                " ",
            ),
            line("refs/remotes/origin/main", "origin/main", "", "", " "),
            line(
                "refs/remotes/origin/feature/ü",
                "origin/feature/ü",
                "",
                "",
                " ",
            ),
            line(
                "refs/remotes/origin/only-remote",
                "origin/only-remote",
                "",
                "",
                " ",
            ),
            line("refs/remotes/team/origin/x", "team/origin/x", "", "", " "),
        ]
        .join("\n")
            + "\n";
        let remotes = vec!["origin".to_owned(), "team/origin".to_owned()];
        let list = parse_branches(raw.as_bytes(), &remotes);

        assert_eq!(list.current.as_deref(), Some("main"));
        let local: Vec<_> = list.local.iter().map(|b| b.name.as_str()).collect();
        assert_eq!(local, vec!["feature/ü", "main", "Zeta"]);
        let remote_only: Vec<_> = list
            .remote_only
            .iter()
            .map(|b| (b.name.as_str(), b.remote.as_deref()))
            .collect();
        assert_eq!(
            remote_only,
            vec![
                ("origin/only-remote", Some("origin")),
                ("team/origin/x", Some("team/origin"))
            ]
        );
    }

    #[test]
    fn empty_and_detached() {
        assert_eq!(parse_branches(b"", &[]), BranchList::default());
        let list = parse_branches(line("refs/heads/main", "main", "", "", " ").as_bytes(), &[]);
        assert_eq!(list.current, None);
        assert_eq!(list.local.len(), 1);
    }
}

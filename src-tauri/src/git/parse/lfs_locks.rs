//! Parses `git lfs locks --json` (with and without `--verify`) and the `--json` replies of
//! `git lfs lock/unlock`.

use serde::{Deserialize, Serialize};

/// A Git LFS file lock on the server.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LfsLock {
    /// Server-side lock id (a string in the LFS API).
    pub id: String,
    /// Repository-relative path with `/` separators.
    pub path: String,
    /// The server user name of whoever holds the lock.
    pub owner: String,
    /// RFC 3339 time the lock was taken (as the server reports it).
    pub locked_at: String,
    /// The lock belongs to the signed-in user (so it can be unlocked without forcing).
    pub ours: bool,
}

#[derive(Deserialize)]
struct RawOwner {
    #[serde(default)]
    name: String,
}

#[derive(Deserialize)]
struct RawLock {
    id: String,
    path: String,
    #[serde(default)]
    owner: Option<RawOwner>,
    #[serde(default)]
    locked_at: String,
}

#[derive(Deserialize)]
struct RawVerify {
    #[serde(default)]
    ours: Vec<RawLock>,
    #[serde(default)]
    theirs: Vec<RawLock>,
}

/// One entry of `git lfs unlock --json`.
#[derive(Deserialize)]
struct RawUnlock {
    #[serde(default)]
    unlocked: bool,
    #[serde(default)]
    reason: Option<String>,
}

fn convert(raw: RawLock, ours: bool) -> LfsLock {
    LfsLock {
        id: raw.id,
        path: raw.path.replace('\\', "/"),
        owner: raw.owner.map(|o| o.name).unwrap_or_default(),
        locked_at: raw.locked_at,
        ours,
    }
}

/// The JSON part of git-lfs output: it may print hints before it, so parse from the first
/// `{` or `[`.
fn json_start(stdout: &[u8]) -> &[u8] {
    let start = stdout
        .iter()
        .position(|b| *b == b'{' || *b == b'[')
        .unwrap_or(stdout.len());
    &stdout[start..]
}

/// Parses `git lfs locks --verify --json` (`{"ours":[…],"theirs":[…]}`), sorted by path.
pub fn parse_verified_locks(stdout: &[u8]) -> Result<Vec<LfsLock>, String> {
    let raw: RawVerify = serde_json::from_slice(json_start(stdout)).map_err(|e| e.to_string())?;
    let mut locks: Vec<LfsLock> = raw
        .ours
        .into_iter()
        .map(|l| convert(l, true))
        .chain(raw.theirs.into_iter().map(|l| convert(l, false)))
        .collect();
    locks.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(locks)
}

/// Parses `git lfs locks --json` (a plain array): every lock, none marked ours yet.
pub fn parse_locks(stdout: &[u8]) -> Result<Vec<LfsLock>, String> {
    let json = json_start(stdout);
    if json.is_empty() {
        return Ok(Vec::new());
    }
    let raw: Vec<RawLock> = serde_json::from_slice(json).map_err(|e| e.to_string())?;
    Ok(raw.into_iter().map(|l| convert(l, false)).collect())
}

/// Every lock in `all`, marked ours when `verified` says so, sorted by path. The verify
/// endpoint needs write access, so for a read-only user it answers with nothing at all:
/// `all` is the full list and `verified` only says which locks are ours. Locks taken
/// between the two requests (only in `verified`) are kept too.
pub fn merge_verified(all: Vec<LfsLock>, verified: Vec<LfsLock>) -> Vec<LfsLock> {
    let ours = |id: &str| verified.iter().any(|v| v.id == id && v.ours);
    let mut locks: Vec<LfsLock> = all
        .into_iter()
        .map(|mut l| {
            l.ours = ours(&l.id);
            l
        })
        .collect();
    for v in &verified {
        if !locks.iter().any(|l| l.id == v.id) {
            locks.push(v.clone());
        }
    }
    locks.sort_by(|a, b| a.path.cmp(&b.path));
    locks
}

/// Parses `git lfs lock --json`: the new lock (git-lfs 3 prints an array, older ones an object).
pub fn parse_new_lock(stdout: &[u8]) -> Result<LfsLock, String> {
    let json = json_start(stdout);
    let raw: RawLock = if json.first() == Some(&b'[') {
        let mut all: Vec<RawLock> = serde_json::from_slice(json).map_err(|e| e.to_string())?;
        if all.is_empty() {
            return Err("git-lfs returned no lock".into());
        }
        all.swap_remove(0)
    } else {
        serde_json::from_slice(json).map_err(|e| e.to_string())?
    };
    Ok(convert(raw, true))
}

/// Parses `git lfs unlock --json`: `Err(reason)` when the server kept the lock.
pub fn parse_unlock(stdout: &[u8]) -> Result<(), String> {
    let json = json_start(stdout);
    let all: Vec<RawUnlock> = if json.first() == Some(&b'[') {
        serde_json::from_slice(json).map_err(|e| e.to_string())?
    } else {
        vec![serde_json::from_slice(json).map_err(|e| e.to_string())?]
    };
    match all.into_iter().find(|u| !u.unlocked) {
        Some(failed) => Err(failed.reason.unwrap_or_else(|| "not unlocked".into())),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Captured from git-lfs 3.7.1 against Forgejo 11.
    const VERIFY: &str = r#"{"ours":[{"id":"1","path":"a.bin","owner":{"name":"tenajlo"},"locked_at":"2026-11-05T23:54:48Z"}],"theirs":[{"id":"7","path":"CAD/Pumpe Gehäuse.SLDPRT","owner":{"name":"alice"},"locked_at":"2026-11-05T23:56:43Z"}]}"#;

    // Captured from git-lfs 3.7.1 against Forgejo 11 (plain `git lfs locks --json`).
    const PLAIN: &str = r#"warning: current Git remote contains credentials
[{"id":"6","path":"a.bin","owner":{"name":"tenajlo"},"locked_at":"2026-11-06T22:55:55Z"},{"id":"7","path":"CAD\\Pumpe Gehäuse.SLDPRT","owner":null,"locked_at":"2026-11-06T22:56:00Z"}]"#;

    #[test]
    fn plain_listing_has_every_lock_none_ours() {
        let locks = parse_locks(PLAIN.as_bytes()).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[1].path, "CAD/Pumpe Gehäuse.SLDPRT");
        assert_eq!(locks[1].owner, "");
        assert!(locks.iter().all(|l| !l.ours));
        assert_eq!(parse_locks(b"").unwrap(), Vec::new());
        assert_eq!(parse_locks(b"[]").unwrap(), Vec::new());
    }

    #[test]
    fn merge_marks_ours_and_keeps_what_verify_hid() {
        let all = parse_locks(PLAIN.as_bytes()).unwrap();
        // A read-only user: verify answers with nothing, every lock is still listed.
        let reader = merge_verified(all.clone(), Vec::new());
        assert_eq!(reader.len(), 2);
        assert!(reader.iter().all(|l| !l.ours));

        // The holder of lock 6, plus a lock taken between the two requests.
        let verified = parse_verified_locks(
            br#"{"ours":[{"id":"6","path":"a.bin","owner":{"name":"tenajlo"}},{"id":"9","path":"b.bin","owner":{"name":"tenajlo"}}],"theirs":[]}"#,
        )
        .unwrap();
        let mine = merge_verified(all, verified);
        let summary: Vec<(&str, bool)> = mine.iter().map(|l| (l.id.as_str(), l.ours)).collect();
        assert_eq!(summary, [("7", false), ("6", true), ("9", true)]);
    }

    #[test]
    fn verified_locks_split_ours_and_theirs() {
        let locks = parse_verified_locks(VERIFY.as_bytes()).unwrap();
        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].path, "CAD/Pumpe Gehäuse.SLDPRT");
        assert_eq!(locks[0].owner, "alice");
        assert!(!locks[0].ours);
        assert_eq!((locks[1].id.as_str(), locks[1].ours), ("1", true));
        assert_eq!(locks[1].locked_at, "2026-11-05T23:54:48Z");
    }

    #[test]
    fn hints_before_json_and_empty_lists() {
        let out = b"hint: The remote resolves to a file:// URL\n{\"ours\":[],\"theirs\":[]}\n";
        assert!(parse_verified_locks(out).unwrap().is_empty());
        assert!(parse_verified_locks(b"").is_err());
        assert!(parse_verified_locks(b"{}").unwrap().is_empty());
    }

    #[test]
    fn windows_separators_are_normalized() {
        let out =
            br#"{"ours":[{"id":"2","path":"plc\\main.L5X","owner":{"name":"me"},"locked_at":""}]}"#;
        assert_eq!(parse_verified_locks(out).unwrap()[0].path, "plc/main.L5X");
    }

    #[test]
    fn new_lock_array_or_object() {
        let arr = br#"[{"id":"2","path":"a.bin","owner":{"name":"alice"},"locked_at":"2026-11-05T23:56:43Z"}]"#;
        let lock = parse_new_lock(arr).unwrap();
        assert_eq!(
            (lock.id.as_str(), lock.owner.as_str(), lock.ours),
            ("2", "alice", true)
        );
        let obj = br#"{"id":"3","path":"b.bin","owner":{"name":"alice"},"locked_at":""}"#;
        assert_eq!(parse_new_lock(obj).unwrap().path, "b.bin");
        assert!(parse_new_lock(b"[]").is_err());
    }

    #[test]
    fn unlock_reports_the_servers_reason() {
        assert!(parse_unlock(br#"[{"path":"a.bin","unlocked":true}]"#).is_ok());
        let err = parse_unlock(
            br#"[{"path":"a.bin","unlocked":false,"reason":"unable to delete lock : Internal Server Error"}]"#,
        )
        .unwrap_err();
        assert!(err.contains("unable to delete lock"));
    }
}

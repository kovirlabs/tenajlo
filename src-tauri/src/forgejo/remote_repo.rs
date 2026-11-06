//! Which Forgejo repository a git remote points at (spec §7, v1.4): `owner/name` on an
//! account's server, from an HTTPS, `ssh://` or scp-like (`git@host:owner/name.git`) URL.

use reqwest::Url;

use crate::auth::accounts::hosts_remote;

/// A repository on a Forgejo server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

impl RepoRef {
    /// `owner/name`, as Forgejo's `full_name`.
    pub fn full_name(&self) -> String {
        format!("{}/{}", self.owner, self.name)
    }
}

/// The repository `remote_url` names on the server at `base_url` (an account's normalized
/// address), or `None` if the remote is on another server or isn't `owner/name`.
/// `ssh_host` is the account's SSH host override (`host[:port]`), if any.
pub fn repo_for_remote(
    base_url: &str,
    ssh_host: Option<&str>,
    remote_url: &str,
) -> Option<RepoRef> {
    let base = Url::parse(base_url).ok()?;
    let base_host = base.host_str()?;
    let same_host = |host: &str| {
        host.eq_ignore_ascii_case(base_host)
            || ssh_host
                .and_then(|h| h.split(':').next())
                .is_some_and(|h| host.eq_ignore_ascii_case(h))
    };

    let path = if let Ok(url) = Url::parse(remote_url) {
        match url.scheme() {
            "https" | "http" => {
                if !hosts_remote(base_url, remote_url) {
                    return None;
                }
                let prefix = base.path().trim_end_matches('/');
                url.path().strip_prefix(prefix)?.to_owned()
            }
            "ssh" => {
                if !same_host(url.host_str()?) {
                    return None;
                }
                url.path().to_owned()
            }
            _ => return None,
        }
    } else {
        // scp-like: [user@]host:path
        let (user_host, path) = remote_url.split_once(':')?;
        let host = user_host.rsplit('@').next()?;
        if host.contains('/') || !same_host(host) {
            return None;
        }
        path.to_owned()
    };
    split_owner_name(&path)
}

/// `/owner/name.git` → `owner/name`. Names use Forgejo's characters: letters, digits, `-`,
/// `_`, `.` (so they're safe in API paths).
fn split_owner_name(path: &str) -> Option<RepoRef> {
    let trimmed = path.trim_matches('/');
    let trimmed = trimmed.strip_suffix(".git").unwrap_or(trimmed);
    let (owner, name) = trimmed.split_once('/')?;
    let valid = |s: &str| {
        !s.is_empty()
            && !s.starts_with('.')
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    (valid(owner) && valid(name)).then(|| RepoRef {
        owner: owner.to_owned(),
        name: name.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(owner: &str, name: &str) -> Option<RepoRef> {
        Some(RepoRef {
            owner: owner.into(),
            name: name.into(),
        })
    }

    #[test]
    fn https_remotes() {
        let base = "https://git.example.com";
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/team/pump-station.git"),
            r("team", "pump-station")
        );
        assert_eq!(
            repo_for_remote(base, None, "https://sam@git.example.com/team/pump_station"),
            r("team", "pump_station")
        );
        assert_eq!(
            repo_for_remote(base, None, "https://other.example.com/team/x.git"),
            None
        );
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/team/x/y.git"),
            None
        );
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/team.git"),
            None
        );
    }

    #[test]
    fn https_remotes_under_a_sub_path() {
        let base = "https://git.example.com/forgejo";
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/forgejo/team/x.git"),
            r("team", "x")
        );
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/team/x.git"),
            None
        );
    }

    #[test]
    fn ssh_remotes() {
        let base = "https://git.example.com";
        assert_eq!(
            repo_for_remote(base, None, "ssh://git@git.example.com:2222/team/x.git"),
            r("team", "x")
        );
        assert_eq!(
            repo_for_remote(base, None, "git@GIT.example.com:team/x.git"),
            r("team", "x")
        );
        assert_eq!(repo_for_remote(base, None, "git@other:team/x.git"), None);
        assert_eq!(
            repo_for_remote(
                base,
                Some("ssh.example.com:2222"),
                "git@ssh.example.com:team/x.git"
            ),
            r("team", "x")
        );
    }

    #[test]
    fn rejects_odd_names_and_local_paths() {
        let base = "https://git.example.com";
        assert_eq!(repo_for_remote(base, None, "C:\\src\\x"), None);
        assert_eq!(repo_for_remote(base, None, "/srv/git/x.git"), None);
        assert_eq!(repo_for_remote(base, None, "file:///srv/team/x.git"), None);
        assert_eq!(
            repo_for_remote(base, None, "https://git.example.com/team/%2e%2e"),
            None
        );
        assert_eq!(
            repo_for_remote(base, None, "git@git.example.com:../x.git"),
            None
        );
    }
}

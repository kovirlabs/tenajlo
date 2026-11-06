//! Commit history and commit file lists (spec §5.3).

use std::collections::HashSet;
use std::path::Path;

use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::log::{parse_log, parse_name_status, Commit, CommitFile, LOG_FORMAT};
use super::remote;

/// Most commits [`mark_unpushed`] looks at. A branch rarely has this many commits that aren't
/// on a remote; past it, older commits just aren't marked.
const MAX_UNPUSHED: u32 = 5000;

/// Up to `limit` commits reachable from HEAD, skipping the first `skip`. Empty for a new repository.
pub async fn history(
    git: &GitBinary,
    root: &Path,
    skip: u32,
    limit: u32,
) -> Result<Vec<Commit>, GitError> {
    let out = GitCommand::new(
        [
            "log".to_owned(),
            LOG_FORMAT.to_owned(),
            "-z".to_owned(),
            "--no-show-signature".to_owned(),
            "--no-color".to_owned(),
            format!("--max-count={limit}"),
            format!("--skip={skip}"),
            "--end-of-options".to_owned(),
            "HEAD".to_owned(),
            "--".to_owned(),
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .ok_exit_codes(&[0, 128])
    .run(git)
    .await?;

    if out.exit_code == Some(128) {
        // Unborn HEAD: no commits yet. Messages are stable because exec forces LC_ALL=C.
        // Anything else is a real error.
        const UNBORN: [&str; 3] = [
            "bad revision 'HEAD'",
            "unknown revision",
            "does not have any commits yet",
        ];
        if UNBORN.iter().any(|m| out.stderr.contains(m)) {
            return Ok(Vec::new());
        }
        return Err(GitError::failed(&out.stderr, &out.stdout, out.exit_code));
    }
    let mut commits = parse_log(&out.stdout);
    mark_unpushed(git, root, &mut commits).await?;
    Ok(commits)
}

/// Marks the commits that no remote-tracking branch contains yet. Repositories without a
/// remote are left unmarked: everything there is local, so the marker would add nothing.
async fn mark_unpushed(
    git: &GitBinary,
    root: &Path,
    commits: &mut [Commit],
) -> Result<(), GitError> {
    if commits.is_empty() || remote::list_remotes(git, root).await?.is_empty() {
        return Ok(());
    }
    let out = GitCommand::new(
        [
            "rev-list".to_owned(),
            format!("--max-count={MAX_UNPUSHED}"),
            "HEAD".to_owned(),
            "--not".to_owned(),
            "--remotes".to_owned(),
            "--".to_owned(),
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let unpushed: HashSet<&str> = stdout.lines().collect();
    for commit in commits {
        commit.unpushed = unpushed.contains(commit.sha.as_str());
    }
    Ok(())
}

/// Files changed by `sha`, against its first parent (merge commits included).
pub async fn commit_files(
    git: &GitBinary,
    root: &Path,
    sha: &str,
) -> Result<Vec<CommitFile>, GitError> {
    let out = GitCommand::new(
        [
            "show",
            "--format=",
            "--name-status",
            "-z",
            "-M",
            "--diff-merges=first-parent",
            "--no-color",
            "--end-of-options",
            sha,
            "--",
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(parse_name_status(&out.stdout))
}

/// True if `s` looks like a (possibly abbreviated) commit hash. Rejects options and ref syntax.
pub fn is_commit_hash(s: &str) -> bool {
    (4..=64).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::parse::status::FileStatusKind;
    use crate::git::test_support::{commit_all, git, git_in, init_repo, with_bare_origin, write};

    #[tokio::test]
    async fn marks_commits_not_on_a_remote() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        write(&repo, "a.txt", "1\n");
        commit_all(&repo, "pushed").await;
        let local = history(&git, &repo, 0, 50).await.unwrap();
        assert!(!local[0].unpushed, "no remote: nothing is marked");

        with_bare_origin(&repo).await;
        let unpublished = history(&git, &repo, 0, 50).await.unwrap();
        assert!(
            unpublished[0].unpushed,
            "remote exists but the branch isn't on it"
        );

        git_in(&repo, &["push", "-q", "-u", "origin", "main"]).await;
        write(&repo, "a.txt", "2\n");
        commit_all(&repo, "local only").await;
        let all = history(&git, &repo, 0, 50).await.unwrap();
        let marks: Vec<_> = all
            .iter()
            .map(|c| (c.summary.as_str(), c.unpushed))
            .collect();
        assert_eq!(marks, [("local only", true), ("pushed", false)]);
    }

    #[tokio::test]
    async fn history_and_files() {
        let (_tmp, repo) = init_repo().await;
        let git = git();
        assert!(
            history(&git, &repo, 0, 50).await.unwrap().is_empty(),
            "unborn HEAD is empty, not an error"
        );

        write(&repo, "a.txt", "1\n");
        commit_all(&repo, "first").await;
        git_in(&repo, &["switch", "-q", "-c", "side"]).await;
        write(&repo, "side.txt", "s\n");
        commit_all(&repo, "side work").await;
        git_in(&repo, &["switch", "-q", "main"]).await;
        write(&repo, "ü dir/b.txt", "2\n");
        git_in(&repo, &["add", "-A"]).await;
        git_in(&repo, &["commit", "-q", "-m", "second", "-m", "body text"]).await;
        git_in(
            &repo,
            &["merge", "-q", "--no-ff", "-m", "merge side", "side"],
        )
        .await;

        let all = history(&git, &repo, 0, 50).await.unwrap();
        let summaries: Vec<_> = all.iter().map(|c| c.summary.as_str()).collect();
        assert_eq!(summaries.len(), 4);
        assert_eq!(summaries[0], "merge side");
        let second = all.iter().find(|c| c.summary == "second").unwrap();
        assert_eq!(second.body, "body text");

        let page = history(&git, &repo, 1, 2).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].sha, all[1].sha);

        // Merge commit files are shown against the first parent.
        let merge_files = commit_files(&git, &repo, &all[0].sha).await.unwrap();
        assert_eq!(merge_files.len(), 1);
        assert_eq!(
            (merge_files[0].path.as_str(), merge_files[0].kind),
            ("side.txt", FileStatusKind::Added)
        );

        let files = commit_files(&git, &repo, &second.sha).await.unwrap();
        assert_eq!(files[0].path, "ü dir/b.txt");
    }

    #[test]
    fn validates_hashes() {
        assert!(is_commit_hash("abc1234"));
        assert!(is_commit_hash(&"f".repeat(40)));
        assert!(!is_commit_hash("--output=/tmp/x"));
        assert!(!is_commit_hash("HEAD~1"));
        assert!(!is_commit_hash("abc"));
    }
}

//! Staging and unstaging individual lines of a changed file (spec §5.3, v1.1).
//!
//! The UI shows one diff per file, HEAD → working tree, as for whole-file staging. Each change
//! line carries a "staged" flag worked out from the index. Staging lines rebuilds the file's
//! index entry as HEAD's content plus the selected changes, so the real index stays the source
//! of truth and command-line git sees the same thing. Callers hold the repository's mutation
//! lock for [`set_lines_staged`].

mod select;
#[cfg(test)]
mod tests;

use std::hash::{DefaultHasher, Hasher};
use std::path::Path;

use serde::{Deserialize, Serialize};

use super::diff::{raw_diff, working_dir_patch, WorkingPatch};
use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};
use super::parse::diff::{parse_diff, split_patch, DiffLineKind, FileDiff, RawHunk, RawPatch};
use super::parse::status::{FileChange, FileStatusKind, StagedState};
use select::{apply_selection, staged_flags, uniform_flags, LineFlags};

/// Line staging state for a working-directory diff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LineStaging {
    /// Identifies the diff these flags belong to. Sent back with [`set_lines_staged`] so a file
    /// that changed in between is never staged against a stale view.
    pub token: String,
    /// `[hunk][line]` like the diff's hunks: whether that change is staged. Context lines are false.
    pub staged: Vec<Vec<bool>>,
}

/// A working-directory file's diff with its line staging state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
pub struct WorkingDiff {
    pub diff: FileDiff,
    /// `None` when the file can only be staged whole.
    pub lines: Option<LineStaging>,
}

/// A line in a diff, by position.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, specta::Type)]
pub struct LineRef {
    pub hunk: u32,
    pub line: u32,
}

/// Why lines couldn't be staged.
#[derive(Debug, thiserror::Error)]
pub enum LineStageError {
    #[error(transparent)]
    Git(#[from] GitError),
    /// The file changed after the diff was shown.
    #[error("file changed since the diff was shown")]
    Changed,
    /// This kind of file can only be staged whole.
    #[error("line staging isn't supported for this file")]
    Unsupported,
    /// A line reference points outside the diff or at an unchanged line.
    #[error("line reference is not a changed line")]
    BadLine,
}

/// The working-directory diff of `file`, plus line staging state when the file supports it.
pub async fn working_diff(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
) -> Result<WorkingDiff, GitError> {
    let whole = |diff| WorkingDiff { diff, lines: None };
    let bytes = match working_dir_patch(git, root, file, has_head).await? {
        WorkingPatch::Settled(diff) => return Ok(whole(diff)),
        WorkingPatch::Raw(bytes) => bytes,
    };
    let diff = parse_diff(&bytes);
    if !matches!(diff, FileDiff::Text { .. }) || !supported(git, root, file, has_head).await? {
        return Ok(whole(diff));
    }
    let RawPatch::Hunks(hunks) = split_patch(&bytes) else {
        return Ok(whole(diff));
    };
    let staged = current_flags(git, root, file, &hunks).await?;
    let staging = LineStaging {
        token: token(&bytes),
        staged,
    };
    Ok(WorkingDiff {
        diff,
        lines: Some(staging),
    })
}

/// Stages (`stage = true`) or unstages the given change lines of `file`, leaving the staged
/// state of its other lines as it is. `token` comes from the [`LineStaging`] the lines refer to.
pub async fn set_lines_staged(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
    token: &str,
    lines: &[LineRef],
    stage: bool,
) -> Result<(), LineStageError> {
    let WorkingPatch::Raw(bytes) = working_dir_patch(git, root, file, has_head).await? else {
        return Err(LineStageError::Changed);
    };
    if self::token(&bytes) != token {
        return Err(LineStageError::Changed);
    }
    if !supported(git, root, file, has_head).await? {
        return Err(LineStageError::Unsupported);
    }
    let RawPatch::Hunks(hunks) = split_patch(&bytes) else {
        return Err(LineStageError::Unsupported);
    };

    let mut flags = current_flags(git, root, file, &hunks).await?;
    for r in lines {
        let (h, l) = (r.hunk as usize, r.line as usize);
        match hunks.get(h).and_then(|hunk| hunk.lines.get(l)) {
            Some(line) if line.kind != DiffLineKind::Context => flags[h][l] = stage,
            _ => return Err(LineStageError::BadLine),
        }
    }

    // Untracked and newly added files have no HEAD version: the index starts from nothing.
    let in_head = has_head
        && matches!(
            file.kind,
            FileStatusKind::Modified | FileStatusKind::Deleted
        );
    let any = flags.iter().flatten().any(|&f| f);
    let all = flags == uniform_flags(&hunks, true);
    if (!in_head && !any) || (file.kind == FileStatusKind::Deleted && all) {
        // Nothing of a new file is staged, or all of a deletion: the path leaves the index.
        return Ok(remove_from_index(git, root, &file.path).await?);
    }
    let base = if in_head {
        head_blob(git, root, &file.path).await?
    } else {
        Vec::new()
    };
    let content =
        apply_selection(&base, &hunks, |h, l| flags[h][l]).map_err(|_| LineStageError::Changed)?;
    let mode = match entry_mode(git, root, &file.path, has_head).await? {
        Some(mode) => mode,
        None => new_file_mode(&root.join(&file.path)).to_owned(),
    };
    write_index_entry(git, root, &file.path, &mode, content).await?;
    Ok(())
}

/// Identifies one exact diff output. Only compared within a running app, never stored.
fn token(patch: &[u8]) -> String {
    let mut h = DefaultHasher::new();
    h.write(patch);
    format!("{:016x}", h.finish())
}

/// Which change lines are staged right now.
async fn current_flags(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    hunks: &[RawHunk<'_>],
) -> Result<LineFlags, GitError> {
    match file.staged {
        StagedState::None => Ok(uniform_flags(hunks, false)),
        StagedState::Full => Ok(uniform_flags(hunks, true)),
        StagedState::Partial => {
            let staged = raw_diff(git, root, &["--cached"], &file.path).await?;
            let unstaged = raw_diff(git, root, &[], &file.path).await?;
            let (RawPatch::Hunks(staged), RawPatch::Hunks(unstaged)) =
                (split_patch(&staged), split_patch(&unstaged))
            else {
                return Ok(uniform_flags(hunks, false));
            };
            Ok(staged_flags(hunks, &staged, &unstaged))
        }
    }
}

/// Whether `file` can be staged line by line. Renames, conflicts, submodules and symlinks can't,
/// nor can files whose stored form differs from their text (LFS and other filters, or a
/// `working-tree-encoding`), since the diff then doesn't show what the index holds.
async fn supported(
    git: &GitBinary,
    root: &Path,
    file: &FileChange,
    has_head: bool,
) -> Result<bool, GitError> {
    let kind_ok = matches!(
        file.kind,
        FileStatusKind::Untracked
            | FileStatusKind::Added
            | FileStatusKind::Modified
            | FileStatusKind::Deleted
    );
    if !kind_ok || file.submodule {
        return Ok(false);
    }
    if let Some(mode) = entry_mode(git, root, &file.path, has_head).await? {
        if mode == "120000" || mode == "160000" {
            return Ok(false);
        }
    } else if tokio::fs::symlink_metadata(root.join(&file.path))
        .await
        .is_ok_and(|m| m.file_type().is_symlink())
    {
        return Ok(false);
    }
    let out = GitCommand::new(
        [
            "check-attr",
            "-z",
            "filter",
            "working-tree-encoding",
            "--",
            &file.path,
        ],
        Access::ReadOnly,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(attrs_allow_line_staging(&out.stdout))
}

/// True when `check-attr -z` output (`path\0attr\0value\0…`) has every attribute unset.
fn attrs_allow_line_staging(out: &[u8]) -> bool {
    out.split(|&b| b == 0)
        .collect::<Vec<_>>()
        .chunks(3)
        .filter(|c| c.len() == 3)
        .all(|c| c[2] == b"unspecified" || c[2] == b"unset")
}

/// The file mode git has for `path`: from the index, else from HEAD. `None` if in neither.
async fn entry_mode(
    git: &GitBinary,
    root: &Path,
    path: &str,
    has_head: bool,
) -> Result<Option<String>, GitError> {
    let index = GitCommand::new(["ls-files", "-s", "-z", "--", path], Access::ReadOnly)
        .literal_pathspecs()
        .cwd(root)
        .run(git)
        .await?;
    if let Some(mode) = first_word(&index.stdout) {
        return Ok(Some(mode));
    }
    if !has_head {
        return Ok(None);
    }
    let head = GitCommand::new(["ls-tree", "-z", "HEAD", "--", path], Access::ReadOnly)
        .literal_pathspecs()
        .cwd(root)
        .run(git)
        .await?;
    Ok(first_word(&head.stdout))
}

/// The mode field that starts `ls-files -s` and `ls-tree` lines.
fn first_word(out: &[u8]) -> Option<String> {
    let word = out.split(|&b| b == b' ').next()?;
    let word = std::str::from_utf8(word).ok()?;
    (word.len() == 6 && word.bytes().all(|b| b.is_ascii_digit())).then(|| word.to_owned())
}

/// Mode for a file new to the index, as `git add` would pick it.
fn new_file_mode(path: &Path) -> &'static str {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if std::fs::metadata(path).is_ok_and(|m| m.permissions().mode() & 0o111 != 0) {
            return "100755";
        }
    }
    #[cfg(not(unix))]
    let _ = path;
    "100644"
}

/// HEAD's version of `path`, byte for byte.
async fn head_blob(git: &GitBinary, root: &Path, path: &str) -> Result<Vec<u8>, GitError> {
    let spec = format!("HEAD:{path}");
    let out = GitCommand::new(["cat-file", "blob", &spec], Access::ReadOnly)
        .cwd(root)
        .run(git)
        .await?;
    Ok(out.stdout)
}

/// Stores `content` as `path`'s staged version. `hash-object --path` applies the path's
/// attributes (line-ending conversion), the same as `git add` would.
async fn write_index_entry(
    git: &GitBinary,
    root: &Path,
    path: &str,
    mode: &str,
    content: Vec<u8>,
) -> Result<(), GitError> {
    let path_arg = format!("--path={path}");
    let out = GitCommand::new(
        ["hash-object", "-w", &path_arg, "--stdin"],
        Access::Mutating,
    )
    .stdin(content)
    .cwd(root)
    .run(git)
    .await?;
    let sha = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if !crate::git::log::is_commit_hash(&sha) {
        return Err(GitError::Parse(format!(
            "unexpected hash-object output: {sha}"
        )));
    }
    GitCommand::new(
        ["update-index", "--add", "--cacheinfo", mode, &sha, path],
        Access::Mutating,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(())
}

/// Drops `path` from the index, leaving the working tree alone.
async fn remove_from_index(git: &GitBinary, root: &Path, path: &str) -> Result<(), GitError> {
    GitCommand::new(
        ["update-index", "--force-remove", "--", path],
        Access::Mutating,
    )
    .cwd(root)
    .run(git)
    .await?;
    Ok(())
}

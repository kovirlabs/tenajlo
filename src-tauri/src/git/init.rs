//! Creating a new repository (spec §8.1: repository list → "Create new…").

use std::path::{Path, PathBuf};

use super::clone::valid_folder_name;
use super::error::GitError;
use super::exec::{Access, GitBinary, GitCommand};

/// The first branch of a new repository.
pub const INITIAL_BRANCH: &str = "main";

/// Why a repository couldn't be created.
#[derive(Debug, thiserror::Error)]
pub enum InitError {
    #[error("invalid repository name")]
    InvalidName,
    #[error("invalid location: {0}")]
    InvalidLocation(&'static str),
    #[error("{} already contains files", .0.display())]
    NotEmpty(PathBuf),
    #[error("could not create {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error(transparent)]
    Git(#[from] GitError),
}

/// Creates `parent/name` (or uses it if it's an empty folder) and runs `git init` there on
/// branch `main`. With `readme`, adds an uncommitted `README.md` for the first commit.
pub async fn init(
    git: &GitBinary,
    parent: &Path,
    name: &str,
    readme: bool,
) -> Result<PathBuf, InitError> {
    let name = name.trim();
    if !valid_folder_name(name) {
        return Err(InitError::InvalidName);
    }
    if !parent.is_absolute() {
        return Err(InitError::InvalidLocation("not an absolute path"));
    }
    let dest = parent.join(name);
    let io = |path: &Path| {
        let path = path.to_owned();
        move |source| InitError::Io { path, source }
    };
    match std::fs::read_dir(&dest) {
        Ok(mut entries) => {
            if entries.next().is_some() {
                return Err(InitError::NotEmpty(dest));
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            std::fs::create_dir_all(&dest).map_err(io(&dest))?;
        }
        Err(e) => return Err(io(&dest)(e)),
    }

    let dest_arg = dest
        .to_str()
        .ok_or(InitError::InvalidLocation("not valid Unicode"))?;
    GitCommand::new(
        ["init", "-q", "-b", INITIAL_BRANCH, "--", dest_arg],
        Access::Mutating,
    )
    .cwd(parent)
    .run(git)
    .await?;
    if readme {
        let path = dest.join("README.md");
        std::fs::write(&path, format!("# {name}\n")).map_err(io(&path))?;
    }
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::status::status;
    use crate::git::test_support::git;

    #[tokio::test]
    async fn creates_a_repository_on_main_with_a_readme() {
        let dir = tempfile::tempdir().unwrap();
        let git = git();
        let parent = dir.path().join("New Projects");
        let repo = init(&git, &parent, " Pump Station ", true).await.unwrap();
        assert_eq!(repo, parent.join("Pump Station"));
        let st = status(&git, &repo).await.unwrap();
        assert_eq!(st.branch.name.as_deref(), Some(INITIAL_BRANCH));
        assert_eq!(st.files.len(), 1, "README waiting for the first commit");
        assert_eq!(
            std::fs::read_to_string(repo.join("README.md")).unwrap(),
            "# Pump Station\n"
        );
    }

    #[tokio::test]
    async fn uses_an_empty_folder_but_refuses_one_with_files() {
        let dir = tempfile::tempdir().unwrap();
        let git = git();
        std::fs::create_dir(dir.path().join("empty")).unwrap();
        init(&git, dir.path(), "empty", false).await.unwrap();
        assert!(dir.path().join("empty/.git").is_dir());

        std::fs::create_dir(dir.path().join("full")).unwrap();
        std::fs::write(dir.path().join("full/notes.txt"), "keep me").unwrap();
        assert!(matches!(
            init(&git, dir.path(), "full", true).await,
            Err(InitError::NotEmpty(_))
        ));
        assert!(!dir.path().join("full/.git").exists());
        assert!(matches!(
            init(&git, dir.path(), "a/b", false).await,
            Err(InitError::InvalidName)
        ));
        assert!(matches!(
            init(&git, Path::new("relative"), "x", false).await,
            Err(InitError::InvalidLocation(_))
        ));
    }
}

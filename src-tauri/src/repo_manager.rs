//! Owns the repository list and per-repository state.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use uuid::Uuid;

use crate::git::error::GitError;
use crate::git::exec::GitBinary;
use crate::git::repo_root;
use crate::store::repositories::{RepositoriesFile, RepositoryEntry, FILE_NAME};
use crate::store::{self, StoreError};

/// A repository as shown in the UI.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    pub id: String,
    /// Alias, or the folder name.
    pub name: String,
    pub path: String,
    /// The folder no longer exists (moved or deleted).
    pub missing: bool,
}

/// The repository list plus current selection.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryList {
    /// Most recently opened first.
    pub repositories: Vec<Repository>,
    pub selected_id: Option<String>,
}

/// Errors from repository-list operations.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("unknown repository id")]
    UnknownRepository,
    #[error(transparent)]
    Git(#[from] GitError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Repository list and selection, persisted to `repositories.json`.
pub struct RepoManager {
    path: PathBuf,
    file: Mutex<RepositoriesFile>,
    /// Set when the file on disk couldn't be loaded safely; we then never overwrite it.
    read_only: bool,
}

impl RepoManager {
    /// Loads the repository list from `data_dir`. Never fails: a file that can't be
    /// loaded (e.g. written by a newer Anvil) yields an empty, read-only list.
    pub fn load(data_dir: &Path) -> Self {
        let path = data_dir.join(FILE_NAME);
        let (file, read_only) = match store::load::<RepositoriesFile>(&path) {
            Ok(f) => (f, false),
            Err(e) => {
                tracing::error!(error = %e, "could not load repository list; changes won't be saved");
                (RepositoriesFile::default(), true)
            }
        };
        Self {
            path,
            file: Mutex::new(file),
            read_only,
        }
    }

    /// Current list, most recently opened first.
    pub fn list(&self) -> RepositoryList {
        let file = self.lock();
        let mut entries: Vec<&RepositoryEntry> = file.repositories.iter().collect();
        entries.sort_by_key(|e| std::cmp::Reverse(e.last_opened));
        RepositoryList {
            repositories: entries.into_iter().map(to_dto).collect(),
            selected_id: file.selected.map(|id| id.to_string()),
        }
    }

    /// Adds the repository containing `dir` and selects it. Re-adding selects the existing entry.
    pub async fn add(&self, git: &GitBinary, dir: &Path) -> Result<Repository, RepoError> {
        let root = repo_root::show_toplevel(git, dir).await?;
        let mut file = self.lock();
        let id = match file.repositories.iter_mut().find(|r| r.path == root) {
            Some(existing) => {
                existing.last_opened = now();
                existing.id
            }
            None => {
                let entry = RepositoryEntry {
                    id: Uuid::new_v4(),
                    path: root,
                    alias: None,
                    last_opened: now(),
                };
                let id = entry.id;
                file.repositories.push(entry);
                id
            }
        };
        file.selected = Some(id);
        self.save(&file)?;
        let entry = file
            .repositories
            .iter()
            .find(|r| r.id == id)
            .ok_or(RepoError::UnknownRepository)?;
        Ok(to_dto(entry))
    }

    /// Removes a repository from the list. Files on disk are untouched.
    pub fn remove(&self, id: Uuid) -> Result<(), RepoError> {
        let mut file = self.lock();
        let before = file.repositories.len();
        file.repositories.retain(|r| r.id != id);
        if file.repositories.len() == before {
            return Err(RepoError::UnknownRepository);
        }
        if file.selected == Some(id) {
            file.selected = None;
        }
        self.save(&file)?;
        Ok(())
    }

    /// Marks a repository as selected and recently opened.
    pub fn select(&self, id: Uuid) -> Result<Repository, RepoError> {
        let mut file = self.lock();
        let entry = file
            .repositories
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or(RepoError::UnknownRepository)?;
        entry.last_opened = now();
        let dto = to_dto(entry);
        file.selected = Some(id);
        self.save(&file)?;
        Ok(dto)
    }

    /// Working-tree root for `id`. All git operations on a repository start here.
    pub fn root(&self, id: Uuid) -> Result<PathBuf, RepoError> {
        let file = self.lock();
        file.repositories
            .iter()
            .find(|r| r.id == id)
            .map(|r| r.path.clone())
            .ok_or(RepoError::UnknownRepository)
    }

    fn save(&self, file: &RepositoriesFile) -> Result<(), StoreError> {
        if self.read_only {
            tracing::warn!("repository list is read-only this session; not saving");
            return Ok(());
        }
        store::save(&self.path, file)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, RepositoriesFile> {
        // A poisoned lock only means another thread panicked mid-update; the data is still usable.
        self.file.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn to_dto(entry: &RepositoryEntry) -> Repository {
    let folder = entry
        .path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned());
    Repository {
        id: entry.id.to_string(),
        name: entry
            .alias
            .clone()
            .or(folder)
            .unwrap_or_else(|| entry.path.display().to_string()),
        path: entry.path.display().to_string(),
        missing: !entry.path.is_dir(),
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::binary::resolve;
    use crate::git::test_support::init_repo;

    #[tokio::test]
    async fn add_select_remove_persist() {
        let data = tempfile::tempdir().unwrap();
        let (_tmp, repo) = init_repo().await;
        let git = resolve(None, None).unwrap();

        let mgr = RepoManager::load(data.path());
        let added = mgr.add(&git, &repo.join(".")).await.unwrap();
        assert_eq!(added.name, "repo");
        assert!(!added.missing);
        // Re-adding (from a subdirectory) doesn't duplicate.
        std::fs::create_dir(repo.join("sub")).unwrap();
        let again = mgr.add(&git, &repo.join("sub")).await.unwrap();
        assert_eq!(again.id, added.id);

        let reloaded = RepoManager::load(data.path());
        let list = reloaded.list();
        assert_eq!(list.repositories.len(), 1);
        assert_eq!(list.selected_id.as_deref(), Some(added.id.as_str()));

        let id: Uuid = added.id.parse().unwrap();
        reloaded.remove(id).unwrap();
        assert!(RepoManager::load(data.path())
            .list()
            .repositories
            .is_empty());
        assert!(matches!(
            reloaded.root(id),
            Err(RepoError::UnknownRepository)
        ));
    }

    #[tokio::test]
    async fn rejects_plain_folder() {
        let data = tempfile::tempdir().unwrap();
        let plain = tempfile::tempdir().unwrap();
        let mgr = RepoManager::load(data.path());
        assert!(matches!(
            mgr.add(&resolve(None, None).unwrap(), plain.path()).await,
            Err(RepoError::Git(GitError::Failed { .. }))
        ));
        assert!(mgr.list().repositories.is_empty());
    }

    #[test]
    fn newer_file_is_never_overwritten() {
        let data = tempfile::tempdir().unwrap();
        let path = data.path().join(FILE_NAME);
        std::fs::write(&path, r#"{"schemaVersion":99}"#).unwrap();
        let mgr = RepoManager::load(data.path());
        assert!(matches!(
            mgr.remove(Uuid::new_v4()),
            Err(RepoError::UnknownRepository)
        ));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"{"schemaVersion":99}"#
        );
    }
}

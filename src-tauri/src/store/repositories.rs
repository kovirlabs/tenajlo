//! `repositories.json`: the user's repository list.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Versioned;

/// File name inside the app data dir.
pub const FILE_NAME: &str = "repositories.json";

/// One repository the user has added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryEntry {
    pub id: Uuid,
    /// Canonical working-tree root.
    pub path: PathBuf,
    pub alias: Option<String>,
    /// Unix seconds; used to order "recent" repositories.
    pub last_opened: u64,
}

/// Contents of `repositories.json`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoriesFile {
    pub repositories: Vec<RepositoryEntry>,
    pub selected: Option<Uuid>,
}

impl Versioned for RepositoriesFile {
    const VERSION: u64 = 1;
}

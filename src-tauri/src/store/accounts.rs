//! `accounts.json`: Forgejo account metadata (spec §6.1). Never holds secrets.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::Versioned;

/// File name inside the app data dir.
pub const FILE_NAME: &str = "accounts.json";

/// Kind of account. Only Forgejo for v1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "lowercase")]
pub enum AccountKind {
    Forgejo,
}

/// One signed-in account. The PAT is in the keychain under [`AccountEntry::keychain_key`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountEntry {
    pub id: Uuid,
    pub kind: AccountKind,
    /// Normalized, e.g. `https://tmc-git01.tmus.local` (no trailing slash).
    pub base_url: String,
    pub login: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    pub token_scopes: Option<Vec<String>>,
    /// SSH host override, e.g. `tmc-git01.tmus.local:2222` (M5).
    pub ssh_host: Option<String>,
}

impl AccountEntry {
    /// Keychain user for this account's PAT: `<baseUrl>|<login>`.
    pub fn keychain_key(&self) -> String {
        format!("{}|{}", self.base_url, self.login)
    }
}

/// Contents of `accounts.json`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountsFile {
    pub accounts: Vec<AccountEntry>,
}

impl Versioned for AccountsFile {
    const VERSION: u64 = 1;
}

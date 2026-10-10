//! Signed-in Forgejo accounts: metadata in `accounts.json`, PATs in the keychain (spec §6.1).
//!
//! One account per server. Signing in again to the same server replaces its account.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use reqwest::Url;
use serde::Serialize;
use uuid::Uuid;

use super::secrets::{Secret, SecretError, SecretStore};
use crate::forgejo::ForgejoUser;
use crate::store::accounts::{AccountEntry, AccountKind, AccountsFile, FILE_NAME};
use crate::store::{self, StoreError};

/// Account metadata for the frontend. Never includes the token (CLAUDE.md rule 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub id: String,
    pub kind: AccountKind,
    pub base_url: String,
    pub login: String,
    pub display_name: String,
    pub avatar_url: Option<String>,
    /// The server stopped accepting the saved token; show "Sign in again".
    pub needs_sign_in: bool,
}

/// Errors changing accounts.
#[derive(Debug, thiserror::Error)]
pub enum AccountError {
    #[error("unknown account id")]
    UnknownAccount,
    /// The account's token was rejected or is missing from the keychain.
    #[error("sign-in required for {}", .0.base_url)]
    SignInRequired(Box<AccountEntry>),
    #[error("accounts.json can't be changed this session")]
    ReadOnly,
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Owns `accounts.json` and the matching keychain entries.
pub struct AccountManager {
    path: PathBuf,
    file: Mutex<AccountsFile>,
    /// Set when the file on disk couldn't be loaded safely; we then never overwrite it.
    read_only: bool,
    secrets: Arc<dyn SecretStore>,
}

impl AccountManager {
    /// Loads accounts from `data_dir`. Never fails: an unreadable file yields no accounts
    /// and a read-only manager.
    pub fn load(data_dir: &Path, secrets: Arc<dyn SecretStore>) -> Self {
        let path = data_dir.join(FILE_NAME);
        let (file, read_only) = match store::load::<AccountsFile>(&path) {
            Ok(f) => (f, false),
            Err(e) => {
                tracing::error!(error = %e, "could not load accounts; changes won't be saved");
                (AccountsFile::default(), true)
            }
        };
        Self {
            path,
            file: Mutex::new(file),
            read_only,
            secrets,
        }
    }

    /// All accounts, ordered by server then login.
    pub fn list(&self) -> Vec<Account> {
        let mut out: Vec<Account> = self.data().accounts.iter().map(to_dto).collect();
        out.sort_by(|a, b| (&a.base_url, &a.login).cmp(&(&b.base_url, &b.login)));
        out
    }

    /// Saves a verified sign-in: the token to the keychain, then the metadata. Replaces any
    /// existing account for `base_url` (and deletes its old token if the login changed).
    pub async fn sign_in(
        &self,
        base_url: &str,
        user: ForgejoUser,
        token: Secret,
    ) -> Result<Account, AccountError> {
        if self.read_only {
            return Err(AccountError::ReadOnly);
        }
        let previous = self
            .data()
            .accounts
            .iter()
            .find(|a| a.base_url == base_url)
            .cloned();
        let display_name = if user.full_name.trim().is_empty() {
            user.login.clone()
        } else {
            user.full_name.trim().to_owned()
        };
        let entry = AccountEntry {
            id: previous.as_ref().map_or_else(Uuid::new_v4, |p| p.id),
            kind: AccountKind::Forgejo,
            base_url: base_url.to_owned(),
            login: user.login,
            display_name,
            avatar_url: user.avatar_url.filter(|u| !u.is_empty()),
            token_scopes: None,
            ssh_host: previous.as_ref().and_then(|p| p.ssh_host.clone()),
            needs_sign_in: false,
        };

        let key = entry.keychain_key();
        self.blocking(move |s| s.set(&key, &token)).await?;

        let saved = self.write(|file| {
            file.accounts.retain(|a| a.base_url != base_url);
            file.accounts.push(entry.clone());
            Ok(true)
        });
        if let Err(e) = saved {
            if previous.as_ref().map(AccountEntry::keychain_key) != Some(entry.keychain_key()) {
                let key = entry.keychain_key();
                let _ = self.blocking(move |s| s.delete(&key)).await;
            }
            return Err(e);
        }

        if let Some(old) = previous.filter(|p| p.login != entry.login) {
            let key = old.keychain_key();
            if let Err(e) = self.blocking(move |s| s.delete(&key)).await {
                tracing::warn!(error = %e, "could not delete the replaced account's token");
            }
        }
        tracing::info!(server = %crate::redact::redact(&entry.base_url), login = %entry.login, "signed in");
        Ok(to_dto(&entry))
    }

    /// Signs out: deletes the token from the keychain, then the metadata.
    pub async fn sign_out(&self, id: Uuid) -> Result<(), AccountError> {
        if self.read_only {
            return Err(AccountError::ReadOnly);
        }
        let entry = self
            .data()
            .accounts
            .iter()
            .find(|a| a.id == id)
            .cloned()
            .ok_or(AccountError::UnknownAccount)?;
        let key = entry.keychain_key();
        self.blocking(move |s| s.delete(&key)).await?;

        self.write(|file| {
            file.accounts.retain(|a| a.id != id);
            Ok(true)
        })?;
        tracing::info!(server = %crate::redact::redact(&entry.base_url), login = %entry.login, "signed out");
        Ok(())
    }

    /// The stored account with `id`.
    pub fn entry(&self, id: Uuid) -> Option<AccountEntry> {
        self.data().accounts.iter().find(|a| a.id == id).cloned()
    }

    /// The account whose server hosts `remote_url`, if any (spec §6.2 step 1).
    pub fn for_remote(&self, remote_url: &str) -> Option<AccountEntry> {
        self.data()
            .accounts
            .iter()
            .find(|a| hosts_remote(&a.base_url, remote_url))
            .cloned()
    }

    /// The account's token from the keychain; `None` if the entry is gone.
    pub async fn token(&self, account: &AccountEntry) -> Result<Option<Secret>, SecretError> {
        let key = account.keychain_key();
        self.blocking(move |s| s.get(&key)).await
    }

    /// The account's token, if it can still be used. `SignInRequired` when the server
    /// rejected it earlier or it's gone from the keychain (which marks the account).
    pub async fn usable_token(&self, account: &AccountEntry) -> Result<Secret, AccountError> {
        if account.needs_sign_in {
            return Err(AccountError::SignInRequired(Box::new(account.clone())));
        }
        self.token(account)
            .await?
            .ok_or_else(|| self.token_rejected(account))
    }

    /// Records that the server rejected the account's token (or it's gone) and returns the
    /// `SignInRequired` error to report.
    pub fn token_rejected(&self, account: &AccountEntry) -> AccountError {
        if let Err(e) = self.mark_needs_sign_in(account.id) {
            tracing::warn!(error = %e, "could not record that sign-in is required");
        }
        AccountError::SignInRequired(Box::new(account.clone()))
    }

    /// Records that the server rejected the account's token. The token is kept until the
    /// user signs in again or signs out.
    pub fn mark_needs_sign_in(&self, id: Uuid) -> Result<(), AccountError> {
        self.write(|file| {
            let entry = file
                .accounts
                .iter_mut()
                .find(|a| a.id == id)
                .ok_or(AccountError::UnknownAccount)?;
            if entry.needs_sign_in {
                return Ok(false);
            }
            entry.needs_sign_in = true;
            tracing::info!(server = %crate::redact::redact(&entry.base_url), "token rejected; sign-in required");
            Ok(true)
        })
    }

    /// Runs a keychain call off the async runtime (the OS store may block or show UI).
    async fn blocking<T: Send + 'static>(
        &self,
        f: impl FnOnce(&dyn SecretStore) -> Result<T, SecretError> + Send + 'static,
    ) -> Result<T, SecretError> {
        let secrets = self.secrets.clone();
        tokio::task::spawn_blocking(move || f(secrets.as_ref()))
            .await
            .map_err(|e| SecretError(e.to_string()))?
    }

    /// Applies `change` to a copy of the accounts, saves it, then publishes it, all under one
    /// lock so concurrent writers can't drop each other's updates. `change` returns false
    /// when it changed nothing, which skips the save. A read-only manager updates memory only.
    fn write(
        &self,
        change: impl FnOnce(&mut AccountsFile) -> Result<bool, AccountError>,
    ) -> Result<(), AccountError> {
        let mut data = self.data();
        let mut next = data.clone();
        if !change(&mut next)? {
            return Ok(());
        }
        if !self.read_only {
            store::save(&self.path, &next)?;
        }
        *data = next;
        Ok(())
    }

    fn data(&self) -> std::sync::MutexGuard<'_, AccountsFile> {
        self.file.lock().unwrap_or_else(|p| p.into_inner())
    }
}

fn to_dto(e: &AccountEntry) -> Account {
    Account {
        id: e.id.to_string(),
        kind: e.kind,
        base_url: e.base_url.clone(),
        login: e.login.clone(),
        display_name: e.display_name.clone(),
        avatar_url: e.avatar_url.clone(),
        needs_sign_in: e.needs_sign_in,
    }
}

/// True if `remote_url` is on the server at `base_url`: same scheme, host and port, and
/// under the server's sub-path. SSH and scp-style URLs never match (M5).
pub fn hosts_remote(base_url: &str, remote_url: &str) -> bool {
    let (Ok(base), Ok(remote)) = (Url::parse(base_url), Url::parse(remote_url)) else {
        return false;
    };
    let prefix = base.path().trim_end_matches('/');
    base.scheme() == remote.scheme()
        && base.host_str().is_some()
        && base.host_str() == remote.host_str()
        && base.port_or_known_default() == remote.port_or_known_default()
        && (prefix.is_empty() || remote.path().starts_with(&format!("{prefix}/")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::secrets::MemoryStore;
    use std::sync::atomic::Ordering;

    fn user(login: &str) -> ForgejoUser {
        ForgejoUser {
            login: login.into(),
            full_name: String::new(),
            avatar_url: Some(String::new()),
            email: String::new(),
        }
    }

    fn setup() -> (tempfile::TempDir, Arc<MemoryStore>, AccountManager) {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemoryStore::default());
        let m = AccountManager::load(dir.path(), secrets.clone());
        (dir, secrets, m)
    }

    const BASE: &str = "https://git.example.com";

    #[tokio::test]
    async fn sign_in_stores_token_in_keychain_only() {
        let (dir, secrets, m) = setup();
        let acct = m
            .sign_in(BASE, user("evan"), Secret::new("pat-123".into()))
            .await
            .unwrap();
        assert_eq!(acct.display_name, "evan", "falls back to the login");
        assert_eq!(acct.avatar_url, None, "empty avatar dropped");
        assert_eq!(
            secrets
                .get(&format!("{BASE}|evan"))
                .unwrap()
                .unwrap()
                .expose(),
            "pat-123"
        );
        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!raw.contains("pat-123"), "{raw}");
        assert!(raw.contains("\"schemaVersion\": 1"));

        let reloaded = AccountManager::load(dir.path(), secrets);
        assert_eq!(reloaded.list(), vec![acct]);
    }

    #[tokio::test]
    async fn re_sign_in_replaces_the_servers_account() {
        let (_dir, secrets, m) = setup();
        let first = m
            .sign_in(BASE, user("evan"), Secret::new("a".into()))
            .await
            .unwrap();
        let second = m
            .sign_in(BASE, user("evan2"), Secret::new("b".into()))
            .await
            .unwrap();
        assert_eq!(first.id, second.id, "same server keeps its id");
        assert_eq!(m.list(), vec![second]);
        assert_eq!(secrets.get(&format!("{BASE}|evan")).unwrap(), None);

        m.sign_in("https://other", user("evan"), Secret::new("c".into()))
            .await
            .unwrap();
        assert_eq!(m.list().len(), 2);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_writes_keep_every_update() {
        let (dir, secrets, m) = setup();
        let m = Arc::new(m);
        let first = m
            .sign_in(BASE, user("evan"), Secret::new("a".into()))
            .await
            .unwrap();
        let first_id: Uuid = first.id.parse().unwrap();

        let mut tasks = Vec::new();
        for i in 0..32 {
            let m = m.clone();
            tasks.push(tokio::spawn(async move {
                let server = format!("https://git{i}.example.com");
                m.sign_in(&server, user("evan"), Secret::new("t".into()))
                    .await
                    .unwrap();
            }));
        }
        let marker = m.clone();
        tasks.push(tokio::spawn(async move {
            marker.mark_needs_sign_in(first_id).unwrap();
        }));
        for t in tasks {
            t.await.unwrap();
        }

        for list in [m.list(), AccountManager::load(dir.path(), secrets).list()] {
            assert_eq!(list.len(), 33);
            let first = list.iter().find(|a| a.base_url == BASE).unwrap();
            assert!(first.needs_sign_in, "flag survived concurrent sign-ins");
        }
    }

    #[tokio::test]
    async fn sign_out_removes_token_and_metadata() {
        let (_dir, secrets, m) = setup();
        let acct = m
            .sign_in(BASE, user("evan"), Secret::new("a".into()))
            .await
            .unwrap();
        m.sign_out(acct.id.parse().unwrap()).await.unwrap();
        assert!(m.list().is_empty());
        assert_eq!(secrets.get(&format!("{BASE}|evan")).unwrap(), None);
        assert!(matches!(
            m.sign_out(Uuid::new_v4()).await,
            Err(AccountError::UnknownAccount)
        ));
    }

    #[tokio::test]
    async fn keychain_failure_saves_nothing() {
        let (dir, secrets, m) = setup();
        secrets.fail.store(true, Ordering::SeqCst);
        assert!(matches!(
            m.sign_in(BASE, user("evan"), Secret::new("a".into())).await,
            Err(AccountError::Secret(_))
        ));
        assert!(m.list().is_empty());
        assert!(!dir.path().join(FILE_NAME).exists());
    }

    #[test]
    fn remote_matching() {
        let m = |b, r| hosts_remote(b, r);
        assert!(m(BASE, "https://GIT.EXAMPLE.COM/team/plc.git"));
        assert!(m(BASE, "https://evan@git.example.com:443/team/plc"));
        assert!(!m(BASE, "https://git.example.com:3000/team/plc.git"));
        assert!(!m(BASE, "http://git.example.com/team/plc.git"));
        assert!(!m(BASE, "https://evil.example.com/team/plc.git"));
        assert!(!m(BASE, "git@git.example.com:team/plc.git"));
        assert!(!m(BASE, "ssh://git@git.example.com/team/plc.git"));
        assert!(m("https://h/forgejo", "https://h/forgejo/team/x.git"));
        assert!(!m("https://h/forgejo", "https://h/forgejoish/team/x.git"));
        assert!(!m("https://h/forgejo", "https://h/team/x.git"));
    }

    #[tokio::test]
    async fn needs_sign_in_persists_and_clears_on_sign_in() {
        let (dir, secrets, m) = setup();
        let acct = m
            .sign_in(BASE, user("evan"), Secret::new("a".into()))
            .await
            .unwrap();
        let entry = m.for_remote(&format!("{BASE}/team/plc.git")).unwrap();
        assert_eq!(m.token(&entry).await.unwrap().unwrap().expose(), "a");

        m.mark_needs_sign_in(entry.id).unwrap();
        let reloaded = AccountManager::load(dir.path(), secrets.clone());
        assert!(reloaded.list()[0].needs_sign_in);
        assert_eq!(
            reloaded.token(&entry).await.unwrap().unwrap().expose(),
            "a",
            "token is kept"
        );

        let again = reloaded
            .sign_in(BASE, user("evan"), Secret::new("b".into()))
            .await
            .unwrap();
        assert_eq!(again.id, acct.id);
        assert!(!again.needs_sign_in);
    }

    #[tokio::test]
    async fn newer_file_is_read_only() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"schemaVersion":99,"accounts":[]}"#,
        )
        .unwrap();
        let m = AccountManager::load(dir.path(), Arc::new(MemoryStore::default()));
        assert!(matches!(
            m.sign_in(BASE, user("evan"), Secret::new("a".into())).await,
            Err(AccountError::ReadOnly)
        ));
    }
}

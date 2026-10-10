//! Passwords and SSH passphrases the user chose to remember (spec §6.2 step 3, §6.3).
//!
//! The secrets live in the OS keychain; `saved-secrets.json` only lists which entries exist,
//! so Settings can show and forget them (keychains can't be enumerated portably).
//!
//! - **HTTPS logins** for servers without a Tenajlo account. The trampoline offers them to git
//!   as an extra credential helper after the user's own helpers, and saves one only when the
//!   user ticked "Remember" and git reports (`store`) that it worked. A login git reports as
//!   rejected (`erase`) is forgotten. Git LFS asks the same helpers, so this covers it too.
//! - **SSH passphrases**, answered to OpenSSH's prompt without showing it. A saved passphrase
//!   that OpenSSH asks for again was wrong, and is forgotten.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use serde::Serialize;

use super::secrets::{Secret, SecretError, SecretStore};
use crate::store::saved_secrets::{SavedLogin, SavedPassphrase, SavedSecretsFile, FILE_NAME};
use crate::store::{self, StoreError};

/// A remembered secret, for Settings. Never includes the secret itself.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(tag = "type")]
pub enum SavedSecretInfo {
    /// A password for `username` on `host`.
    Login {
        id: String,
        host: String,
        username: String,
    },
    /// The passphrase of the SSH key at `key`.
    Passphrase { id: String, key: String },
}

/// Errors changing saved secrets.
#[derive(Debug, thiserror::Error)]
pub enum SavedSecretError {
    #[error("no saved secret with that id")]
    Unknown,
    #[error("saved-secrets.json can't be changed this session")]
    ReadOnly,
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// Owns `saved-secrets.json` and the matching keychain entries.
pub struct SavedSecrets {
    path: std::path::PathBuf,
    file: Mutex<SavedSecretsFile>,
    /// The file on disk couldn't be loaded safely; never overwrite it.
    read_only: bool,
    secrets: Arc<dyn SecretStore>,
}

impl SavedSecrets {
    /// Loads from `data_dir`. Never fails: an unreadable file yields nothing saved and a
    /// read-only manager.
    pub fn load(data_dir: &Path, secrets: Arc<dyn SecretStore>) -> Self {
        let path = data_dir.join(FILE_NAME);
        let (file, read_only) = match store::load::<SavedSecretsFile>(&path) {
            Ok(f) => (f, false),
            Err(e) => {
                tracing::error!(error = %e, "could not load saved secrets; changes won't be saved");
                (SavedSecretsFile::default(), true)
            }
        };
        Self {
            path,
            file: Mutex::new(file),
            read_only,
            secrets,
        }
    }

    /// Everything saved: logins by host then user, then passphrases by key.
    pub fn list(&self) -> Vec<SavedSecretInfo> {
        let data = self.data();
        let mut logins = data.logins.clone();
        logins.sort_by(|a, b| (&a.host, &a.username).cmp(&(&b.host, &b.username)));
        let mut keys = data.passphrases.clone();
        keys.sort_by(|a, b| a.key.cmp(&b.key));
        logins
            .into_iter()
            .map(|l| SavedSecretInfo::Login {
                id: l.keychain_key(),
                host: l.host,
                username: l.username,
            })
            .chain(keys.into_iter().map(|p| SavedSecretInfo::Passphrase {
                id: p.keychain_key(),
                key: p.key,
            }))
            .collect()
    }

    /// The saved login for `protocol://host`, limited to `username` when git names one.
    pub async fn login(
        &self,
        protocol: &str,
        host: &str,
        username: Option<&str>,
    ) -> Option<(String, Secret)> {
        let host = normalize_host(protocol, host);
        let entry = self
            .data()
            .logins
            .iter()
            .find(|l| {
                l.protocol == protocol && l.host == host && username.is_none_or(|u| u == l.username)
            })
            .cloned()?;
        let secret = self.get(entry.keychain_key()).await?;
        Some((entry.username, secret))
    }

    /// Remembers `password` for `username` on `protocol://host`, replacing any earlier one.
    pub async fn save_login(
        &self,
        protocol: &str,
        host: &str,
        username: &str,
        password: Secret,
    ) -> Result<(), SavedSecretError> {
        let entry = SavedLogin {
            protocol: protocol.to_owned(),
            host: normalize_host(protocol, host),
            username: username.to_owned(),
        };
        self.save(entry.keychain_key(), password, move |file| {
            if file.logins.contains(&entry) {
                return false;
            }
            file.logins.push(entry);
            true
        })
        .await?;
        tracing::info!("saved a login");
        Ok(())
    }

    /// Forgets the saved login git just reported as rejected, if `password` is the saved one.
    /// (A password from somewhere else failing says nothing about the saved one.)
    pub async fn forget_rejected_login(
        &self,
        protocol: &str,
        host: &str,
        username: &str,
        password: &str,
    ) {
        let entry = SavedLogin {
            protocol: protocol.to_owned(),
            host: normalize_host(protocol, host),
            username: username.to_owned(),
        };
        if !self.data().logins.contains(&entry) {
            return;
        }
        let saved = self.get(entry.keychain_key()).await;
        if saved.is_some_and(|s| s.expose() == password) {
            tracing::info!("forgetting a saved login the server rejected");
            let _ = self.forget(&entry.keychain_key()).await;
        }
    }

    /// The saved passphrase for the SSH key at `key`.
    pub async fn passphrase(&self, key: &str) -> Option<Secret> {
        let entry = SavedPassphrase {
            key: key.to_owned(),
        };
        if !self.data().passphrases.contains(&entry) {
            return None;
        }
        self.get(entry.keychain_key()).await
    }

    /// Remembers the passphrase for the SSH key at `key`.
    pub async fn save_passphrase(
        &self,
        key: &str,
        passphrase: Secret,
    ) -> Result<(), SavedSecretError> {
        let entry = SavedPassphrase {
            key: key.to_owned(),
        };
        self.save(entry.keychain_key(), passphrase, move |file| {
            if file.passphrases.contains(&entry) {
                return false;
            }
            file.passphrases.push(entry);
            true
        })
        .await?;
        tracing::info!("saved an SSH key passphrase");
        Ok(())
    }

    /// Forgets the passphrase for `key` (OpenSSH rejected it).
    pub async fn forget_passphrase(&self, key: &str) {
        let id = SavedPassphrase {
            key: key.to_owned(),
        }
        .keychain_key();
        let _ = self.forget(&id).await;
    }

    /// Forgets the saved secret `id` (from [`Self::list`]): keychain entry, then metadata.
    pub async fn forget(&self, id: &str) -> Result<(), SavedSecretError> {
        if self.read_only {
            return Err(SavedSecretError::ReadOnly);
        }
        let known = {
            let d = self.data();
            d.logins.iter().any(|l| l.keychain_key() == id)
                || d.passphrases.iter().any(|p| p.keychain_key() == id)
        };
        if !known {
            return Err(SavedSecretError::Unknown);
        }
        let key = id.to_owned();
        self.blocking(move |s| s.delete(&key)).await?;
        self.write(|file| {
            file.logins.retain(|l| l.keychain_key() != id);
            file.passphrases.retain(|p| p.keychain_key() != id);
            true
        })
    }

    /// Reads a keychain entry. A missing or unreadable entry counts as nothing saved; a
    /// missing one also drops its metadata so Settings doesn't list it.
    async fn get(&self, key: String) -> Option<Secret> {
        let k = key.clone();
        match self.blocking(move |s| s.get(&k)).await {
            Ok(Some(secret)) => Some(secret),
            Ok(None) => {
                let _ = self.write(|file| {
                    file.logins.retain(|l| l.keychain_key() != key);
                    file.passphrases.retain(|p| p.keychain_key() != key);
                    true
                });
                None
            }
            Err(e) => {
                tracing::warn!(error = %e, "could not read a saved secret");
                None
            }
        }
    }

    async fn save(
        &self,
        key: String,
        secret: Secret,
        add: impl FnOnce(&mut SavedSecretsFile) -> bool,
    ) -> Result<(), SavedSecretError> {
        if self.read_only {
            return Err(SavedSecretError::ReadOnly);
        }
        let k = key.clone();
        self.blocking(move |s| s.set(&k, &secret)).await?;
        if let Err(e) = self.write(add) {
            let _ = self.blocking(move |s| s.delete(&key)).await;
            return Err(e);
        }
        Ok(())
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

    /// Applies `change` to a copy, saves it, then publishes it, all under one lock. `change`
    /// returns false when it changed nothing. A read-only manager updates memory only.
    fn write(
        &self,
        change: impl FnOnce(&mut SavedSecretsFile) -> bool,
    ) -> Result<(), SavedSecretError> {
        let mut data = self.data();
        let mut next = data.clone();
        if !change(&mut next) || next == *data {
            return Ok(());
        }
        if !self.read_only {
            store::save(&self.path, &next)?;
        }
        *data = next;
        Ok(())
    }

    fn data(&self) -> MutexGuard<'_, SavedSecretsFile> {
        self.file.lock().unwrap_or_else(|p| p.into_inner())
    }
}

/// Lowercase host, without the protocol's default port: git sends `host:443` when the remote
/// URL spells the port out.
fn normalize_host(protocol: &str, host: &str) -> String {
    let default_port = if protocol == "https" { ":443" } else { ":80" };
    let host = host.to_ascii_lowercase();
    host.strip_suffix(default_port)
        .map_or_else(|| host.clone(), str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::secrets::MemoryStore;
    use std::sync::atomic::Ordering;

    fn setup() -> (tempfile::TempDir, Arc<MemoryStore>, SavedSecrets) {
        let dir = tempfile::tempdir().unwrap();
        let secrets = Arc::new(MemoryStore::default());
        let saved = SavedSecrets::load(dir.path(), secrets.clone());
        (dir, secrets, saved)
    }

    fn pw(s: &str) -> Secret {
        Secret::new(s.into())
    }

    #[tokio::test]
    async fn logins_round_trip_without_secrets_on_disk() {
        let (dir, secrets, saved) = setup();
        saved
            .save_login("https", "GIT.example.com:443", "evan", pw("hunter2"))
            .await
            .unwrap();
        let raw = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();
        assert!(!raw.contains("hunter2"), "{raw}");
        assert_eq!(
            secrets
                .get("login|https|git.example.com|evan")
                .unwrap()
                .unwrap()
                .expose(),
            "hunter2"
        );

        let (user, secret) = saved.login("https", "git.example.com", None).await.unwrap();
        assert_eq!((user.as_str(), secret.expose()), ("evan", "hunter2"));
        assert!(saved
            .login("https", "git.example.com", Some("other"))
            .await
            .is_none());
        assert!(saved.login("http", "git.example.com", None).await.is_none());
        assert!(saved
            .login("https", "git.example.com:8443", None)
            .await
            .is_none());

        // Loads back from disk.
        let reloaded = SavedSecrets::load(dir.path(), secrets);
        assert_eq!(
            reloaded.list(),
            vec![SavedSecretInfo::Login {
                id: "login|https|git.example.com|evan".into(),
                host: "git.example.com".into(),
                username: "evan".into(),
            }]
        );
    }

    #[tokio::test]
    async fn rejected_logins_are_forgotten_only_if_they_were_the_saved_one() {
        let (_dir, _secrets, saved) = setup();
        saved
            .save_login("https", "h", "evan", pw("old"))
            .await
            .unwrap();
        saved
            .forget_rejected_login("https", "h", "evan", "typed")
            .await;
        assert!(saved.login("https", "h", None).await.is_some());
        saved
            .forget_rejected_login("https", "h:443", "evan", "old")
            .await;
        assert!(saved.login("https", "h", None).await.is_none());
        assert_eq!(saved.list(), vec![]);
    }

    #[tokio::test]
    async fn passphrases_and_forget_by_id() {
        let (_dir, _secrets, saved) = setup();
        let key = "C:\\Users\\Evan G\\.ssh\\id_ed25519";
        saved.save_passphrase(key, pw("pp")).await.unwrap();
        assert_eq!(saved.passphrase(key).await.unwrap().expose(), "pp");
        assert!(saved.passphrase("/other").await.is_none());

        let listed = saved.list();
        let [SavedSecretInfo::Passphrase { id, .. }] = listed.as_slice() else {
            panic!("{:?}", saved.list());
        };
        saved.forget(id).await.unwrap();
        assert!(saved.passphrase(key).await.is_none());
        assert!(matches!(
            saved.forget(id).await,
            Err(SavedSecretError::Unknown)
        ));
    }

    #[tokio::test]
    async fn an_entry_gone_from_the_keychain_is_dropped() {
        let (_dir, secrets, saved) = setup();
        saved.save_passphrase("/k", pw("pp")).await.unwrap();
        secrets.delete("ssh-passphrase|/k").unwrap();
        assert!(saved.passphrase("/k").await.is_none());
        assert_eq!(saved.list(), vec![]);
    }

    #[tokio::test]
    async fn a_locked_keychain_saves_nothing() {
        let (_dir, secrets, saved) = setup();
        secrets.fail.store(true, Ordering::SeqCst);
        assert!(saved
            .save_login("https", "h", "evan", pw("x"))
            .await
            .is_err());
        secrets.fail.store(false, Ordering::SeqCst);
        assert_eq!(saved.list(), vec![]);
    }
}

//! OS keychain access (spec §6.1). Secrets live only here and in memory while in use.
//!
//! Entries use `service = "tenajlo"`, `user = "<baseUrl>|<login>"`.

use std::fmt;

/// Keychain service name for every Tenajlo entry.
pub const SERVICE: &str = "tenajlo";

/// A secret value (PAT, password). `Debug` never prints it; there is no `Serialize`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    /// Wraps a secret.
    pub fn new(value: String) -> Self {
        Self(value)
    }

    /// The raw value. Only for handing to git or an HTTP header, never for logging.
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// Keychain errors. Messages come from the OS store and never contain the secret.
#[derive(Debug, thiserror::Error)]
#[error("keychain error: {0}")]
pub struct SecretError(pub String);

/// Where secrets are kept. Blocking: call from `spawn_blocking`.
pub trait SecretStore: Send + Sync {
    /// The secret for `key`, or `None` if there is no entry.
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError>;
    /// Creates or replaces the entry for `key`.
    fn set(&self, key: &str, secret: &Secret) -> Result<(), SecretError>;
    /// Deletes the entry for `key`. Missing entries are not an error.
    fn delete(&self, key: &str) -> Result<(), SecretError>;
}

/// The OS keychain: Windows Credential Manager, macOS Keychain, or Secret Service.
pub struct KeyringStore;

impl KeyringStore {
    fn entry(key: &str) -> Result<keyring::Entry, SecretError> {
        keyring::Entry::new(SERVICE, key).map_err(|e| SecretError(e.to_string()))
    }
}

impl SecretStore for KeyringStore {
    fn get(&self, key: &str) -> Result<Option<Secret>, SecretError> {
        match Self::entry(key)?.get_password() {
            Ok(s) => Ok(Some(Secret::new(s))),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }

    fn set(&self, key: &str, secret: &Secret) -> Result<(), SecretError> {
        Self::entry(key)?
            .set_password(secret.expose())
            .map_err(|e| SecretError(e.to_string()))
    }

    fn delete(&self, key: &str) -> Result<(), SecretError> {
        match Self::entry(key)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(SecretError(e.to_string())),
        }
    }
}

/// In-memory store for tests.
#[cfg(test)]
pub use memory::MemoryStore;

#[cfg(test)]
mod memory {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, MutexGuard};

    use super::{Secret, SecretError, SecretStore};

    #[derive(Default)]
    pub struct MemoryStore {
        entries: Mutex<HashMap<String, Secret>>,
        /// When set, every call fails (simulates a locked or missing keychain).
        pub fail: AtomicBool,
    }

    impl MemoryStore {
        fn check(&self) -> Result<MutexGuard<'_, HashMap<String, Secret>>, SecretError> {
            if self.fail.load(Ordering::SeqCst) {
                return Err(SecretError("keychain locked".into()));
            }
            Ok(self.entries.lock().unwrap_or_else(|p| p.into_inner()))
        }
    }

    impl SecretStore for MemoryStore {
        fn get(&self, key: &str) -> Result<Option<Secret>, SecretError> {
            Ok(self.check()?.get(key).cloned())
        }

        fn set(&self, key: &str, secret: &Secret) -> Result<(), SecretError> {
            self.check()?.insert(key.to_owned(), secret.clone());
            Ok(())
        }

        fn delete(&self, key: &str) -> Result<(), SecretError> {
            self.check()?.remove(key);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_debug_is_redacted() {
        assert_eq!(
            format!("{:?}", Secret::new("hunter2".into())),
            "Secret(***)"
        );
    }

    /// Run manually: `cargo test -- --ignored keyring_round_trip`.
    #[test]
    #[ignore = "touches the real OS keychain"]
    fn keyring_round_trip() {
        let key = format!("selftest|{}", uuid::Uuid::new_v4());
        let store = KeyringStore;
        assert_eq!(store.get(&key).unwrap(), None);
        store.set(&key, &Secret::new("v1".into())).unwrap();
        store.set(&key, &Secret::new("v2".into())).unwrap();
        assert_eq!(store.get(&key).unwrap().unwrap().expose(), "v2");
        store.delete(&key).unwrap();
        store.delete(&key).unwrap();
        assert_eq!(store.get(&key).unwrap(), None);
    }

    #[test]
    fn memory_store_round_trip() {
        let s = MemoryStore::default();
        assert_eq!(s.get("k").unwrap(), None);
        s.set("k", &Secret::new("v".into())).unwrap();
        assert_eq!(s.get("k").unwrap().unwrap().expose(), "v");
        s.delete("k").unwrap();
        s.delete("k").unwrap();
        assert_eq!(s.get("k").unwrap(), None);
    }
}

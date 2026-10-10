//! `saved-secrets.json`: which passwords and SSH passphrases the user chose to remember
//! (spec §6.2, §6.3). Never holds secrets; each entry names a keychain entry.

use serde::{Deserialize, Serialize};

use super::Versioned;

/// File name inside the app data dir.
pub const FILE_NAME: &str = "saved-secrets.json";

/// An HTTPS login for a server without a Tenajlo account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedLogin {
    /// `https`, or `http` for a server on a plain-HTTP address.
    pub protocol: String,
    /// Lowercase `host` or `host:port`, without the protocol's default port.
    pub host: String,
    pub username: String,
}

impl SavedLogin {
    /// Keychain user for the password: `login|<protocol>|<host>|<username>`. The prefix keeps
    /// it apart from account tokens, which are keyed `<baseUrl>|<login>`.
    pub fn keychain_key(&self) -> String {
        format!("login|{}|{}|{}", self.protocol, self.host, self.username)
    }
}

/// The passphrase of one SSH private key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedPassphrase {
    /// The key file's path, exactly as OpenSSH names it in its prompt.
    pub key: String,
}

impl SavedPassphrase {
    /// Keychain user for the passphrase: `ssh-passphrase|<key path>`.
    pub fn keychain_key(&self) -> String {
        format!("ssh-passphrase|{}", self.key)
    }
}

/// Contents of `saved-secrets.json`.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedSecretsFile {
    #[serde(default)]
    pub logins: Vec<SavedLogin>,
    #[serde(default)]
    pub passphrases: Vec<SavedPassphrase>,
}

impl Versioned for SavedSecretsFile {
    const VERSION: u64 = 1;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keychain_keys_never_look_like_account_keys() {
        let login = SavedLogin {
            protocol: "https".into(),
            host: "git.example.com".into(),
            username: "a|b@c".into(),
        };
        assert_eq!(login.keychain_key(), "login|https|git.example.com|a|b@c");
        let pass = SavedPassphrase {
            key: "C:\\Users\\Evan G\\.ssh\\id_ed25519".into(),
        };
        assert_eq!(
            pass.keychain_key(),
            "ssh-passphrase|C:\\Users\\Evan G\\.ssh\\id_ed25519"
        );
        // Account keys start with the server's URL.
        assert!(!login.keychain_key().starts_with("http"));
        assert!(!pass.keychain_key().starts_with("http"));
    }
}

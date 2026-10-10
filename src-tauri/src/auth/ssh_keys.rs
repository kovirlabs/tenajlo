//! SSH keys in the user's `~/.ssh` (spec §6.3 key management): list the public keys and
//! generate an ed25519 key.
//!
//! Keys are generated in-process with `ssh-key`, so a passphrase never appears on a command
//! line. Tenajlo only creates `id_ed25519`, the name OpenSSH tries by default, and never
//! overwrites an existing key or edits `~/.ssh/config`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ssh_key::rand_core::OsRng;
use ssh_key::{Algorithm, HashAlg, LineEnding, PrivateKey, PublicKey};

use super::secrets::Secret;

/// The key Tenajlo generates. OpenSSH uses it without any configuration.
pub const DEFAULT_KEY: &str = "id_ed25519";
/// Public key files larger than this aren't keys.
const MAX_PUBLIC_KEY_BYTES: u64 = 16 * 1024;

/// A public key found in `~/.ssh`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalSshKey {
    /// File name of the public key, e.g. `id_ed25519.pub`.
    pub file_name: String,
    /// Algorithm as OpenSSH names it, e.g. `ssh-ed25519`.
    pub key_type: String,
    /// The key's comment (often `user@host`); may be empty.
    pub comment: String,
    /// `SHA256:…`, as `ssh-keygen -l` and Forgejo show it.
    pub fingerprint: String,
}

/// Errors reading or creating SSH keys. Never contains a passphrase.
#[derive(Debug, thiserror::Error)]
pub enum SshKeyError {
    #[error("{0} already exists")]
    Exists(String),
    #[error("no public key named {0}")]
    NotFound(String),
    #[error("{0} isn't an SSH public key")]
    Invalid(String),
    #[error("couldn't create the key: {0}")]
    Generate(String),
    #[error("{action} {path}: {source}")]
    Io {
        action: &'static str,
        path: String,
        source: std::io::Error,
    },
}

fn io_error(action: &'static str, path: &Path) -> impl FnOnce(std::io::Error) -> SshKeyError {
    let path = path.display().to_string();
    move |source| SshKeyError::Io {
        action,
        path,
        source,
    }
}

/// `~/.ssh` under `home`.
pub fn ssh_dir(home: &Path) -> PathBuf {
    home.join(".ssh")
}

/// The public keys in `dir`, sorted by file name. A missing directory means no keys;
/// files that aren't public keys are skipped.
pub fn list(dir: &Path) -> Result<Vec<LocalSshKey>, SshKeyError> {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_error("reading", dir)(e)),
    };
    let mut keys: Vec<LocalSshKey> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            name.ends_with(".pub").then(|| read(dir, &name).ok())?
        })
        .collect();
    keys.sort_by(|a, b| a.file_name.cmp(&b.file_name));
    Ok(keys)
}

/// The public key `file_name` in `dir` and its one-line OpenSSH text. `file_name` must be a
/// plain `*.pub` name, not a path.
pub fn read_public(dir: &Path, file_name: &str) -> Result<(LocalSshKey, String), SshKeyError> {
    if !is_public_key_name(file_name) {
        return Err(SshKeyError::NotFound(file_name.to_owned()));
    }
    let path = dir.join(file_name);
    let meta = match fs::metadata(&path) {
        Ok(m) => m,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SshKeyError::NotFound(file_name.to_owned()))
        }
        Err(e) => return Err(io_error("reading", &path)(e)),
    };
    if !meta.is_file() || meta.len() > MAX_PUBLIC_KEY_BYTES {
        return Err(SshKeyError::Invalid(file_name.to_owned()));
    }
    let text = fs::read_to_string(&path).map_err(|_| SshKeyError::Invalid(file_name.to_owned()))?;
    let key = PublicKey::from_openssh(text.trim())
        .map_err(|_| SshKeyError::Invalid(file_name.to_owned()))?;
    let line = key
        .to_openssh()
        .map_err(|_| SshKeyError::Invalid(file_name.to_owned()))?;
    Ok((describe(file_name, &key), line))
}

fn read(dir: &Path, file_name: &str) -> Result<LocalSshKey, SshKeyError> {
    read_public(dir, file_name).map(|(k, _)| k)
}

/// A plain file name ending in `.pub`: no separators, no `..`, not hidden.
fn is_public_key_name(name: &str) -> bool {
    name.len() > ".pub".len()
        && name.ends_with(".pub")
        && !name.starts_with('.')
        && !name.contains(['/', '\\', ':', '\0'])
}

fn describe(file_name: &str, key: &PublicKey) -> LocalSshKey {
    LocalSshKey {
        file_name: file_name.to_owned(),
        key_type: key.algorithm().as_str().to_owned(),
        comment: key.comment().to_owned(),
        fingerprint: key.fingerprint(HashAlg::Sha256).to_string(),
    }
}

/// Creates `dir/id_ed25519` and `id_ed25519.pub`, encrypted with `passphrase` if given.
/// Fails with `Exists` rather than replace either file.
pub fn generate(
    dir: &Path,
    comment: &str,
    passphrase: Option<&Secret>,
) -> Result<LocalSshKey, SshKeyError> {
    let private_path = dir.join(DEFAULT_KEY);
    let public_name = format!("{DEFAULT_KEY}.pub");
    let public_path = dir.join(&public_name);
    for p in [&private_path, &public_path] {
        if p.symlink_metadata().is_ok() {
            return Err(SshKeyError::Exists(p.display().to_string()));
        }
    }

    let mut key = PrivateKey::random(&mut OsRng, Algorithm::Ed25519)
        .map_err(|e| SshKeyError::Generate(e.to_string()))?;
    key.set_comment(comment.trim());
    let public = key.public_key().clone();
    if let Some(pass) = passphrase.filter(|p| !p.expose().is_empty()) {
        key = key
            .encrypt(&mut OsRng, pass.expose())
            .map_err(|e| SshKeyError::Generate(e.to_string()))?;
    }
    let private_text = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| SshKeyError::Generate(e.to_string()))?;
    let public_text = public
        .to_openssh()
        .map_err(|e| SshKeyError::Generate(e.to_string()))?;

    create_ssh_dir(dir)?;
    write_new(&private_path, private_text.as_bytes(), 0o600)?;
    if let Err(e) = write_new(&public_path, format!("{public_text}\n").as_bytes(), 0o644) {
        // Don't leave half a key pair behind.
        let _ = fs::remove_file(&private_path);
        return Err(e);
    }
    tracing::info!(file = %DEFAULT_KEY, "generated an SSH key");
    Ok(describe(&public_name, &public))
}

/// Creates `~/.ssh` (owner-only on Unix) if it's missing.
fn create_ssh_dir(dir: &Path) -> Result<(), SshKeyError> {
    if dir.is_dir() {
        return Ok(());
    }
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut builder, 0o700);
    builder.create(dir).map_err(io_error("creating", dir))
}

/// Writes a file that must not exist yet. `mode` applies on Unix; on Windows the file inherits
/// the user profile's permissions, which OpenSSH accepts.
fn write_new(path: &Path, data: &[u8], mode: u32) -> Result<(), SshKeyError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, mode);
    #[cfg(not(unix))]
    let _ = mode;
    let mut file = options.open(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::AlreadyExists {
            SshKeyError::Exists(path.display().to_string())
        } else {
            io_error("creating", path)(e)
        }
    })?;
    file.write_all(data)
        .and_then(|()| file.sync_all())
        .map_err(io_error("writing", path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_lists_and_reads_back() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = ssh_dir(tmp.path());
        assert_eq!(list(&dir).unwrap(), vec![]);

        let made = generate(&dir, "me@laptop", None).unwrap();
        assert_eq!(made.file_name, "id_ed25519.pub");
        assert_eq!(made.key_type, "ssh-ed25519");
        assert_eq!(made.comment, "me@laptop");
        assert!(made.fingerprint.starts_with("SHA256:"));
        assert_eq!(list(&dir).unwrap(), vec![made.clone()]);

        let (read, line) = read_public(&dir, "id_ed25519.pub").unwrap();
        assert_eq!(read, made);
        assert!(line.starts_with("ssh-ed25519 AAAA") && line.ends_with(" me@laptop"));
        let private = fs::read_to_string(dir.join("id_ed25519")).unwrap();
        let key = PrivateKey::from_openssh(&private).unwrap();
        assert!(!key.is_encrypted());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = |p: &Path| fs::metadata(p).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(&dir), 0o700);
            assert_eq!(mode(&dir.join("id_ed25519")), 0o600);
        }
    }

    #[test]
    fn passphrase_encrypts_the_private_key() {
        let tmp = tempfile::tempdir().unwrap();
        let pass = Secret::new("correct horse".into());
        generate(tmp.path(), "", Some(&pass)).unwrap();
        let private = fs::read_to_string(tmp.path().join("id_ed25519")).unwrap();
        let key = PrivateKey::from_openssh(&private).unwrap();
        assert!(key.is_encrypted());
        assert!(key.decrypt("wrong").is_err());
        assert!(key.decrypt("correct horse").is_ok());
    }

    #[test]
    fn never_overwrites_a_key() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join("id_ed25519"), "existing").unwrap();
        assert!(matches!(
            generate(tmp.path(), "", None),
            Err(SshKeyError::Exists(_))
        ));
        assert_eq!(
            fs::read_to_string(tmp.path().join("id_ed25519")).unwrap(),
            "existing"
        );
        assert!(!tmp.path().join("id_ed25519.pub").exists());
    }

    #[test]
    fn skips_files_that_are_not_keys() {
        let tmp = tempfile::tempdir().unwrap();
        generate(tmp.path(), "", None).unwrap();
        fs::write(tmp.path().join("notes.pub"), "hello").unwrap();
        fs::write(tmp.path().join("known_hosts"), "x").unwrap();
        let names: Vec<_> = list(tmp.path())
            .unwrap()
            .into_iter()
            .map(|k| k.file_name)
            .collect();
        assert_eq!(names, ["id_ed25519.pub"]);
        assert!(matches!(
            read_public(tmp.path(), "notes.pub"),
            Err(SshKeyError::Invalid(_))
        ));
    }

    #[test]
    fn rejects_paths_as_names() {
        let tmp = tempfile::tempdir().unwrap();
        for name in [
            "../id_ed25519.pub",
            "a/b.pub",
            "a\\b.pub",
            ".pub",
            "id_ed25519",
            "C:x.pub",
        ] {
            assert!(
                matches!(read_public(tmp.path(), name), Err(SshKeyError::NotFound(_))),
                "{name}"
            );
        }
    }
}

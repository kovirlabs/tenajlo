//! Authentication: accounts, keychain, askpass trampoline and UI prompts (spec §6).

pub mod accounts;
pub mod broker;
pub mod credential_helper;
pub mod prompt;
pub mod remote_auth;
pub mod secrets;
pub mod ssh_keys;
pub mod trampoline;

use std::path::PathBuf;

/// The bundled `tenajlo-askpass` next to the app executable, if present and non-empty.
pub fn askpass_path() -> Option<PathBuf> {
    let name = if cfg!(windows) {
        "tenajlo-askpass.exe"
    } else {
        "tenajlo-askpass"
    };
    let path = std::env::current_exe().ok()?.parent()?.join(name);
    // build.rs may leave an empty placeholder when the sidecar wasn't built.
    let usable = path.metadata().is_ok_and(|m| m.is_file() && m.len() > 0);
    usable.then_some(path)
}

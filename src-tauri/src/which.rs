//! Looking up programs on `PATH`.

use std::path::PathBuf;

/// `dir/name` for each directory on `PATH`, in order. Relative entries are skipped: they
/// would resolve against whatever the current directory happens to be.
pub fn path_candidates(name: &str) -> Vec<PathBuf> {
    let Some(path_var) = std::env::var_os("PATH") else {
        return Vec::new();
    };
    std::env::split_paths(&path_var)
        .filter(|dir| dir.is_absolute())
        .map(|dir| dir.join(name))
        .collect()
}

/// The first `name` on `PATH` that is a file.
pub fn on_path(name: &str) -> Option<PathBuf> {
    path_candidates(name).into_iter().find(|p| p.is_file())
}

use std::path::PathBuf;

fn main() {
    ensure_askpass_placeholder();
    tauri_build::build();
}

/// Tauri requires every `externalBin` to exist whenever this build script runs, including
/// for `cargo test` / `cargo clippy`. `scripts/build-askpass.mjs` (run by `pnpm tauri dev`
/// and `pnpm tauri build`) puts the real binary there; this only fills the gap so plain
/// cargo commands work on a fresh checkout. The app checks for an empty file at startup.
fn ensure_askpass_placeholder() {
    let target = std::env::var("TARGET").unwrap_or_default();
    let ext = if target.contains("windows") {
        ".exe"
    } else {
        ""
    };
    let path = PathBuf::from("binaries").join(format!("tenajlo-askpass-{target}{ext}"));
    if !path.exists() {
        let _ = std::fs::create_dir_all("binaries");
        let _ = std::fs::write(&path, b"");
    }
    println!("cargo:rerun-if-changed={}", path.display());
}

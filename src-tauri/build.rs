use std::path::PathBuf;

fn main() {
    ensure_askpass_placeholder();
    ensure_mingit_placeholder();
    tauri_build::build();
}

/// Windows builds bundle `resources/mingit` (tauri.windows.conf.json), which must exist for
/// this build script. `scripts/fetch-mingit.mjs` (run by `pnpm tauri dev/build` on Windows)
/// fills it; an empty folder keeps plain cargo commands working, and debug builds then fall
/// back to git on PATH.
fn ensure_mingit_placeholder() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows") {
        let _ = std::fs::create_dir_all("resources/mingit");
    }
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

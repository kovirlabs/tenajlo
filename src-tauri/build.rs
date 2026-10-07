use std::path::PathBuf;

fn main() {
    ensure_askpass_placeholder();
    ensure_bundled_resource_placeholders();
    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("windows-msvc") {
        // tauri-build embeds the Common Controls v6 manifest into the app executable only, so
        // test executables fail to start (STATUS_ENTRYPOINT_NOT_FOUND: the dialog APIs need
        // it). Embed the same manifest through the linker instead, which covers every
        // executable this package builds, tests included.
        println!("cargo:rerun-if-changed=windows-app-manifest.xml");
        let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default())
            .join("windows-app-manifest.xml");
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
        let attributes = tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        if let Err(e) = tauri_build::try_build(attributes) {
            panic!("tauri-build failed: {e:#}");
        }
    } else {
        tauri_build::build();
    }
}

/// Release builds bundle `resources/THIRD_PARTY_LICENSES.html` (tauri.<platform>.conf.json)
/// and, on Windows, `resources/mingit`; both must exist for this build script. `pnpm tauri
/// build` generates them (scripts/third-party-licenses.mjs, scripts/fetch-mingit.mjs);
/// placeholders keep plain cargo commands and `tauri dev` working, and debug builds then fall
/// back to git on PATH.
fn ensure_bundled_resource_placeholders() {
    let licenses = std::path::Path::new("resources/THIRD_PARTY_LICENSES.html");
    if !licenses.exists() {
        let _ = std::fs::create_dir_all("resources");
        let _ = std::fs::write(licenses, b"");
    }
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

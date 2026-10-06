//! Anvil — desktop Git client for self-hosted Forgejo.

pub mod commands;
pub mod error;
pub mod git;
pub mod redact;
pub mod state;

use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

/// Registers every command with tauri-specta. Shared by the app and `export-bindings`.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![commands::app::check_git])
}

/// Builds and runs the Tauri application.
pub fn run() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("anvil_lib=info")),
        )
        .init();

    let builder = specta_builder();
    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let bundled_git_dir = app.path().resource_dir().ok().map(|d| d.join("mingit"));
            app.manage(state::AppState { bundled_git_dir });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Anvil");
}

#[cfg(test)]
mod tests {
    use specta_typescript::Typescript;

    /// Fails when `src/bindings.ts` is stale. Fix with `pnpm bindings`.
    #[test]
    fn bindings_are_up_to_date() {
        let dir = tempfile::tempdir().unwrap();
        let fresh = dir.path().join("bindings.ts");
        super::specta_builder()
            .export(Typescript::default(), &fresh)
            .unwrap();
        let committed = concat!(env!("CARGO_MANIFEST_DIR"), "/../src/bindings.ts");
        let committed = std::fs::read_to_string(committed).unwrap_or_default();
        assert_eq!(
            std::fs::read_to_string(fresh).unwrap(),
            committed,
            "run `pnpm bindings`"
        );
    }
}

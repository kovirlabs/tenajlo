//! Anvil — desktop Git client for self-hosted Forgejo.

pub mod commands;
pub mod error;
pub mod git;
pub mod os_trash;
pub mod redact;
pub mod repo_manager;
pub mod state;
pub mod store;
pub mod watcher;

use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

/// Registers every command with tauri-specta. Shared by the app and `export-bindings`.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app::check_git,
            commands::repos::list_repositories,
            commands::repos::add_local_repository,
            commands::repos::remove_repository,
            commands::repos::select_repository,
            commands::repo::get_status,
            commands::repo::get_working_diff,
            commands::repo::get_history,
            commands::repo::get_commit_files,
            commands::repo::get_commit_diff,
            commands::repo::get_branches,
            commands::repo::watch_repository,
            commands::changes::set_staged,
            commands::changes::commit_changes,
            commands::changes::get_identity,
            commands::changes::set_global_identity,
            commands::changes::discard_changes,
            commands::changes::ignore_file,
            commands::branches::preview_branch_name,
            commands::branches::create_branch,
            commands::branches::switch_branch,
            commands::branches::delete_branch,
            commands::branches::get_saved_changes,
            commands::branches::restore_saved_changes,
        ])
        .events(collect_events![watcher::RepoChanged])
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
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let bundled_git_dir = app.path().resource_dir().ok().map(|d| d.join("mingit"));
            let data_dir = app.path().app_data_dir()?;
            let repos = repo_manager::RepoManager::load(&data_dir);
            app.manage(state::AppState::new(bundled_git_dir, repos));
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

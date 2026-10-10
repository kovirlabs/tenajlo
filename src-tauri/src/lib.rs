//! Tenajlo — desktop Git client for self-hosted Forgejo.

pub mod auth;
pub mod commands;
pub mod editor;
pub mod error;
pub mod forgejo;
pub mod git;
pub mod logging;
pub mod operations;
pub mod os_trash;
pub mod redact;
pub mod repo_manager;
pub mod settings;
pub mod state;
pub mod store;
pub mod watcher;
pub mod which;

use tauri::Manager;
use tauri_specta::{collect_commands, collect_events, Builder};

/// Registers every command with tauri-specta. Shared by the app and `export-bindings`.
pub fn specta_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new()
        .commands(collect_commands![
            commands::app::check_git,
            commands::app::get_app_info,
            commands::app::open_logs_folder,
            commands::repos::list_repositories,
            commands::repos::add_local_repository,
            commands::repos::default_repository_folder,
            commands::repos::create_repository,
            commands::repos::remove_repository,
            commands::repos::select_repository,
            commands::repo::get_status,
            commands::repo::get_working_diff,
            commands::repo::get_history,
            commands::repo::get_commit_files,
            commands::repo::get_commit_diff,
            commands::repo::get_branches,
            commands::repo::watch_repository,
            commands::repo::get_lfs_status,
            commands::changes::set_staged,
            commands::changes::set_lines_staged,
            commands::changes::commit_changes,
            commands::changes::get_identity,
            commands::changes::set_global_identity,
            commands::changes::discard_changes,
            commands::changes::ignore_extension,
            commands::changes::ignore_file,
            commands::changes::undo_commit,
            commands::accounts::list_accounts,
            commands::accounts::check_server,
            commands::accounts::sign_in,
            commands::accounts::sign_out,
            commands::accounts::open_token_settings,
            commands::accounts::get_account_identity,
            commands::clone::list_forgejo_repositories,
            commands::clone::suggest_clone_path,
            commands::clone::choose_clone_folder,
            commands::clone::clone_repository,
            commands::merge::get_operation_state,
            commands::merge::mark_resolved,
            commands::merge::abort_operation,
            commands::merge::open_repo_file,
            commands::merge::reveal_repo_file,
            commands::settings::get_settings,
            commands::settings::save_settings,
            commands::settings::choose_folder,
            commands::settings::choose_file,
            commands::settings::get_global_identity,
            commands::settings::list_editors,
            commands::auth::answer_auth_prompt,
            commands::auth::cancel_operation,
            commands::sync::get_sync_state,
            commands::sync::sync,
            commands::branches::preview_branch_name,
            commands::branches::create_branch,
            commands::branches::switch_branch,
            commands::branches::delete_branch,
            commands::branches::get_saved_changes,
            commands::branches::restore_saved_changes,
        ])
        .events(collect_events![
            watcher::RepoChanged,
            auth::broker::AuthPromptRequested,
            commands::remote_op::GitProgress
        ])
}

/// Builds and runs the Tauri application.
pub fn run() {
    let builder = specta_builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Rust-side only (token settings link); the WebView gets no opener permissions.
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let bundled_git_dir = app.path().resource_dir().ok().map(|d| d.join("mingit"));
            let data_dir = app.path().app_data_dir()?;
            if let Some(guard) = logging::init(&data_dir.join("logs")) {
                app.manage(guard);
            }
            tracing::info!(version = env!("CARGO_PKG_VERSION"), "Tenajlo starting");
            let repos = repo_manager::RepoManager::load(&data_dir);
            let accounts = auth::accounts::AccountManager::load(
                &data_dir,
                std::sync::Arc::new(auth::secrets::KeyringStore),
            );
            let settings = settings::SettingsManager::load(&data_dir);
            let forgejo = forgejo::ForgejoClient::new()?;

            let prompts = auth::broker::PromptBroker::default();
            let prompt_fn = prompts.prompt_fn(app.handle().clone());
            let trampoline = match tauri::async_runtime::block_on(auth::trampoline::Trampoline::start(prompt_fn)) {
                Ok(t) => Some(t),
                Err(e) => {
                    tracing::error!(error = %e, "askpass trampoline failed to start; sign-in prompts unavailable");
                    None
                }
            };
            let askpass = auth::askpass_path();
            if askpass.is_none() {
                tracing::error!("tenajlo-askpass sidecar missing; run `node scripts/build-askpass.mjs`");
            }
            app.manage(state::AppState::new(bundled_git_dir, repos, accounts, settings, forgejo, prompts, trampoline, askpass));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Tenajlo");
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
        // Normalize line endings in case a Windows checkout converted them.
        let committed = std::fs::read_to_string(committed)
            .unwrap_or_default()
            .replace("\r\n", "\n");
        let fresh = std::fs::read_to_string(fresh).unwrap();
        assert!(
            fresh == committed,
            "src/bindings.ts is stale: run `pnpm bindings`"
        );
    }
}

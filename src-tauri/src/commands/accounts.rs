//! Forgejo sign-in and account commands (spec §6.4). Tokens flow UI → Rust only.

use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

use crate::auth::accounts::Account;
use crate::auth::secrets::Secret;
use crate::error::AppError;
use crate::forgejo::address::{join, normalize_base_url};
use crate::forgejo::ServerInfo;
use crate::git::identity::Identity;
use crate::state::AppState;

/// Signed-in accounts (metadata only).
#[tauri::command]
#[specta::specta]
pub fn list_accounts(state: State<'_, AppState>) -> Vec<Account> {
    state.accounts.list()
}

/// Checks that `server` is a reachable, trusted Forgejo server (sign-in step 2).
#[tauri::command]
#[specta::specta]
pub async fn check_server(
    state: State<'_, AppState>,
    server: String,
) -> Result<ServerInfo, AppError> {
    let base = normalize_base_url(&server)?;
    Ok(state.forgejo.server_info(&base).await?)
}

/// Verifies `token` with `GET /api/v1/user`, then saves the account and keychain entry.
#[tauri::command]
#[specta::specta]
pub async fn sign_in(
    state: State<'_, AppState>,
    server: String,
    token: String,
) -> Result<Account, AppError> {
    let base = normalize_base_url(&server)?;
    let token = Secret::new(token.trim().to_owned());
    if token.expose().is_empty() {
        return Err(AppError::invalid_input("Paste your access token first."));
    }
    let user = state.forgejo.current_user(&base, &token).await?;
    let base_url = crate::forgejo::address::base_string(&base);
    Ok(state.accounts.sign_in(&base_url, user, token).await?)
}

/// Signs out: removes the account and deletes its token from the keychain.
#[tauri::command]
#[specta::specta]
pub async fn sign_out(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let id = id
        .parse()
        .map_err(|_| AppError::invalid_input("That account id isn't valid."))?;
    Ok(state.accounts.sign_out(id).await?)
}

/// Opens the server's "Applications" settings page, where the user creates a token.
#[tauri::command]
#[specta::specta]
pub fn open_token_settings(app: AppHandle, server: String) -> Result<(), AppError> {
    let base = normalize_base_url(&server)?;
    let url = join(&base, "user/settings/applications");
    app.opener()
        .open_url(url.as_str(), None::<&str>)
        .map_err(|e| AppError::internal("Tenajlo couldn't open your web browser.", e.to_string()))
}

/// Name and email from the account's Forgejo profile, to prefill the git identity on first
/// run. Fetched live; Tenajlo doesn't store the email.
#[tauri::command]
#[specta::specta]
pub async fn get_account_identity(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<Identity, AppError> {
    let account = super::account(&state, &account_id)?;
    let token = state.accounts.usable_token(&account).await?;
    let user = state
        .forgejo
        .current_user(&normalize_base_url(&account.base_url)?, &token)
        .await
        .map_err(|e| super::account_api_error(&state, &account, e))?;
    let name = Some(user.full_name.trim().to_owned()).filter(|n| !n.is_empty());
    // A hidden email is a placeholder that shouldn't end up on commits.
    let email =
        Some(user.email.trim().to_owned()).filter(|e| !e.is_empty() && !e.contains("noreply"));
    Ok(Identity { name, email })
}

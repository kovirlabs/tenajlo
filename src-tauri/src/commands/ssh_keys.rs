//! SSH key commands (spec §6.3 key management): list and create keys in `~/.ssh`, and add
//! them to a Forgejo account. Passphrases flow UI → Rust only.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use crate::auth::secrets::Secret;
use crate::auth::ssh_keys::{self, LocalSshKey};
use crate::error::AppError;
use crate::forgejo::address::normalize_base_url;
use crate::forgejo::keys::{self, AddKeyError, KEY_UPLOAD_SCOPES};
use crate::state::AppState;

/// One account's SSH key status.
#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AccountSshKeys {
    pub account_id: String,
    /// Whether the account's token may add keys (`write:user`).
    pub can_add: bool,
    /// Fingerprints (`SHA256:…`) of the keys already on the account.
    pub fingerprints: Vec<String>,
    /// Permissions for a replacement token when `can_add` is false.
    pub required_scopes: Vec<String>,
}

fn ssh_dir(app: &AppHandle) -> Result<PathBuf, AppError> {
    app.path()
        .home_dir()
        .map(|home| ssh_keys::ssh_dir(&home))
        .map_err(|_| AppError::invalid_input("Tenajlo couldn't find your home folder."))
}

/// The public keys in `~/.ssh`.
#[tauri::command]
#[specta::specta]
pub async fn list_ssh_keys(app: AppHandle) -> Result<Vec<LocalSshKey>, AppError> {
    let dir = ssh_dir(&app)?;
    Ok(blocking(move || ssh_keys::list(&dir)).await??)
}

/// Creates `~/.ssh/id_ed25519`, protected by `passphrase` if it isn't empty.
#[tauri::command]
#[specta::specta]
pub async fn create_ssh_key(
    app: AppHandle,
    comment: String,
    passphrase: Option<String>,
) -> Result<LocalSshKey, AppError> {
    let dir = ssh_dir(&app)?;
    let passphrase = passphrase.map(Secret::new);
    Ok(blocking(move || ssh_keys::generate(&dir, &comment, passphrase.as_ref())).await??)
}

/// Which keys `account_id` already has, and whether its token may add more.
#[tauri::command]
#[specta::specta]
pub async fn get_account_ssh_keys(
    state: State<'_, AppState>,
    account_id: String,
) -> Result<AccountSshKeys, AppError> {
    let account = super::account(&state, &account_id)?;
    let token = state.accounts.usable_token(&account).await?;
    let base = normalize_base_url(&account.base_url)?;
    let (listed, allowed) = tokio::join!(
        keys::list_keys(&state.forgejo, &base, &token),
        keys::can_add_keys(&state.forgejo, &base, &token),
    );
    let api_error = |e| super::account_api_error(&state, &account, e);
    Ok(AccountSshKeys {
        account_id,
        can_add: allowed.map_err(api_error)?,
        fingerprints: listed
            .map_err(api_error)?
            .into_iter()
            .map(|k| k.fingerprint)
            .collect(),
        required_scopes: KEY_UPLOAD_SCOPES.iter().map(|s| (*s).to_owned()).collect(),
    })
}

/// Adds the public key `file_name` from `~/.ssh` to `account_id` as `title`. Checks the
/// token's permission first, so nothing is sent with a token that can't add keys.
#[tauri::command]
#[specta::specta]
pub async fn add_ssh_key(
    app: AppHandle,
    state: State<'_, AppState>,
    account_id: String,
    file_name: String,
    title: String,
) -> Result<(), AppError> {
    let title = title.trim().to_owned();
    if title.is_empty() {
        return Err(AppError::invalid_input("Give the key a name first."));
    }
    let account = super::account(&state, &account_id)?;
    let dir = ssh_dir(&app)?;
    let (_, line) = blocking(move || ssh_keys::read_public(&dir, &file_name)).await??;
    let token = state.accounts.usable_token(&account).await?;
    let base = normalize_base_url(&account.base_url)?;
    let api_error = |e| super::account_api_error(&state, &account, e);

    if !keys::can_add_keys(&state.forgejo, &base, &token)
        .await
        .map_err(api_error)?
    {
        return Err(not_allowed());
    }
    match keys::add_key(&state.forgejo, &base, &token, &title, &line).await {
        Ok(_) => {
            tracing::info!(server = %crate::redact::redact(&account.base_url), "added an SSH key");
            Ok(())
        }
        Err(AddKeyError::NotAllowed) => Err(not_allowed()),
        Err(AddKeyError::AlreadyUsed) => Err(AppError::invalid_input(
            "This key is already on the server, either on your account or on someone else's. Each key can only be added once.",
        )),
        Err(AddKeyError::Api(e)) => Err(api_error(e)),
    }
}

fn not_allowed() -> AppError {
    AppError::invalid_input(format!(
        "Your saved token can't add SSH keys. Create a new token with these permissions: {}.",
        KEY_UPLOAD_SCOPES.join(", ")
    ))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> T + Send + 'static,
) -> Result<T, AppError> {
    tokio::task::spawn_blocking(f).await.map_err(|e| {
        AppError::internal("Something went wrong reading your SSH keys.", e.to_string())
    })
}

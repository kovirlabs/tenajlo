//! Answers to sign-in prompts, remembered passwords, and cancelling running operations.

use tauri::State;

use crate::auth::prompt::AuthAnswer;
use crate::auth::saved_secrets::SavedSecretInfo;
use crate::error::AppError;
use crate::state::AppState;

/// Delivers the user's answer to a prompt (`None` = cancelled, which stops the operation).
#[tauri::command]
#[specta::specta]
pub fn answer_auth_prompt(
    state: State<'_, AppState>,
    prompt_id: String,
    answer: Option<AuthAnswer>,
) {
    state.prompts.answer(&prompt_id, answer);
}

/// Cancels a running fetch, pull or push. Unknown ids are ignored.
#[tauri::command]
#[specta::specta]
pub fn cancel_operation(state: State<'_, AppState>, op_id: String) {
    state.operations.cancel(&op_id);
}

/// Passwords and SSH passphrases the user chose to remember (names only, never the secrets).
#[tauri::command]
#[specta::specta]
pub fn list_saved_secrets(state: State<'_, AppState>) -> Vec<SavedSecretInfo> {
    state.saved_secrets.list()
}

/// Forgets a remembered password or passphrase: deletes it from the keychain.
#[tauri::command]
#[specta::specta]
pub async fn forget_saved_secret(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    Ok(state.saved_secrets.forget(&id).await?)
}

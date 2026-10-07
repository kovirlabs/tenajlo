//! Answers to sign-in prompts, and cancelling running operations.

use tauri::State;

use crate::auth::prompt::AuthAnswer;
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

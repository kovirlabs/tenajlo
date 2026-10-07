//! Connects trampoline prompts to the UI: emits `AuthPromptRequested` and waits for
//! `answer_auth_prompt`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::oneshot;
use uuid::Uuid;

use super::prompt::{AuthAnswer, PromptKind};
use super::trampoline::{PendingPrompt, PromptFn};

/// Ask the user something on behalf of a running git operation.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct AuthPromptRequested {
    pub prompt_id: String,
    pub repo_id: String,
    pub op_id: String,
    pub kind: PromptKind,
}

type Waiters = Arc<Mutex<HashMap<String, oneshot::Sender<Option<AuthAnswer>>>>>;

/// Pending UI prompts by id.
#[derive(Clone, Default)]
pub struct PromptBroker {
    waiting: Waiters,
}

impl PromptBroker {
    /// A [`PromptFn`] for the trampoline that asks through the UI.
    pub fn prompt_fn(&self, app: AppHandle) -> PromptFn {
        let waiting = self.waiting.clone();
        Arc::new(move |p: PendingPrompt| {
            let (tx, rx) = oneshot::channel();
            let prompt_id = Uuid::new_v4().to_string();
            waiting
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(prompt_id.clone(), tx);
            let event = AuthPromptRequested {
                prompt_id: prompt_id.clone(),
                repo_id: p.repo_id,
                op_id: p.op_id,
                kind: p.kind,
            };
            let emitted = event.emit(&app);
            let waiting = waiting.clone();
            Box::pin(async move {
                // Removes the waiter however this future ends: answered, timed out, or
                // dropped because the operation was cancelled.
                let _cleanup = Cleanup { waiting, prompt_id };
                if let Err(e) = emitted {
                    tracing::warn!(error = %e, "could not show auth prompt");
                    return None;
                }
                rx.await.ok().flatten()
            })
        })
    }

    /// Delivers the user's answer. Unknown or already-answered ids are ignored.
    pub fn answer(&self, prompt_id: &str, answer: Option<AuthAnswer>) {
        if let Some(tx) = self
            .waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(prompt_id)
        {
            let _ = tx.send(answer);
        }
    }
}

struct Cleanup {
    waiting: Waiters,
    prompt_id: String,
}

impl Drop for Cleanup {
    fn drop(&mut self) {
        self.waiting
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.prompt_id);
    }
}

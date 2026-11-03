//! Loopback server answering `tenajlo-askpass` requests (spec §6.5).
//!
//! Each git operation registers a random 32-byte token, valid only while its [`OpToken`]
//! lives. Requests with unknown tokens are rejected and logged without the token.
//!
//! Operations on an account's server also get an [`AccountCredential`]: in credential-helper
//! mode the trampoline answers git's `get` with the account's login and PAT, only for that
//! server's protocol and host. Other HTTPS operations get [`SavedSecrets`] logins instead, as
//! a helper after the user's own; SSH passphrases the user chose to remember are answered
//! without a dialog.

mod handlers;
mod server;
mod token;

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use super::credential_helper::AccountCredential;
use super::prompt::{AuthAnswer, PromptKind};
use super::saved_secrets::SavedSecrets;

pub use token::OpToken;

/// How long the user has to answer a prompt before the operation is cancelled.
pub const PROMPT_TIMEOUT: Duration = Duration::from_secs(5 * 60);

/// A prompt waiting for the user.
#[derive(Debug, Clone)]
pub struct PendingPrompt {
    pub repo_id: String,
    pub op_id: String,
    pub kind: PromptKind,
}

/// Shows a prompt and resolves with the user's answer (`None` = cancelled).
pub type PromptFn = Arc<
    dyn Fn(PendingPrompt) -> Pin<Box<dyn Future<Output = Option<AuthAnswer>> + Send>> + Send + Sync,
>;

struct OpEntry {
    repo_id: String,
    op_id: String,
    cancel: CancellationToken,
    account: Option<AccountCredential>,
    /// git asked for and received the account's credential.
    account_used: bool,
    /// Password from a combined username+password dialog, handed to git's next
    /// `Password for …` prompt and then forgotten.
    cached_password: Option<String>,
    /// Keys whose passphrase was already asked for: asking again means it was wrong.
    passphrase_keys: Vec<String>,
    /// Background operations never show prompts; git just gets no answer.
    interactive: bool,
    /// The user ticked "Remember" when signing in: save the login once git reports it worked.
    remember_login: bool,
}

struct Inner {
    port: u16,
    ops: Mutex<HashMap<String, OpEntry>>,
    prompt: PromptFn,
    prompt_timeout: Duration,
    /// Remembered logins and passphrases; `None` in tests that don't need them.
    saved: Option<Arc<SavedSecrets>>,
}

/// The running trampoline server. Cheap to clone.
#[derive(Clone)]
pub struct Trampoline {
    inner: Arc<Inner>,
}

impl Trampoline {
    /// Binds `127.0.0.1:0` and starts accepting helper connections. Nothing is remembered.
    pub async fn start(prompt: PromptFn) -> std::io::Result<Self> {
        Self::spawn(prompt, PROMPT_TIMEOUT, None).await
    }

    /// Like [`Self::start`], also answering from and saving to `saved`.
    pub async fn start_with_saved(
        prompt: PromptFn,
        saved: Arc<SavedSecrets>,
    ) -> std::io::Result<Self> {
        Self::spawn(prompt, PROMPT_TIMEOUT, Some(saved)).await
    }

    /// Like [`Self::start`] with a custom prompt timeout (tests).
    pub async fn start_with_timeout(
        prompt: PromptFn,
        prompt_timeout: Duration,
    ) -> std::io::Result<Self> {
        Self::spawn(prompt, prompt_timeout, None).await
    }
}

impl Inner {
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, OpEntry>> {
        self.ops.lock().unwrap_or_else(|p| p.into_inner())
    }
}

#[cfg(test)]
mod tests;

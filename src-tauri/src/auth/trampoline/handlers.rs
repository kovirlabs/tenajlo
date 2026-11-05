//! Request handling: credential-helper requests, remembered logins, and askpass prompts.

use tenajlo_askpass::{CredentialOp, Fields, Request};

use super::{Inner, PendingPrompt};
use crate::auth::credential_helper;
use crate::auth::prompt::{parse_prompt, PromptKind};
use crate::auth::saved_secrets::SavedSecrets;
use crate::auth::secrets::Secret;

/// Credential-helper request: answered from the operation's account if it has one, else
/// from remembered logins.
pub(super) async fn credential(inner: &Inner, req: &Request) -> Fields {
    let remember = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return Vec::new();
        };
        if let Some(account) = &entry.account {
            // `store` and `erase` are ignored (see [`credential_helper::answer`]).
            return match credential_helper::answer(account, req.op, &req.fields) {
                Some(fields) => {
                    entry.account_used = true;
                    fields
                }
                None => Vec::new(),
            };
        }
        entry.remember_login
    };
    let Some(saved) = &inner.saved else {
        return Vec::new();
    };
    saved_login(saved, req.op, &req.fields, remember).await
}

/// `get` answers from a remembered login; `store` saves the login the user asked to remember;
/// `erase` forgets a remembered login the server rejected.
async fn saved_login(
    saved: &SavedSecrets,
    op: Option<CredentialOp>,
    fields: &Fields,
    remember: bool,
) -> Fields {
    let field = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    let (Some(protocol @ ("https" | "http")), Some(host)) = (field("protocol"), field("host"))
    else {
        return Vec::new();
    };
    match op {
        Some(CredentialOp::Get) => match saved.login(protocol, host, field("username")).await {
            Some((username, password)) => vec![
                ("username".into(), username),
                ("password".into(), password.expose().to_owned()),
            ],
            None => Vec::new(),
        },
        Some(CredentialOp::Store) if remember => {
            if let (Some(username), Some(password)) = (field("username"), field("password")) {
                let password = Secret::new(password.to_owned());
                if let Err(e) = saved.save_login(protocol, host, username, password).await {
                    tracing::warn!(error = %e, "could not remember a login");
                }
            }
            Vec::new()
        }
        Some(CredentialOp::Erase) => {
            if let (Some(username), Some(password)) = (field("username"), field("password")) {
                saved
                    .forget_rejected_login(protocol, host, username, password)
                    .await;
            }
            Vec::new()
        }
        _ => Vec::new(),
    }
}

pub(super) async fn answer(inner: &Inner, req: Request) -> Option<String> {
    let mut kind = parse_prompt(req.prompt.as_deref().unwrap_or_default());
    let interactive = {
        let mut ops = inner.lock();
        let Some(entry) = ops.get_mut(&req.token) else {
            tracing::warn!("rejected trampoline request: unknown or expired token");
            return None;
        };
        if let PromptKind::Passphrase { key, retry } = &mut kind {
            *retry = entry.passphrase_keys.contains(key);
            if !*retry {
                entry.passphrase_keys.push(key.clone());
            }
        }
        entry.interactive
    };
    // A remembered passphrase answers without a dialog, even for background operations.
    // OpenSSH asking again for the same key means it was wrong, so it's forgotten.
    if let (PromptKind::Passphrase { key, retry }, Some(saved)) = (&kind, &inner.saved) {
        if *retry {
            saved.forget_passphrase(key).await;
        } else if let Some(passphrase) = saved.passphrase(key).await {
            return Some(passphrase.expose().to_owned());
        }
    }
    if !interactive {
        return None;
    }

    let (pending, cancel) = {
        let mut ops = inner.lock();
        let entry = ops.get_mut(&req.token)?;
        if let (PromptKind::Password { .. }, Some(pw)) = (&kind, entry.cached_password.take()) {
            return Some(pw);
        }
        let pending = PendingPrompt {
            repo_id: entry.repo_id.clone(),
            op_id: entry.op_id.clone(),
            kind: kind.clone(),
        };
        (pending, entry.cancel.clone())
    };

    let asked = tokio::select! {
        res = tokio::time::timeout(inner.prompt_timeout, (inner.prompt)(pending)) => res.ok().flatten(),
        () = cancel.cancelled() => None,
    };
    let Some(answer) = asked else {
        // Spec §6.5: cancelling or timing out kills the git process.
        cancel.cancel();
        return None;
    };
    match kind {
        PromptKind::Credentials { .. } | PromptKind::Password { .. } => {
            let mut ops = inner.lock();
            let entry = ops.get_mut(&req.token);
            if let Some(entry) = entry {
                // Saved by `store` once git reports the login worked.
                entry.remember_login |= answer.remember;
                if matches!(kind, PromptKind::Credentials { .. }) {
                    entry.cached_password = Some(answer.secret);
                    return Some(answer.username.unwrap_or_default());
                }
            }
            Some(answer.secret)
        }
        PromptKind::Passphrase { key, .. } => {
            if let (true, Some(saved)) = (answer.remember, &inner.saved) {
                // Saved now: OpenSSH never says it worked, but asks again if it didn't.
                let passphrase = Secret::new(answer.secret.clone());
                if let Err(e) = saved.save_passphrase(&key, passphrase).await {
                    tracing::warn!(error = %e, "could not remember an SSH key passphrase");
                }
            }
            Some(answer.secret)
        }
        // Accepting is decided here, not by the UI's text: OpenSSH expects exactly "yes".
        PromptKind::HostKey { .. } => Some("yes".to_owned()),
        PromptKind::Other { .. } => Some(answer.secret),
    }
}

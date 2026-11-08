//! "Open in Tenajlo" links (spec §8.3, v1.5): `tenajlo://clone?url=<percent-encoded URL>`.
//!
//! A link can come from any web page, so it is untrusted input. It only fills in the Clone
//! dialog's URL tab; the user still picks the folder and clicks Clone. Links are checked here
//! (scheme, action, exactly one `url` parameter, and the clone dialog's own URL rules) before
//! the frontend sees anything, and a rejected link is ignored with a redacted log line.
//!
//! Delivery: a link that started Tenajlo (Windows/Linux command line, macOS open event) or
//! reached the running copy (single-instance forwarding, macOS open event) is kept in
//! [`PendingCloneLink`] and announced with [`CloneLinkReceived`]. The frontend takes it with
//! `take_clone_link` once its main screen is up, so a link that arrives during startup waits.

use std::sync::Mutex;

use reqwest::Url;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::git::clone::validate_url;
use crate::redact::redact;

/// The URL scheme Tenajlo registers (`tauri.conf.json` → `plugins.deep-link.desktop.schemes`).
pub const SCHEME: &str = "tenajlo";
/// Longest link accepted, before decoding. Forgejo clone URLs are far shorter.
const MAX_LINK_LEN: usize = 4096;
/// Longest clone URL accepted, after decoding.
const MAX_URL_LEN: usize = 2048;
/// Longest form of a rejected link written to the log.
const MAX_LOG_LEN: usize = 300;

/// What a valid link asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkRequest {
    /// Fill in the Clone dialog with this URL (already checked by the clone URL rules).
    Clone { url: String },
}

/// Why a link was ignored. Only for logs; the user sees nothing.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinkError {
    #[error("link is too long")]
    TooLong,
    #[error("link contains control characters or spaces")]
    BadCharacters,
    #[error("not a URL")]
    NotAUrl,
    #[error("not a tenajlo: link")]
    WrongScheme,
    #[error("unknown action")]
    UnknownAction,
    #[error("unexpected parts in the link")]
    UnexpectedParts,
    #[error("expected exactly one url parameter and nothing else")]
    BadParameters,
    #[error("clone URL rejected: {0}")]
    BadCloneUrl(&'static str),
}

/// Parses and checks an Open in Tenajlo link. Pure: no I/O, no logging.
///
/// Accepts `tenajlo://clone?url=<URL>` (a trailing `/` after `clone` is allowed, since some
/// launchers add one). The clone URL must pass the Clone dialog's rules
/// ([`validate_url`]: `https://`, `http://` only for loopback test servers, `ssh://`, scp-style
/// `user@host:path`; no password, nothing git or ssh could read as an option), and in addition
/// must not carry any user name in an `http(s)` URL (a token can hide there), must not start
/// with `-`, and must not contain control characters or whitespace.
pub fn parse_link(raw: &str) -> Result<LinkRequest, LinkError> {
    if raw.len() > MAX_LINK_LEN {
        return Err(LinkError::TooLong);
    }
    if raw.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(LinkError::BadCharacters);
    }
    let link = Url::parse(raw).map_err(|_| LinkError::NotAUrl)?;
    if link.scheme() != SCHEME {
        return Err(LinkError::WrongScheme);
    }
    if !link
        .host_str()
        .is_some_and(|h| h.eq_ignore_ascii_case("clone"))
    {
        return Err(LinkError::UnknownAction);
    }
    let extra_parts = !link.username().is_empty()
        || link.password().is_some()
        || link.port().is_some()
        || !matches!(link.path(), "" | "/")
        || link.fragment().is_some();
    if extra_parts {
        return Err(LinkError::UnexpectedParts);
    }
    let mut pairs = link.query_pairs();
    let url = match (pairs.next(), pairs.next()) {
        (Some((key, value)), None) if key == "url" => value.into_owned(),
        _ => return Err(LinkError::BadParameters),
    };
    check_clone_url(&url)?;
    Ok(LinkRequest::Clone { url })
}

/// The extra rules for a clone URL that came from a link, then the Clone dialog's own rules.
fn check_clone_url(url: &str) -> Result<(), LinkError> {
    if url.is_empty() {
        return Err(LinkError::BadCloneUrl("empty"));
    }
    if url.len() > MAX_URL_LEN {
        return Err(LinkError::BadCloneUrl("too long"));
    }
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(LinkError::BadCloneUrl("control characters or spaces"));
    }
    if url.starts_with('-') {
        return Err(LinkError::BadCloneUrl("starts with -"));
    }
    validate_url(url).map_err(|e| match e {
        crate::git::clone::CloneError::InvalidUrl(reason) => LinkError::BadCloneUrl(reason),
        _ => LinkError::BadCloneUrl("invalid"),
    })?;
    if let Ok(parsed) = Url::parse(url) {
        if matches!(parsed.scheme(), "http" | "https") && !parsed.username().is_empty() {
            return Err(LinkError::BadCloneUrl("contains a user name"));
        }
    }
    Ok(())
}

/// A rejected link as it may appear in the log: query values decoded (so an encoded
/// `https://user:token@host` is visible to [`redact`]), credentials masked, shortened.
/// Use with `{:?}` so control characters are escaped.
pub fn for_log(raw: &str) -> String {
    let readable = match Url::parse(raw) {
        Ok(link) => {
            let mut out = format!(
                "{}://{}{}",
                link.scheme(),
                link.host_str().unwrap_or_default(),
                link.path()
            );
            let params: Vec<String> = link
                .query_pairs()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            if !params.is_empty() {
                out.push('?');
                out.push_str(&params.join("&"));
            }
            out
        }
        Err(_) => raw.to_owned(),
    };
    redact(&readable).chars().take(MAX_LOG_LEN).collect()
}

/// Sent to the frontend when a valid link arrives. The frontend then calls `take_clone_link`
/// (once its main screen is up), which also covers links that arrived before it listened.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct CloneLinkReceived {
    /// The checked clone URL.
    pub url: String,
}

/// The most recent valid link the frontend hasn't taken yet.
#[derive(Default)]
pub struct PendingCloneLink(Mutex<Option<String>>);

impl PendingCloneLink {
    /// Keeps `url`, replacing an older link that wasn't taken.
    pub fn set(&self, url: String) {
        *self.0.lock().unwrap_or_else(|p| p.into_inner()) = Some(url);
    }

    /// Returns and forgets the pending link.
    pub fn take(&self) -> Option<String> {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).take()
    }
}

/// Handles one link from the OS: valid → pending + event; otherwise logged (redacted) and
/// ignored.
pub fn receive(app: &AppHandle, raw: &str) {
    match parse_link(raw) {
        Ok(LinkRequest::Clone { url }) => {
            tracing::info!(url = %redact(&url), "Open in Tenajlo link received");
            app.state::<PendingCloneLink>().set(url.clone());
            if let Err(e) = (CloneLinkReceived { url }).emit(app) {
                tracing::warn!(error = %e, "couldn't announce an Open in Tenajlo link");
            }
        }
        Err(reason) => {
            tracing::warn!(link = ?for_log(raw), %reason, "ignored an Open in Tenajlo link");
        }
    }
}

/// Starts handling links: those arriving from now on, and the one that started Tenajlo.
/// Call once from `setup`, after [`PendingCloneLink`] is managed.
pub fn listen(app: &AppHandle) {
    use tauri_plugin_deep_link::DeepLinkExt;

    // Installed copies are registered by their package (NSIS registry keys under HKCU, the
    // .deb desktop file's MimeType, the macOS Info.plist). A development build on Windows or
    // Linux registers itself so links can be tried; macOS only honors bundled apps.
    #[cfg(all(debug_assertions, any(windows, target_os = "linux")))]
    if let Err(e) = app.deep_link().register_all() {
        tracing::warn!(error = %e, "couldn't register the tenajlo: link scheme for development");
    }

    let handle = app.clone();
    app.deep_link().on_open_url(move |event| {
        for url in event.urls() {
            receive(&handle, url.as_str());
        }
    });
    // A link that started Tenajlo: the command line on Windows/Linux (read before this ran).
    match app.deep_link().get_current() {
        Ok(Some(urls)) => urls.iter().for_each(|url| receive(app, url.as_str())),
        Ok(None) => {}
        Err(e) => tracing::warn!(error = %e, "couldn't read the link that started Tenajlo"),
    }
}

/// Brings the main window to the front (a link forwarded to the running copy).
pub fn focus_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn clone_url(raw: &str) -> Result<String, LinkError> {
        parse_link(raw).map(|LinkRequest::Clone { url }| url)
    }

    #[test]
    fn accepts_forgejo_links() {
        // Forgejo fills `{url}` with encodeURIComponent(clone URL).
        assert_eq!(
            clone_url("tenajlo://clone?url=https%3A%2F%2Fgit.example.com%2Fteam%2Fplc.git"),
            Ok("https://git.example.com/team/plc.git".into())
        );
        assert_eq!(
            clone_url(
                "tenajlo://clone/?url=ssh%3A%2F%2Fgit%40git.example.com%3A2222%2Fteam%2Fplc.git"
            ),
            Ok("ssh://git@git.example.com:2222/team/plc.git".into())
        );
        assert_eq!(
            clone_url("tenajlo://clone?url=git%40git.example.com%3Ateam%2Fplc.git"),
            Ok("git@git.example.com:team/plc.git".into())
        );
        assert_eq!(
            clone_url("TENAJLO://CLONE?url=http%3A%2F%2Flocalhost%3A3000%2Fevan%2Fx.git"),
            Ok("http://localhost:3000/evan/x.git".into()),
            "scheme and action are case-insensitive; loopback http like the Clone dialog"
        );
        // Not encoded at all still parses (the URL has no `&` or `#`).
        assert_eq!(
            clone_url("tenajlo://clone?url=https://git.example.com/team/plc.git"),
            Ok("https://git.example.com/team/plc.git".into())
        );
    }

    #[test]
    fn decodes_encoded_and_unicode_characters() {
        assert_eq!(
            clone_url("tenajlo://clone?url=https%3A%2F%2Fgit.example.com%2Fteam%2Fa%2Bb%20.git"),
            Err(LinkError::BadCloneUrl("control characters or spaces")),
            "%20 decodes to a space"
        );
        assert_eq!(
            clone_url("tenajlo://clone?url=https%3A%2F%2Fgit.example.com%2Fteam%2Fa%2Bb.git"),
            Ok("https://git.example.com/team/a+b.git".into())
        );
        assert_eq!(
            clone_url(
                "tenajlo://clone?url=https%3A%2F%2Fgit.example.com%2F%C3%BCnic%C3%B8de%2Frepo.git"
            ),
            Ok("https://git.example.com/ünicøde/repo.git".into())
        );
    }

    #[test]
    fn rejects_other_links() {
        for (raw, why) in [
            ("https://git.example.com/x.git", LinkError::WrongScheme),
            (
                "vscode://clone?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::WrongScheme,
            ),
            (
                "tenajlo://open?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::UnknownAction,
            ),
            (
                "tenajlo:clone?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::UnknownAction,
            ),
            (
                "tenajlo://clone/extra?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::UnexpectedParts,
            ),
            (
                "tenajlo://u:p@clone?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::UnexpectedParts,
            ),
            (
                "tenajlo://clone:99?url=https%3A%2F%2Fh%2Fx.git",
                LinkError::UnexpectedParts,
            ),
            (
                "tenajlo://clone?url=https%3A%2F%2Fh%2Fx.git#frag",
                LinkError::UnexpectedParts,
            ),
            ("tenajlo://clone", LinkError::BadParameters),
            (
                "tenajlo://clone?repo=https%3A%2F%2Fh%2Fx.git",
                LinkError::BadParameters,
            ),
            (
                "tenajlo://clone?url=https%3A%2F%2Fh%2Fa.git&url=https%3A%2F%2Fh%2Fb.git",
                LinkError::BadParameters,
            ),
            (
                "tenajlo://clone?url=https%3A%2F%2Fh%2Fa.git&path=C%3A%5Cevil",
                LinkError::BadParameters,
            ),
            (
                "tenajlo://clone?url=https://h/a.git&x=1",
                LinkError::BadParameters,
            ),
            ("tenajlo://clone?url=", LinkError::BadCloneUrl("empty")),
            ("tenajlo://clone url", LinkError::BadCharacters),
            (
                "tenajlo://clone?url=https://h/a.git\n",
                LinkError::BadCharacters,
            ),
            ("not a link", LinkError::BadCharacters),
            ("", LinkError::NotAUrl),
        ] {
            assert_eq!(parse_link(raw).map(|_| ()), Err(why), "{raw:?}");
        }
    }

    #[test]
    fn rejects_unsafe_clone_urls() {
        for url in [
            "http%3A%2F%2Fgit.example.com%2Fx.git", // plain http to a real host
            "https%3A%2F%2Fevan%3Apat%40h%2Fx.git", // password
            "https%3A%2F%2Fghp_token%40h%2Fx.git",  // token as user name
            "file%3A%2F%2F%2Ftmp%2Fx",              // local path
            "%2Ftmp%2Fx",                           // local path
            "C%3A%5Crepos%5Cx",                     // Windows path
            "--upload-pack%3Devil",                 // option
            "-oProxyCommand%3Devil%3Ax",            // option
            "ssh%3A%2F%2F-oProxyCommand%3Devil%2Fx", // option as host
            "git%40-oProxyCommand%3Devil%3Ax",      // option as scp host
            "ext%3A%3Ash%20-c%20evil",              // git remote helper
            "fd%3A%3A17",                           // git remote helper
            "https%3A%2F%2Fh%2Fx.git%0Aevil",       // newline
            "https%3A%2F%2Fh%2Fx%00.git",           // NUL
            "%20https%3A%2F%2Fh%2Fx.git",           // leading space
        ] {
            let raw = format!("tenajlo://clone?url={url}");
            assert!(
                matches!(parse_link(&raw), Err(LinkError::BadCloneUrl(_))),
                "{raw:?} → {:?}",
                parse_link(&raw)
            );
        }
    }

    #[test]
    fn rejects_overlong_input() {
        let long_link = format!(
            "tenajlo://clone?url=https://h/{}.git",
            "a".repeat(MAX_LINK_LEN)
        );
        assert_eq!(parse_link(&long_link), Err(LinkError::TooLong));
        let long_url = format!(
            "tenajlo://clone?url=https://h/{}.git",
            "a".repeat(MAX_URL_LEN)
        );
        assert_eq!(
            parse_link(&long_url),
            Err(LinkError::BadCloneUrl("too long"))
        );
    }

    #[test]
    fn log_form_masks_credentials_even_when_encoded() {
        let logged = for_log("tenajlo://clone?url=https%3A%2F%2Fevan%3Asecret-pat%40h%2Fx.git&x=1");
        assert!(!logged.contains("secret-pat"), "{logged}");
        assert!(!logged.contains("evan"), "{logged}");
        assert_eq!(logged, "tenajlo://clone?url=https://***@h/x.git&x=1");

        let logged = for_log("tenajlo://user:secret-pat@clone?url=x");
        assert!(!logged.contains("secret-pat"), "{logged}");

        let logged = for_log("not a url https://evan:secret-pat@h/x");
        assert!(!logged.contains("secret-pat"), "{logged}");

        assert_eq!(for_log(&"a".repeat(5000)).chars().count(), MAX_LOG_LEN);
    }

    #[test]
    fn pending_link_is_taken_once() {
        let pending = PendingCloneLink::default();
        assert_eq!(pending.take(), None);
        pending.set("https://h/a.git".into());
        pending.set("https://h/b.git".into());
        assert_eq!(pending.take().as_deref(), Some("https://h/b.git"));
        assert_eq!(pending.take(), None);
    }
}

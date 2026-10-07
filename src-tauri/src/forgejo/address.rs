//! Normalizing the server address the user types (spec §6.4 step 1).

use reqwest::Url;

use super::ForgejoError;

/// Normalizes a user-typed server address to `https://host[:port][/path]`, no trailing `/`.
///
/// Adds `https://` when no scheme is given. Plain `http://` is only allowed for loopback
/// hosts (a local test server): a token must never cross the network unencrypted.
/// Rejects credentials, queries and fragments in the address.
pub fn normalize_base_url(input: &str) -> Result<Url, ForgejoError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ForgejoError::InvalidUrl("empty".into()));
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let mut url = Url::parse(&with_scheme).map_err(|e| ForgejoError::InvalidUrl(e.to_string()))?;

    match url.scheme() {
        "https" => {}
        "http" if is_loopback(&url) => {}
        "http" => {
            return Err(ForgejoError::InvalidUrl(
                "http is not allowed; use https".into(),
            ))
        }
        other => {
            return Err(ForgejoError::InvalidUrl(format!(
                "unsupported scheme {other}"
            )))
        }
    }
    if url.host_str().is_none_or(str::is_empty) {
        return Err(ForgejoError::InvalidUrl("missing host".into()));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(ForgejoError::InvalidUrl(
            "the address must not contain a user name or password".into(),
        ));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(ForgejoError::InvalidUrl(
            "the address must not contain ? or #".into(),
        ));
    }
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url)
}

/// `base` without a trailing slash, the form stored in accounts and keychain keys.
pub fn base_string(base: &Url) -> String {
    base.as_str().trim_end_matches('/').to_owned()
}

/// `base` joined with a relative API path, keeping any sub-path install prefix.
pub fn join(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    let joined = format!("{}/{}", base.path().trim_end_matches('/'), path);
    url.set_path(&joined);
    url
}

/// True for `localhost`, `127.0.0.0/8` and `::1`.
pub fn is_loopback(url: &Url) -> bool {
    match url.host_str() {
        Some(h) if h.eq_ignore_ascii_case("localhost") => true,
        Some(h) => h
            .trim_start_matches('[')
            .trim_end_matches(']')
            .parse::<std::net::IpAddr>()
            .is_ok_and(|ip| ip.is_loopback()),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn norm(s: &str) -> Result<String, ForgejoError> {
        normalize_base_url(s).map(|u| base_string(&u))
    }

    #[test]
    fn adds_https_and_strips_trailing_slash() {
        assert_eq!(
            norm("  GIT.EXAMPLE.COM/ ").unwrap(),
            "https://git.example.com"
        );
        assert_eq!(
            norm("https://git.example.com:3000/forgejo//").unwrap(),
            "https://git.example.com:3000/forgejo"
        );
    }

    #[test]
    fn http_only_for_loopback() {
        assert_eq!(
            norm("http://localhost:3000").unwrap(),
            "http://localhost:3000"
        );
        assert_eq!(
            norm("http://127.0.0.1:3000/").unwrap(),
            "http://127.0.0.1:3000"
        );
        assert_eq!(norm("http://[::1]:3000").unwrap(), "http://[::1]:3000");
        assert!(matches!(
            norm("http://git.example.com"),
            Err(ForgejoError::InvalidUrl(_))
        ));
    }

    #[test]
    fn rejects_bad_addresses() {
        for bad in [
            "",
            "ftp://host",
            "https://user:pat@host",
            "https://host/?a=b",
            "https://host/#x",
            "https://",
        ] {
            assert!(norm(bad).is_err(), "{bad:?} should be rejected");
        }
    }

    #[test]
    fn join_keeps_subpath() {
        let base = normalize_base_url("https://h/forgejo").unwrap();
        assert_eq!(
            join(&base, "api/v1/version").as_str(),
            "https://h/forgejo/api/v1/version"
        );
        let root = normalize_base_url("https://h").unwrap();
        assert_eq!(join(&root, "api/v1/user").as_str(), "https://h/api/v1/user");
    }
}

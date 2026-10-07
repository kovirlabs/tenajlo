//! Answers git credential-helper requests from an account (spec §6.2 step 1).

use tenajlo_askpass::{CredentialOp, Fields};

use super::secrets::Secret;

/// The account credential git may use during one operation.
#[derive(Debug, Clone)]
pub struct AccountCredential {
    /// `https` (or `http` for a loopback test server).
    pub protocol: String,
    /// `host` or `host:port`, as git's credential protocol sends it.
    pub host: String,
    pub login: String,
    pub token: Secret,
}

/// The reply to a credential-helper request, or `None` for "not ours".
///
/// Answers `get` only for the account's protocol and host, and only when git hasn't named a
/// different user. `store` and `erase` are ignored: the PAT is managed by sign-in, and after
/// an auth failure Tenajlo checks the token itself instead of deleting it.
pub fn answer(
    account: &AccountCredential,
    op: Option<CredentialOp>,
    fields: &Fields,
) -> Option<Fields> {
    if op != Some(CredentialOp::Get) {
        return None;
    }
    let field = |key: &str| {
        fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    };
    // git sends `host:443` when the remote URL spells out the default port.
    let default_port = if account.protocol == "https" {
        ":443"
    } else {
        ":80"
    };
    let same_server = field("protocol") == Some(account.protocol.as_str())
        && field("host").is_some_and(|h| {
            h.strip_suffix(default_port)
                .unwrap_or(h)
                .eq_ignore_ascii_case(&account.host)
        });
    // A remote URL naming another user (`https://someone@host/…`) is not ours to answer.
    let same_user = field("username").is_none_or(|u| u == account.login);
    (same_server && same_user).then(|| {
        vec![
            ("username".into(), account.login.clone()),
            ("password".into(), account.token.expose().to_owned()),
        ]
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn account() -> AccountCredential {
        AccountCredential {
            protocol: "https".into(),
            host: "tmc-git01.tmus.local".into(),
            login: "evan".into(),
            token: Secret::new("pat-123".into()),
        }
    }

    fn ask(op: CredentialOp, fields: &[(&str, &str)]) -> Option<Fields> {
        let fields = fields
            .iter()
            .map(|(k, v)| ((*k).into(), (*v).into()))
            .collect();
        answer(&account(), Some(op), &fields)
    }

    #[test]
    fn answers_get_for_the_account_server() {
        let expected = Some(vec![
            ("username".into(), "evan".into()),
            ("password".into(), "pat-123".into()),
        ]);
        let host = [("protocol", "https"), ("host", "tmc-git01.tmus.local")];
        assert_eq!(ask(CredentialOp::Get, &host), expected);
        let explicit = [
            ("protocol", "https"),
            ("host", "TMC-GIT01.tmus.local:443"),
            ("username", "evan"),
        ];
        assert_eq!(ask(CredentialOp::Get, &explicit), expected);
    }

    #[test]
    fn refuses_other_servers_users_and_ops() {
        for wrong in [
            vec![("protocol", "http"), ("host", "tmc-git01.tmus.local")],
            vec![("protocol", "https"), ("host", "tmc-git01.tmus.local:3000")],
            vec![("protocol", "https"), ("host", "evil.example")],
            vec![
                ("protocol", "https"),
                ("host", "tmc-git01.tmus.local.evil.example"),
            ],
            vec![
                ("protocol", "https"),
                ("host", "tmc-git01.tmus.local"),
                ("username", "other"),
            ],
            vec![("host", "tmc-git01.tmus.local")],
        ] {
            assert_eq!(ask(CredentialOp::Get, &wrong), None, "{wrong:?}");
        }
        let host = [("protocol", "https"), ("host", "tmc-git01.tmus.local")];
        assert_eq!(ask(CredentialOp::Store, &host), None);
        assert_eq!(ask(CredentialOp::Erase, &host), None);
        assert_eq!(answer(&account(), None, &Vec::new()), None);
    }
}

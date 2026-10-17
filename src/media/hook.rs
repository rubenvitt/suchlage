//! Entscheidung für MediaMTX `authMethod: http` (ADR 0003). Publish nur die eigene RC auf
//! ihrem Pfad, Lesen und Playback nur der interne Leser (der Server-Proxy).

use serde::Deserialize;

use crate::sn::is_valid_sn;

pub const INTERNAL_USER: &str = "suchlage-internal";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AuthRequest {
    pub user: String,
    pub password: String,
    pub action: String,
    pub path: String,
    pub protocol: String,
    pub query: String,
}

impl AuthRequest {
    /// RTMP überträgt Zugangsdaten als `?user=…&pass=…` (M0).
    pub fn credentials(&self) -> (String, String) {
        if !self.user.is_empty() {
            return (self.user.clone(), self.password.clone());
        }
        let mut user = String::new();
        let mut pass = String::new();
        for (k, v) in url::form_urlencoded::parse(self.query.trim_start_matches('?').as_bytes()) {
            match k.as_ref() {
                "user" => user = v.into_owned(),
                "pass" => pass = v.into_owned(),
                _ => {}
            }
        }
        (user, pass)
    }

    /// SN aus `live/<SN>`, sonst `None`.
    pub fn gateway_sn(&self) -> Option<&str> {
        self.path.strip_prefix("live/").filter(|sn| is_valid_sn(sn))
    }
}

pub fn decide(req: &AuthRequest, internal_secret: &str, stream_key: Option<&str>) -> bool {
    let (user, password) = req.credentials();
    match req.action.as_str() {
        "publish" => match (req.gateway_sn(), stream_key) {
            (Some(sn), Some(key)) => {
                !key.is_empty() && user == format!("rc-{sn}") && password == key
            }
            _ => false,
        },
        "read" | "playback" => {
            !internal_secret.is_empty() && user == INTERNAL_USER && password == internal_secret
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(action: &str, path: &str, user: &str, password: &str) -> AuthRequest {
        AuthRequest {
            action: action.into(),
            path: path.into(),
            user: user.into(),
            password: password.into(),
            ..Default::default()
        }
    }

    #[test]
    fn rc_may_publish_only_on_its_own_path_with_its_key() {
        assert!(decide(
            &req("publish", "live/RC1", "rc-RC1", "key"),
            "s",
            Some("key")
        ));
        assert!(!decide(
            &req("publish", "live/RC1", "rc-RC1", "falsch"),
            "s",
            Some("key")
        ));
        assert!(!decide(
            &req("publish", "live/RC1", "rc-RC2", "key"),
            "s",
            Some("key")
        ));
        assert!(
            !decide(&req("publish", "live/RC1", "rc-RC1", ""), "s", Some("")),
            "leerer Schlüssel gilt nie"
        );
        assert!(!decide(
            &req("publish", "andere/RC1", "rc-RC1", "key"),
            "s",
            Some("key")
        ));
    }

    #[test]
    fn rtmp_query_credentials_are_understood() {
        let r = AuthRequest {
            action: "publish".into(),
            path: "live/RC1".into(),
            query: "user=rc-RC1&pass=key".into(),
            ..Default::default()
        };
        assert!(decide(&r, "s", Some("key")));
    }

    #[test]
    fn only_the_internal_reader_may_read_or_play_back() {
        assert!(decide(
            &req("read", "live/RC1", INTERNAL_USER, "s"),
            "s",
            None
        ));
        assert!(decide(
            &req("playback", "live/RC1", INTERNAL_USER, "s"),
            "s",
            None
        ));
        assert!(!decide(&req("read", "live/RC1", "", ""), "s", None));
        assert!(!decide(
            &req("read", "live/RC1", INTERNAL_USER, "falsch"),
            "s",
            None
        ));
        assert!(!decide(&req("api", "", INTERNAL_USER, "s"), "s", None));
    }
}

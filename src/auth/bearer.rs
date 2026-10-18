//! Zugriffstokens der Flugmappe (SUL-66): JWT nach RFC 9068 von Pocket ID, lokal geprüft gegen den
//! JWKS des in den OIDC-Einstellungen stehenden Ausstellers. Kein Rückfall über Userinfo.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openidconnect::core::{
    CoreJsonWebKey, CoreJsonWebKeySet, CoreJsonWebKeyUse, CoreJwsSigningAlgorithm,
};
use openidconnect::{JsonWebKey, JsonWebKeyAlgorithm};
use serde::Deserialize;

use crate::auth::oidc::OidcState;
use crate::error::AppError;

/// Spielraum für Uhren, die nicht genau gehen (Ablauf und Ausstellungszeit).
pub const CLOCK_SKEW_S: i64 = 60;

/// Woran das Token gemessen wird.
#[derive(Debug, Clone, Copy)]
pub struct Expected<'a> {
    /// Aussteller aus den OIDC-Einstellungen.
    pub issuer: &'a str,
    /// Client-ID der Flugmappe aus den Einstellungen „Flugmappe“.
    pub audience: &'a str,
    /// Jetzt, in Sekunden seit Epoch.
    pub now_s: i64,
}

/// Was nach der Prüfung zählt: das Konto in Suchlage hängt an (`iss`, `sub`).
#[derive(Debug, Clone, PartialEq)]
pub struct AccessClaims {
    pub issuer: String,
    pub subject: String,
}

/// Warum ein Token abgelehnt wurde. Jeder Grund ist ein 401.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    Malformed,
    /// `none`, HS*, unbekannt oder nicht zum Schlüssel passend.
    Algorithm,
    /// `typ` ist nicht `at+jwt`.
    Type,
    /// Kein Schlüssel mit diesem `kid` im JWKS.
    UnknownKey,
    Signature,
    /// Ein ID-Token (`type: id-token`) ist kein Zugriffstoken.
    IdToken,
    Issuer,
    Expired,
    NotYetValid,
    Audience,
    ClientId,
}

impl Reason {
    pub fn message(self) -> &'static str {
        match self {
            Reason::Malformed => "Zugriffstoken unlesbar",
            Reason::Algorithm => "Zugriffstoken mit unzulässigem Signaturverfahren",
            Reason::Type | Reason::IdToken => "Kein Zugriffstoken (at+jwt)",
            Reason::UnknownKey => "Zugriffstoken mit unbekanntem Schlüssel",
            Reason::Signature => "Signatur des Zugriffstokens ungültig",
            Reason::Issuer => "Zugriffstoken von einem anderen Aussteller",
            Reason::Expired => "Zugriffstoken abgelaufen",
            Reason::NotYetValid => "Zugriffstoken noch nicht gültig",
            Reason::Audience | Reason::ClientId => {
                "Zugriffstoken nicht für die Flugmappe ausgestellt"
            }
        }
    }
}

#[derive(Deserialize)]
struct Header {
    alg: serde_json::Value,
    typ: Option<String>,
    kid: Option<String>,
    /// Erweiterungen, die verstanden werden müssen – Suchlage versteht keine.
    crit: Option<serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Audience {
    One(String),
    Many(Vec<String>),
}

impl Audience {
    fn contains(&self, wanted: &str) -> bool {
        match self {
            Audience::One(a) => a == wanted,
            Audience::Many(all) => all.iter().any(|a| a == wanted),
        }
    }
}

/// Zeiten sind JSON-Zahlen (NumericDate), auch mit Nachkommastellen.
#[derive(Deserialize)]
struct Claims {
    iss: Option<String>,
    sub: Option<String>,
    aud: Option<Audience>,
    exp: Option<f64>,
    iat: Option<f64>,
    nbf: Option<f64>,
    client_id: Option<String>,
    #[serde(rename = "type")]
    token_type: Option<String>,
}

fn decode<T: serde::de::DeserializeOwned>(part: &str) -> Result<T, Reason> {
    let bytes = URL_SAFE_NO_PAD
        .decode(part)
        .map_err(|_| Reason::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| Reason::Malformed)
}

/// Asymmetrisch signiert: `none` und HMAC (geteiltes Geheimnis) sind ausgeschlossen.
fn algorithm(raw: serde_json::Value) -> Result<CoreJwsSigningAlgorithm, Reason> {
    use CoreJwsSigningAlgorithm::*;
    match serde_json::from_value(raw) {
        Ok(None | HmacSha256 | HmacSha384 | HmacSha512) | Err(_) => Err(Reason::Algorithm),
        Ok(alg) => Ok(alg),
    }
}

/// Der Schlüssel darf signieren und, falls er ein Verfahren nennt, genau dieses.
fn usable(key: &CoreJsonWebKey, alg: &CoreJwsSigningAlgorithm) -> bool {
    let sign = key
        .key_use()
        .is_none_or(|u| *u == CoreJsonWebKeyUse::Signature);
    let matches = match key.signing_alg() {
        JsonWebKeyAlgorithm::Algorithm(a) => a == alg,
        JsonWebKeyAlgorithm::Unspecified => true,
        JsonWebKeyAlgorithm::Unsupported => false,
    };
    sign && matches
}

/// Prüft ein Zugriffstoken vollständig gegen den JWKS. Rein: Zeit und Schlüssel kommen herein.
pub fn verify_access_token(
    jwt: &str,
    jwks: &CoreJsonWebKeySet,
    expected: &Expected,
) -> Result<AccessClaims, Reason> {
    let mut parts = jwt.split('.');
    let (Some(header_b64), Some(payload_b64), Some(signature_b64), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return Err(Reason::Malformed);
    };
    let header: Header = decode(header_b64)?;
    if header.crit.is_some() {
        return Err(Reason::Malformed);
    }
    let alg = algorithm(header.alg)?;
    let typ = header.typ.unwrap_or_default().to_ascii_lowercase();
    if typ != "at+jwt" && typ != "application/at+jwt" {
        return Err(Reason::Type);
    }

    let by_kid: Vec<&CoreJsonWebKey> = jwks
        .keys()
        .iter()
        .filter(|k| {
            header
                .kid
                .as_deref()
                .is_none_or(|kid| k.key_id().is_some_and(|id| id.as_str() == kid))
        })
        .collect();
    if by_kid.is_empty() {
        return Err(Reason::UnknownKey);
    }
    let keys: Vec<_> = by_kid.into_iter().filter(|k| usable(k, &alg)).collect();
    if keys.is_empty() {
        return Err(Reason::Algorithm);
    }
    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| Reason::Malformed)?;
    if signature.is_empty() {
        return Err(Reason::Signature);
    }
    let message = format!("{header_b64}.{payload_b64}");
    if !keys.iter().any(|k| {
        k.verify_signature(&alg, message.as_bytes(), &signature)
            .is_ok()
    }) {
        return Err(Reason::Signature);
    }

    let claims: Claims = decode(payload_b64)?;
    if claims.token_type.as_deref() == Some("id-token") {
        return Err(Reason::IdToken);
    }
    let issuer = claims.iss.ok_or(Reason::Issuer)?;
    if issuer.trim_end_matches('/') != expected.issuer.trim_end_matches('/') {
        return Err(Reason::Issuer);
    }
    let now = expected.now_s as f64;
    let skew = CLOCK_SKEW_S as f64;
    match claims.exp {
        Some(exp) if exp + skew > now => {}
        Some(_) => return Err(Reason::Expired),
        None => return Err(Reason::Malformed),
    }
    if [claims.iat, claims.nbf]
        .into_iter()
        .flatten()
        .any(|t| t > now + skew)
    {
        return Err(Reason::NotYetValid);
    }
    if !claims
        .aud
        .is_some_and(|aud| aud.contains(expected.audience))
    {
        return Err(Reason::Audience);
    }
    if claims
        .client_id
        .is_some_and(|client| client != expected.audience)
    {
        return Err(Reason::ClientId);
    }
    let subject = claims
        .sub
        .filter(|s| !s.is_empty())
        .ok_or(Reason::Malformed)?;
    Ok(AccessClaims { issuer, subject })
}

/// Prüft mit dem JWKS des Ausstellers. Kennt der JWKS den `kid` nicht (Schlüsseltausch), wird er
/// einmal neu geladen – höchstens alle 60 s. Ein abgelehntes Token ist 401, ein nicht erreichbarer
/// Anmeldedienst 503.
pub async fn verify(
    oidc: &OidcState,
    jwt: &str,
    expected: &Expected<'_>,
) -> Result<AccessClaims, AppError> {
    let jwks = oidc.jwks(expected.issuer, false).await?;
    let result = match verify_access_token(jwt, &jwks, expected) {
        Err(Reason::UnknownKey) => {
            let jwks = oidc.jwks(expected.issuer, true).await?;
            verify_access_token(jwt, &jwks, expected)
        }
        other => other,
    };
    result.map_err(|reason| {
        tracing::info!(?reason, "Zugriffstoken der Flugmappe abgelehnt");
        AppError::UnauthorizedWith(reason.message().into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::oidc::test_idp;
    use openidconnect::PrivateSigningKey;
    use serde_json::{Value, json};

    const ISSUER: &str = "https://id.example.org";
    const CLIENT: &str = "flugmappe";
    const NOW: i64 = 1_792_324_800;

    fn expected() -> Expected<'static> {
        Expected {
            issuer: ISSUER,
            audience: CLIENT,
            now_s: NOW,
        }
    }

    fn jwks() -> CoreJsonWebKeySet {
        CoreJsonWebKeySet::new(vec![test_idp::signing_key().as_verification_key()])
    }

    fn header() -> Value {
        json!({"alg": "RS256", "typ": "at+jwt", "kid": test_idp::KEY_ID})
    }

    /// Wie Pocket ID: `aud` als Array mit Client-ID und Aussteller, `client_id` gesetzt.
    fn claims() -> Value {
        json!({
            "iss": ISSUER, "sub": "sub-1", "aud": [CLIENT, ISSUER], "exp": NOW + 3600, "iat": NOW,
            "jti": "j-1", "scope": "openid profile", "scp": ["openid", "profile"], "client_id": CLIENT
        })
    }

    fn with(mut base: Value, changes: Value) -> Value {
        for (k, v) in changes.as_object().unwrap() {
            if v.is_null() {
                base.as_object_mut().unwrap().remove(k);
            } else {
                base[k] = v.clone();
            }
        }
        base
    }

    fn check(header: Value, claims: Value) -> Result<AccessClaims, Reason> {
        verify_access_token(&test_idp::sign(&header, &claims), &jwks(), &expected())
    }

    #[test]
    fn a_pocket_id_access_token_is_accepted() {
        assert_eq!(
            check(header(), claims()),
            Ok(AccessClaims {
                issuer: ISSUER.into(),
                subject: "sub-1".into()
            })
        );
        // Aussteller mit Schrägstrich, `aud` als String, ohne `client_id`, Typ als Medientyp.
        let relaxed = with(
            claims(),
            json!({"iss": format!("{ISSUER}/"), "aud": CLIENT, "client_id": null}),
        );
        let header = with(header(), json!({"typ": "application/AT+JWT"}));
        assert!(check(header, relaxed).is_ok());
    }

    #[test]
    fn expiry_and_issue_time_allow_a_minute_of_skew() {
        assert!(check(header(), with(claims(), json!({"exp": NOW - 30}))).is_ok());
        assert_eq!(
            check(header(), with(claims(), json!({"exp": NOW - 61}))),
            Err(Reason::Expired)
        );
        assert_eq!(
            check(header(), with(claims(), json!({"exp": null}))),
            Err(Reason::Malformed)
        );
        assert!(check(header(), with(claims(), json!({"iat": NOW + 30}))).is_ok());
        assert_eq!(
            check(header(), with(claims(), json!({"iat": NOW + 120}))),
            Err(Reason::NotYetValid)
        );
        assert_eq!(
            check(header(), with(claims(), json!({"nbf": NOW + 120}))),
            Err(Reason::NotYetValid)
        );
    }

    #[test]
    fn issuer_audience_and_client_must_match() {
        assert_eq!(
            check(
                header(),
                with(claims(), json!({"iss": "https://evil.example.org"}))
            ),
            Err(Reason::Issuer)
        );
        assert_eq!(
            check(
                header(),
                with(claims(), json!({"aud": ["suchlage", ISSUER]}))
            ),
            Err(Reason::Audience),
            "Token für einen anderen Client"
        );
        assert_eq!(
            check(header(), with(claims(), json!({"aud": null}))),
            Err(Reason::Audience)
        );
        assert_eq!(
            check(header(), with(claims(), json!({"client_id": "suchlage"}))),
            Err(Reason::ClientId)
        );
        assert_eq!(
            check(header(), with(claims(), json!({"sub": ""}))),
            Err(Reason::Malformed)
        );
    }

    #[test]
    fn id_tokens_and_plain_jwts_are_no_access_tokens() {
        let id_token_claims = with(
            claims(),
            json!({"type": "id-token", "client_id": null, "aud": CLIENT}),
        );
        assert_eq!(
            check(
                with(header(), json!({"typ": "JWT"})),
                id_token_claims.clone()
            ),
            Err(Reason::Type)
        );
        assert_eq!(
            check(header(), id_token_claims),
            Err(Reason::IdToken),
            "auch mit passendem typ"
        );
        assert_eq!(
            check(with(header(), json!({"typ": null})), claims()),
            Err(Reason::Type)
        );
    }

    #[test]
    fn unsigned_and_shared_secret_tokens_are_rejected() {
        let unsigned = format!(
            "{}.{}.",
            URL_SAFE_NO_PAD.encode(json!({"alg": "none", "typ": "at+jwt"}).to_string()),
            URL_SAFE_NO_PAD.encode(claims().to_string())
        );
        assert_eq!(
            verify_access_token(&unsigned, &jwks(), &expected()),
            Err(Reason::Algorithm)
        );
        for alg in ["HS256", "HS512", "XY999"] {
            let token = test_idp::sign(&with(header(), json!({ "alg": alg })), &claims());
            assert_eq!(
                verify_access_token(&token, &jwks(), &expected()),
                Err(Reason::Algorithm),
                "{alg}"
            );
        }
        // Der Schlüssel ist RSA: ES256 passt nicht zu ihm.
        assert!(check(with(header(), json!({"alg": "ES256"})), claims()).is_err());
    }

    #[test]
    fn unknown_key_and_broken_signature_are_told_apart() {
        assert_eq!(
            check(with(header(), json!({"kid": "anderer"})), claims()),
            Err(Reason::UnknownKey)
        );
        assert!(
            check(with(header(), json!({"kid": null})), claims()).is_ok(),
            "ohne kid zählt jeder Schlüssel"
        );
        assert_eq!(
            verify_access_token(
                &test_idp::sign(&header(), &claims()),
                &CoreJsonWebKeySet::new(vec![]),
                &expected()
            ),
            Err(Reason::UnknownKey)
        );

        let token = test_idp::sign(&header(), &claims());
        let (head, sig) = token.rsplit_once('.').unwrap();
        let mut bytes = URL_SAFE_NO_PAD.decode(sig).unwrap();
        bytes[10] ^= 0x01;
        let broken = format!("{head}.{}", URL_SAFE_NO_PAD.encode(&bytes));
        assert_eq!(
            verify_access_token(&broken, &jwks(), &expected()),
            Err(Reason::Signature)
        );
        // Andere Nutzlast unter der alten Signatur.
        let (h, rest) = token.split_once('.').unwrap();
        let (_, sig) = rest.split_once('.').unwrap();
        let forged = format!(
            "{h}.{}.{sig}",
            URL_SAFE_NO_PAD.encode(with(claims(), json!({"sub": "admin"})).to_string())
        );
        assert_eq!(
            verify_access_token(&forged, &jwks(), &expected()),
            Err(Reason::Signature)
        );
    }

    #[test]
    fn garbage_is_malformed() {
        for bad in [
            "",
            "abc",
            "a.b",
            "a.b.c.d",
            "!!.??.##",
            &format!("{}.e30.c2ln", URL_SAFE_NO_PAD.encode("kein json")),
        ] {
            assert_eq!(
                verify_access_token(bad, &jwks(), &expected()),
                Err(Reason::Malformed),
                "{bad}"
            );
        }
        let critical = with(header(), json!({"crit": ["exp"]}));
        assert_eq!(check(critical, claims()), Err(Reason::Malformed));
    }

    #[test]
    fn a_key_for_encryption_or_another_algorithm_does_not_verify() {
        let key = test_idp::signing_key().as_verification_key();
        let mut json = serde_json::to_value(&key).unwrap();
        json["use"] = json!("enc");
        let enc: CoreJsonWebKey = serde_json::from_value(json.clone()).unwrap();
        let token = test_idp::sign(&header(), &claims());
        assert_eq!(
            verify_access_token(&token, &CoreJsonWebKeySet::new(vec![enc]), &expected()),
            Err(Reason::Algorithm)
        );
        json["use"] = json!("sig");
        json["alg"] = json!("RS512");
        let rs512: CoreJsonWebKey = serde_json::from_value(json).unwrap();
        assert_eq!(
            verify_access_token(&token, &CoreJsonWebKeySet::new(vec![rs512]), &expected()),
            Err(Reason::Algorithm)
        );
        // Mit passendem Verfahren signiert, gilt derselbe Schlüssel.
        let rs512_token = {
            let header = with(header(), json!({"alg": "RS512"}));
            let input = format!(
                "{}.{}",
                URL_SAFE_NO_PAD.encode(header.to_string()),
                URL_SAFE_NO_PAD.encode(claims().to_string())
            );
            let sig = test_idp::signing_key()
                .sign(
                    &CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha512,
                    input.as_bytes(),
                )
                .unwrap();
            format!("{input}.{}", URL_SAFE_NO_PAD.encode(sig))
        };
        assert!(verify_access_token(&rs512_token, &jwks(), &expected()).is_ok());
    }

    #[tokio::test]
    async fn verify_reloads_the_jwks_for_an_unknown_key_at_most_every_minute() {
        let idp = test_idp::start("suchlage").await;
        let token = idp.access_token_for(test_idp::SUBJECT, CLIENT, 300);
        let expected = Expected {
            issuer: &idp.issuer,
            audience: CLIENT,
            now_s: chrono::Utc::now().timestamp(),
        };
        let oidc = OidcState::new();
        assert_eq!(
            verify(&oidc, &token, &expected).await.unwrap().subject,
            test_idp::SUBJECT
        );
        assert_eq!(idp.jwks_count(), 1);
        verify(&oidc, &token, &expected).await.unwrap();
        assert_eq!(idp.jwks_count(), 1, "aus dem Cache");

        let rotated = idp.access_token(test_idp::AccessToken {
            kid: "neu".into(),
            ..test_idp::AccessToken::new(test_idp::SUBJECT, CLIENT)
        });
        let err = verify(&oidc, &rotated, &expected).await.unwrap_err();
        assert!(matches!(err, AppError::UnauthorizedWith(_)), "{err:?}");
        assert_eq!(
            idp.jwks_count(),
            1,
            "innerhalb einer Minute nicht neu geladen"
        );

        let eager = OidcState::new().with_jwks_min_refresh(std::time::Duration::ZERO);
        verify(&eager, &token, &expected).await.unwrap();
        verify(&eager, &rotated, &expected).await.unwrap_err();
        assert_eq!(idp.jwks_count(), 3, "einmal geladen, einmal neu");

        let unreachable = Expected {
            issuer: "http://127.0.0.1:1",
            ..expected
        };
        let err = verify(&oidc, &token, &unreachable).await.unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)), "{err:?}");
    }
}

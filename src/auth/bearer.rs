//! Zugriffstokens der Flugmappe (SUL-66): JWT nach RFC 9068 von Pocket ID, lokal geprüft gegen den
//! JWKS des in den OIDC-Einstellungen stehenden Ausstellers. Kein Rückfall über Userinfo.

use std::sync::Arc;
use std::time::Instant;

use axum::extract::{FromRef, FromRequestParts};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use openidconnect::core::{
    CoreJsonWebKey, CoreJsonWebKeySet, CoreJsonWebKeyUse, CoreJwsSigningAlgorithm,
};
use openidconnect::{JsonWebKey, JsonWebKeyAlgorithm};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::auth::oidc::OidcState;
use crate::error::AppError;
use crate::settings::SettingsHandle;
use crate::store::users::{self, User};
use crate::time::now_ms;

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

/// Ein Token, das ohne Schlüssel geprüft ist: Form, Kopf und Claims passen. Nichts davon gilt als
/// vertraut, bevor `verify_signature` die Signatur bestätigt hat.
#[derive(Debug)]
pub struct Unverified<'a> {
    /// `Kopf.Nutzlast`, wie signiert.
    message: &'a str,
    signature: Vec<u8>,
    alg: CoreJwsSigningAlgorithm,
    kid: Option<String>,
    claims: AccessClaims,
}

/// Erster Schritt, ohne Schlüssel: Zerlegen, Kopf (`crit`, `alg`, `typ`), Nutzlast, `type`, `iss`,
/// `exp`/`iat`/`nbf`, `aud`, `client_id`, `sub`. Nur was hier besteht, darf einen Abruf des JWKS
/// auslösen; kaputte, abgelaufene oder fremde Tokens sind 401, auch wenn der Anbieter fehlt.
pub fn precheck<'a>(jwt: &'a str, expected: &Expected) -> Result<Unverified<'a>, Reason> {
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

    let signature = URL_SAFE_NO_PAD
        .decode(signature_b64)
        .map_err(|_| Reason::Malformed)?;
    if signature.is_empty() {
        return Err(Reason::Signature);
    }
    Ok(Unverified {
        message: &jwt[..header_b64.len() + 1 + payload_b64.len()],
        signature,
        alg,
        kid: header.kid,
        claims: AccessClaims { issuer, subject },
    })
}

impl Unverified<'_> {
    /// Zweiter Schritt: Signatur gegen den JWKS (Schlüssel über `kid`). Erst danach gelten die
    /// Claims.
    pub fn verify_signature(&self, jwks: &CoreJsonWebKeySet) -> Result<AccessClaims, Reason> {
        let by_kid: Vec<&CoreJsonWebKey> = jwks
            .keys()
            .iter()
            .filter(|k| {
                self.kid
                    .as_deref()
                    .is_none_or(|kid| k.key_id().is_some_and(|id| id.as_str() == kid))
            })
            .collect();
        if by_kid.is_empty() {
            return Err(Reason::UnknownKey);
        }
        let keys: Vec<_> = by_kid
            .into_iter()
            .filter(|k| usable(k, &self.alg))
            .collect();
        if keys.is_empty() {
            return Err(Reason::Algorithm);
        }
        if !keys.iter().any(|k| {
            k.verify_signature(&self.alg, self.message.as_bytes(), &self.signature)
                .is_ok()
        }) {
            return Err(Reason::Signature);
        }
        Ok(self.claims.clone())
    }
}

/// Prüft ein Zugriffstoken vollständig gegen den JWKS. Rein: Zeit und Schlüssel kommen herein.
pub fn verify_access_token(
    jwt: &str,
    jwks: &CoreJsonWebKeySet,
    expected: &Expected,
) -> Result<AccessClaims, Reason> {
    precheck(jwt, expected)?.verify_signature(jwks)
}

fn rejected(reason: Reason) -> AppError {
    tracing::info!(?reason, "Zugriffstoken der Flugmappe abgelehnt");
    AppError::UnauthorizedWith(reason.message().into())
}

/// Prüft erst ohne, dann mit dem JWKS des Ausstellers. Kennt der JWKS den `kid` nicht
/// (Schlüsseltausch), wird er einmal neu geladen – höchstens alle 60 s. Ein abgelehntes Token ist
/// 401; 503 nur, wenn für ein sonst gültiges Token nie Schlüssel geladen werden konnten. `now`
/// steuert den Cache (Tests stellen die Zeit vor).
pub async fn verify(
    oidc: &OidcState,
    jwt: &str,
    expected: &Expected<'_>,
    now: Instant,
) -> Result<AccessClaims, AppError> {
    let token = precheck(jwt, expected).map_err(rejected)?;
    let jwks = oidc.jwks(expected.issuer, false, now).await?;
    let result = match token.verify_signature(&jwks) {
        Err(Reason::UnknownKey) => {
            token.verify_signature(&oidc.jwks(expected.issuer, true, now).await?)
        }
        other => other,
    };
    result.map_err(rejected)
}

/// Konto zu einem geprüften Token. Die einzige Stelle, an der die Schnittstelle der Flugmappe das
/// Konto nachschlägt; ein gesperrtes Konto (SUL-52) findet `find_by_oidc` nicht. Konten entstehen
/// hier nie.
pub async fn account(pool: &SqlitePool, claims: &AccessClaims) -> Result<User, AppError> {
    // Alle Konten haben `operator` oder `admin`; beide dürfen die Schnittstelle nutzen.
    users::find_by_oidc(pool, &claims.issuer, &claims.subject)
        .await?
        .ok_or(AppError::Forbidden)
}

/// `Authorization: Bearer <Token>`, Schema ohne Rücksicht auf Groß- und Kleinschreibung.
fn bearer_token(parts: &Parts) -> Option<&str> {
    let value = parts.headers.get(AUTHORIZATION)?.to_str().ok()?;
    let (scheme, token) = value.split_once(' ')?;
    let token = token.trim();
    (scheme.eq_ignore_ascii_case("bearer") && !token.is_empty()).then_some(token)
}

/// Konto hinter dem Zugriffstoken der Flugmappe. Schnittstelle aus → 503 (vor allem anderen),
/// Token fehlt oder ungültig → 401, kein Konto in Suchlage → 403.
pub struct FlugmappeUser(pub User);

impl<S: Send + Sync> FromRequestParts<S> for FlugmappeUser
where
    SqlitePool: FromRef<S>,
    SettingsHandle: FromRef<S>,
    Arc<OidcState>: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        let settings = SettingsHandle::from_ref(state).current();
        let flugmappe = settings.flugmappe.ok_or_else(|| {
            AppError::Unavailable("Die Schnittstelle für die Flugmappe ist aus".into())
        })?;
        let oidc = settings.oidc.ok_or_else(|| {
            AppError::Unavailable("Die Anmeldung über Pocket ID ist nicht eingerichtet".into())
        })?;
        let token = bearer_token(parts).ok_or(AppError::Unauthorized)?;
        let expected = Expected {
            issuer: &oidc.issuer,
            audience: &flugmappe.client_id,
            now_s: now_ms().div_euclid(1000),
        };
        let claims = verify(
            &Arc::<OidcState>::from_ref(state),
            token,
            &expected,
            Instant::now(),
        )
        .await?;
        Ok(FlugmappeUser(
            account(&SqlitePool::from_ref(state), &claims).await?,
        ))
    }
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

    fn later(t0: Instant, s: u64) -> Instant {
        t0 + std::time::Duration::from_secs(s)
    }

    /// Test-IdP mit einem gültigen Token und einem mit unbekanntem `kid` (Schlüsseltausch).
    async fn live() -> (test_idp::TestIdp, String, String) {
        let idp = test_idp::start("suchlage").await;
        let token = idp.access_token_for(test_idp::SUBJECT, CLIENT, 300);
        let rotated = idp.access_token(test_idp::AccessToken {
            kid: "neu".into(),
            ..test_idp::AccessToken::new(test_idp::SUBJECT, CLIENT)
        });
        (idp, token, rotated)
    }

    fn live_expected(issuer: &str) -> Expected<'_> {
        Expected {
            issuer,
            audience: CLIENT,
            now_s: chrono::Utc::now().timestamp(),
        }
    }

    fn is_401(r: Result<AccessClaims, AppError>) -> bool {
        matches!(r, Err(AppError::UnauthorizedWith(_)))
    }

    #[tokio::test]
    async fn verify_reloads_the_jwks_for_an_unknown_key_at_most_every_minute() {
        let (idp, token, rotated) = live().await;
        let expected = live_expected(&idp.issuer);
        let oidc = OidcState::new();
        let t0 = Instant::now();
        assert_eq!(
            verify(&oidc, &token, &expected, t0).await.unwrap().subject,
            test_idp::SUBJECT
        );
        assert_eq!(idp.jwks_count(), 1, "mit der Discovery");
        verify(&oidc, &token, &expected, later(t0, 1))
            .await
            .unwrap();
        assert_eq!(idp.jwks_count(), 1, "aus dem Cache");

        assert!(is_401(
            verify(&oidc, &rotated, &expected, later(t0, 30)).await
        ));
        assert_eq!(
            idp.jwks_count(),
            1,
            "innerhalb einer Minute nicht neu geladen"
        );
        assert!(is_401(
            verify(&oidc, &rotated, &expected, later(t0, 61)).await
        ));
        assert_eq!(idp.jwks_count(), 2, "danach direkt von jwks_uri");

        let eager = OidcState::new().with_min_refresh(std::time::Duration::ZERO);
        verify(&eager, &token, &expected, t0).await.unwrap();
        assert!(is_401(verify(&eager, &rotated, &expected, t0).await));
        assert_eq!(idp.jwks_count(), 4, "einmal geladen, einmal neu");
    }

    #[tokio::test]
    async fn a_failed_refresh_is_throttled_and_the_old_keys_stay() {
        let (idp, token, rotated) = live().await;
        let expected = live_expected(&idp.issuer);
        let oidc = OidcState::new();
        let t0 = Instant::now();
        verify(&oidc, &token, &expected, t0).await.unwrap();
        idp.set_down(true);

        assert!(is_401(
            verify(&oidc, &rotated, &expected, later(t0, 61)).await
        ));
        assert_eq!(idp.jwks_count(), 2, "ein Versuch, gescheitert");
        verify(&oidc, &token, &expected, later(t0, 62))
            .await
            .unwrap();
        assert!(is_401(
            verify(&oidc, &rotated, &expected, later(t0, 120)).await
        ));
        assert_eq!(
            idp.jwks_count(),
            2,
            "kein zweiter Abruf innerhalb von 60 s nach dem Fehlschlag"
        );
        assert!(is_401(
            verify(&oidc, &rotated, &expected, later(t0, 121)).await
        ));
        assert_eq!(idp.jwks_count(), 3);
    }

    #[tokio::test]
    async fn an_expired_cache_outlives_an_unreachable_idp() {
        let (idp, token, _) = live().await;
        let expected = live_expected(&idp.issuer);
        let oidc = OidcState::new();
        let t0 = Instant::now();
        verify(&oidc, &token, &expected, t0).await.unwrap();
        idp.set_down(true);

        verify(&oidc, &token, &expected, later(t0, 3_601))
            .await
            .expect("alte Schlüssel gelten weiter");
        assert_eq!(idp.discovery_count(), 2, "abgelaufen: neu versucht");
        verify(&oidc, &token, &expected, later(t0, 3_630))
            .await
            .unwrap();
        assert_eq!(idp.discovery_count(), 2, "gedrosselt");

        idp.set_down(false);
        verify(&oidc, &token, &expected, later(t0, 3_662))
            .await
            .unwrap();
        assert_eq!(
            (idp.discovery_count(), idp.jwks_count()),
            (3, 2),
            "wieder frisch geladen"
        );
    }

    #[tokio::test]
    async fn without_any_keys_a_valid_token_is_503_and_retried_at_most_every_minute() {
        let (idp, token, _) = live().await;
        idp.set_down(true);
        let expected = live_expected(&idp.issuer);
        let oidc = OidcState::new();
        let t0 = Instant::now();
        for s in [0, 30] {
            let err = verify(&oidc, &token, &expected, later(t0, s))
                .await
                .unwrap_err();
            assert!(matches!(err, AppError::Unavailable(_)), "{err:?}");
        }
        assert_eq!(idp.discovery_count(), 1, "Discovery-Fehler gedrosselt");
        verify(&oidc, &token, &expected, later(t0, 61))
            .await
            .unwrap_err();
        assert_eq!(idp.discovery_count(), 2);
        idp.set_down(false);
        verify(&oidc, &token, &expected, later(t0, 122))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn broken_or_misaddressed_tokens_are_401_without_asking_the_idp() {
        // Port 1 auf Loopback ist zu; ein Abruf schlüge mit 503 fehl.
        let issuer = "http://127.0.0.1:1";
        let expected = Expected {
            issuer,
            audience: CLIENT,
            now_s: NOW,
        };
        let for_issuer = |changes: Value| {
            test_idp::sign(
                &header(),
                &with(claims(), with(json!({"iss": issuer}), changes)),
            )
        };
        let oidc = OidcState::new();
        for bad in [
            "kaputt".to_string(),
            "a.b.c".into(),
            for_issuer(json!({"exp": NOW - 120})),
            for_issuer(json!({"aud": ["suchlage"]})),
            for_issuer(json!({"iss": "https://evil.example.org"})),
            test_idp::sign(&with(header(), json!({"typ": "JWT"})), &claims()),
        ] {
            let result = verify(&oidc, &bad, &expected, Instant::now()).await;
            assert!(is_401(result), "{bad}");
        }
        let err = verify(&oidc, &for_issuer(json!({})), &expected, Instant::now())
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn cache_hits_do_not_wait_for_a_slow_refresh() {
        let (idp, token, rotated) = live().await;
        let oidc = Arc::new(OidcState::new());
        let t0 = Instant::now();
        verify(&oidc, &token, &live_expected(&idp.issuer), t0)
            .await
            .unwrap();
        idp.set_jwks_delay(std::time::Duration::from_secs(5));
        let slow = tokio::spawn({
            let (oidc, issuer) = (oidc.clone(), idp.issuer.clone());
            async move {
                verify(&oidc, &rotated, &live_expected(&issuer), later(t0, 61))
                    .await
                    .is_ok()
            }
        });
        while idp.jwks_count() < 2 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        let quick = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            verify(&oidc, &token, &live_expected(&idp.issuer), later(t0, 62)),
        )
        .await;
        assert!(matches!(quick, Ok(Ok(_))), "{quick:?}");
        slow.abort();
    }

    #[tokio::test]
    async fn the_first_load_is_shared_by_concurrent_requests() {
        let (idp, token, _) = live().await;
        idp.set_jwks_delay(std::time::Duration::from_millis(200));
        let oidc = OidcState::new();
        let expected = live_expected(&idp.issuer);
        let t0 = Instant::now();
        let (a, b) = tokio::join!(
            verify(&oidc, &token, &expected, t0),
            verify(&oidc, &token, &expected, t0)
        );
        assert!(a.is_ok() && b.is_ok(), "{a:?} {b:?}");
        assert_eq!(idp.jwks_count(), 1);
    }
}

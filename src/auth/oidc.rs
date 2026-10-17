//! OIDC (Authorization Code + PKCE, State, Nonce) gegen Pocket ID. Muster aus Lifeline
//! `auth/oidc`, aber mit Einstellungen aus der DB statt Env und ohne OnceLock-Globals.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use openidconnect::core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, EndpointMaybeSet, EndpointNotSet,
    EndpointSet, HttpRequest, HttpResponse, IssuerUrl, Nonce, PkceCodeChallenge, PkceCodeVerifier,
    RedirectUrl, Scope, TokenResponse,
};

use crate::error::AppError;
use crate::settings::{OidcSettings, issuer_allowed};

pub const STATE_COOKIE: &str = "suchlage_oidc";
/// So lange gilt eine begonnene Anmeldung, und so lange lebt das Bindungscookie.
pub const PENDING_TTL: Duration = Duration::from_secs(600);
const PENDING_MAX: usize = 200;
const DISCOVERY_TTL: Duration = Duration::from_secs(3600);

type Client = CoreClient<
    EndpointSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointNotSet,
    EndpointMaybeSet,
    EndpointMaybeSet,
>;

#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub issuer: String,
    pub subject: String,
    pub display_name: String,
}

struct Pending {
    nonce: String,
    verifier: String,
    next: String,
    created: Instant,
}

pub struct OidcState {
    pending: Mutex<HashMap<String, Pending>>,
    discovery: tokio::sync::Mutex<Option<(String, CoreProviderMetadata, Instant)>>,
    https: Http,
    loopback: Http,
}

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct HttpError(String);

/// reqwest als `AsyncHttpClient`: keine Redirects (SSRF), Zeitgrenzen.
struct Http(reqwest::Client);

impl<'c> openidconnect::AsyncHttpClient<'c> for Http {
    type Error = HttpError;
    type Future = BoxFuture<'c, Result<HttpResponse, HttpError>>;

    fn call(&'c self, request: HttpRequest) -> Self::Future {
        Box::pin(async move {
            let (parts, body) = request.into_parts();
            let res = self
                .0
                .request(parts.method, parts.uri.to_string())
                .headers(parts.headers)
                .body(body)
                .send()
                .await
                .map_err(|e| HttpError(e.to_string()))?;
            let status = res.status();
            let headers = res.headers().clone();
            let bytes = res.bytes().await.map_err(|e| HttpError(e.to_string()))?;
            let mut builder = openidconnect::http::Response::builder().status(status);
            if let Some(h) = builder.headers_mut() {
                *h = headers;
            }
            builder
                .body(bytes.to_vec())
                .map_err(|e| HttpError(e.to_string()))
        })
    }
}

impl Http {
    fn new(https_only: bool) -> Self {
        Http(
            reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(15))
                .connect_timeout(Duration::from_secs(8))
                .https_only(https_only)
                .build()
                .expect("HTTP-Client baubar"),
        )
    }
}

fn unavailable() -> AppError {
    AppError::Unavailable("Anmeldedienst nicht erreichbar".into())
}

/// Nur lokale Pfade als Rücksprungziel, nie `//host` oder absolute URLs.
pub fn safe_next(next: &str) -> String {
    // Browser entfernen Tab und Zeilenumbruch aus URLs: `/\t/evil` würde sonst zu `//evil`.
    let plain = next
        .chars()
        .all(|c| !c.is_control() && !c.is_whitespace() && c != '\\');
    if plain && next.starts_with('/') && !next.starts_with("//") {
        next.to_string()
    } else {
        "/".into()
    }
}

impl Default for OidcState {
    fn default() -> Self {
        Self::new()
    }
}

impl OidcState {
    pub fn new() -> Self {
        Self {
            pending: Mutex::new(HashMap::new()),
            discovery: tokio::sync::Mutex::new(None),
            https: Http::new(true),
            loopback: Http::new(false),
        }
    }

    /// http erlaubt `issuer_allowed` nur für Loopback; alles andere geht nur über https.
    fn http(&self, issuer: &str) -> &Http {
        if url::Url::parse(issuer).is_ok_and(|u| u.scheme() == "http") {
            &self.loopback
        } else {
            &self.https
        }
    }

    /// Discovery mit Cache je Issuer. Die Sperre hält über den Abruf, damit gleichzeitige
    /// Anmeldungen den Anbieter nur einmal fragen.
    async fn metadata(&self, issuer: &str) -> Result<CoreProviderMetadata, AppError> {
        if !issuer_allowed(issuer) {
            return Err(AppError::Validation(
                "Issuer muss mit https:// beginnen (http nur für localhost)".into(),
            ));
        }
        let mut cache = self.discovery.lock().await;
        if let Some((cached_issuer, meta, at)) = cache.as_ref()
            && cached_issuer == issuer
            && at.elapsed() < DISCOVERY_TTL
        {
            return Ok(meta.clone());
        }
        let url = IssuerUrl::new(issuer.to_string())
            .map_err(|_| AppError::Validation("Issuer ist keine gültige URL".into()))?;
        let meta = CoreProviderMetadata::discover_async(url, self.http(issuer))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, issuer, "OIDC-Discovery fehlgeschlagen");
                AppError::Unavailable("Anmeldedienst nicht erreichbar oder Issuer falsch".into())
            })?;
        *cache = Some((issuer.to_string(), meta.clone(), Instant::now()));
        Ok(meta)
    }

    pub async fn check_discovery(&self, issuer: &str) -> Result<(), AppError> {
        self.metadata(issuer.trim_end_matches('/'))
            .await
            .map(|_| ())
    }

    async fn client(&self, cfg: &OidcSettings, redirect_url: &str) -> Result<Client, AppError> {
        let meta = self.metadata(&cfg.issuer).await?;
        let redirect = RedirectUrl::new(redirect_url.to_string())
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(CoreClient::from_provider_metadata(
            meta,
            ClientId::new(cfg.client_id.clone()),
            Some(ClientSecret::new(cfg.client_secret.clone())),
        )
        .set_redirect_uri(redirect))
    }

    /// Autorisierungs-URL des Anbieters und das CSRF-State für das Bindungscookie.
    pub async fn begin(
        &self,
        cfg: &OidcSettings,
        redirect_url: &str,
        next: String,
    ) -> Result<(url::Url, String), AppError> {
        let client = self.client(cfg, redirect_url).await?;
        let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
        let (url, csrf, nonce) = client
            .authorize_url(
                CoreAuthenticationFlow::AuthorizationCode,
                CsrfToken::new_random,
                Nonce::new_random,
            )
            .add_scope(Scope::new("profile".into()))
            .set_pkce_challenge(challenge)
            .url();
        let mut pending = self.pending.lock().unwrap();
        pending.retain(|_, p| p.created.elapsed() < PENDING_TTL);
        if pending.len() >= PENDING_MAX {
            return Err(AppError::Unavailable(
                "Zu viele offene Anmeldungen – bitte gleich erneut versuchen".into(),
            ));
        }
        pending.insert(
            csrf.secret().clone(),
            Pending {
                nonce: nonce.secret().clone(),
                verifier: verifier.secret().clone(),
                next: safe_next(&next),
                created: Instant::now(),
            },
        );
        Ok((url, csrf.secret().clone()))
    }

    /// Löst den Code ein und prüft den ID-Token. Ein State gilt genau einmal; der zweite Wert
    /// ist das Rücksprungziel aus `begin`.
    pub async fn finish(
        &self,
        cfg: &OidcSettings,
        redirect_url: &str,
        state: &str,
        code: &str,
    ) -> Result<(Identity, String), AppError> {
        let pending = {
            let mut map = self.pending.lock().unwrap();
            map.remove(state)
                .filter(|p| p.created.elapsed() < PENDING_TTL)
        }
        .ok_or(AppError::Unauthorized)?;
        let client = self.client(cfg, redirect_url).await?;
        let token = client
            .exchange_code(AuthorizationCode::new(code.to_string()))
            .map_err(|_| unavailable())?
            .set_pkce_verifier(PkceCodeVerifier::new(pending.verifier))
            .request_async(self.http(&cfg.issuer))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, "OIDC-Token-Austausch fehlgeschlagen");
                unavailable()
            })?;
        let id_token = token.id_token().ok_or(AppError::Unauthorized)?;
        let claims = id_token
            .claims(&client.id_token_verifier(), &Nonce::new(pending.nonce))
            .map_err(|e| {
                tracing::warn!(error = %e, "ID-Token abgelehnt");
                AppError::Unauthorized
            })?;
        let display_name = claims
            .preferred_username()
            .map(|u| u.to_string())
            .or_else(|| {
                claims
                    .name()
                    .and_then(|n| n.get(None))
                    .map(|n| n.to_string())
            })
            .or_else(|| claims.email().map(|e| e.to_string()))
            .unwrap_or_else(|| claims.subject().to_string());
        Ok((
            Identity {
                issuer: claims.issuer().to_string(),
                subject: claims.subject().to_string(),
                display_name,
            },
            pending.next,
        ))
    }
}

#[cfg(test)]
pub mod test_idp {
    //! Minimaler OIDC-Anbieter für Tests: Discovery, JWKS, Token-Endpunkt mit RS256-ID-Token.
    //! Er zählt Discovery-Abrufe und merkt sich jede Token-Anfrage, damit Tests PKCE und
    //! Redirect-URL prüfen können.

    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::extract::{Form, State};
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use openidconnect::core::{
        CoreIdToken, CoreIdTokenClaims, CoreJsonWebKeySet, CoreJwsSigningAlgorithm,
        CoreRsaPrivateSigningKey,
    };
    use openidconnect::{
        Audience, EmptyAdditionalClaims, EndUserUsername, IssuerUrl, JsonWebKeyId, Nonce,
        PrivateSigningKey, StandardClaims, SubjectIdentifier,
    };

    /// Erfundenes Konto des Test-IdP.
    pub const SUBJECT: &str = "sub-1";
    pub const USERNAME: &str = "testpilot";

    #[derive(Clone)]
    pub struct TestIdp {
        pub issuer: String,
        /// Nonce, die der nächste ID-Token trägt. Der Test übernimmt sie aus der Autorisierungs-URL.
        pub nonce: Arc<Mutex<String>>,
        discovery_requests: Arc<AtomicUsize>,
        token_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
        client_id: &'static str,
        key: Arc<CoreRsaPrivateSigningKey>,
    }

    impl TestIdp {
        /// Formularfelder der letzten Anfrage an den Token-Endpunkt.
        pub fn last_token_request(&self) -> HashMap<String, String> {
            self.token_requests
                .lock()
                .unwrap()
                .last()
                .cloned()
                .expect("Token-Endpunkt wurde aufgerufen")
        }

        pub fn discovery_count(&self) -> usize {
            self.discovery_requests.load(Ordering::SeqCst)
        }

        pub fn token_request_count(&self) -> usize {
            self.token_requests.lock().unwrap().len()
        }
    }

    pub async fn start(client_id: &'static str) -> TestIdp {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let pem = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/oidc/test-rsa.pem"
        ))
        .unwrap();
        let key = CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new("test".into())))
            .unwrap();
        let idp = TestIdp {
            issuer,
            nonce: Arc::new(Mutex::new(String::new())),
            discovery_requests: Arc::new(AtomicUsize::new(0)),
            token_requests: Arc::new(Mutex::new(Vec::new())),
            client_id,
            key: Arc::new(key),
        };
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(idp.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        idp
    }

    async fn discovery(State(idp): State<TestIdp>) -> Json<serde_json::Value> {
        idp.discovery_requests.fetch_add(1, Ordering::SeqCst);
        let i = &idp.issuer;
        Json(serde_json::json!({
            "issuer": i, "authorization_endpoint": format!("{i}/authorize"), "token_endpoint": format!("{i}/token"),
            "jwks_uri": format!("{i}/jwks"), "response_types_supported": ["code"], "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        }))
    }

    async fn jwks(State(idp): State<TestIdp>) -> Json<CoreJsonWebKeySet> {
        Json(CoreJsonWebKeySet::new(vec![idp.key.as_verification_key()]))
    }

    async fn token(
        State(idp): State<TestIdp>,
        Form(form): Form<HashMap<String, String>>,
    ) -> Json<serde_json::Value> {
        idp.token_requests.lock().unwrap().push(form);
        let now = chrono::Utc::now();
        let claims = CoreIdTokenClaims::new(
            IssuerUrl::new(idp.issuer.clone()).unwrap(),
            vec![Audience::new(idp.client_id.into())],
            now + chrono::Duration::minutes(5),
            now,
            StandardClaims::new(SubjectIdentifier::new(SUBJECT.into()))
                .set_preferred_username(Some(EndUserUsername::new(USERNAME.into()))),
            EmptyAdditionalClaims {},
        )
        .set_nonce(Some(Nonce::new(idp.nonce.lock().unwrap().clone())));
        let id_token = CoreIdToken::new(
            claims,
            idp.key.as_ref(),
            CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
            None,
            None,
        )
        .unwrap();
        Json(
            serde_json::json!({ "access_token": "at", "token_type": "Bearer", "expires_in": 300, "id_token": id_token.to_string() }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REDIRECT: &str = "http://127.0.0.1:8080/api/auth/oidc/callback";

    fn cfg(issuer: &str) -> OidcSettings {
        OidcSettings {
            issuer: issuer.into(),
            client_id: "suchlage".into(),
            client_secret: "secret".into(),
        }
    }

    #[tokio::test]
    async fn code_flow_with_pkce_and_nonce_yields_identity() {
        let idp = test_idp::start("suchlage").await;
        let oidc = OidcState::new();
        let (url, state) = oidc
            .begin(&cfg(&idp.issuer), REDIRECT, "/flights".into())
            .await
            .unwrap();
        let q: HashMap<_, _> = url.query_pairs().into_owned().collect();
        assert_eq!(q["state"], state);
        assert_eq!(q["code_challenge_method"], "S256");
        assert_eq!(q["redirect_uri"], REDIRECT);
        assert!(q["scope"].split(' ').any(|s| s == "openid"), "{q:?}");
        *idp.nonce.lock().unwrap() = q["nonce"].clone();

        let (identity, next) = oidc
            .finish(&cfg(&idp.issuer), REDIRECT, &state, "code-1")
            .await
            .unwrap();
        assert_eq!(
            identity,
            Identity {
                issuer: idp.issuer.clone(),
                subject: test_idp::SUBJECT.into(),
                display_name: test_idp::USERNAME.into(),
            }
        );
        assert_eq!(next, "/flights");

        let form = idp.last_token_request();
        assert_eq!(form["grant_type"], "authorization_code");
        assert_eq!(form["code"], "code-1");
        assert_eq!(form["redirect_uri"], REDIRECT);
        let verifier = PkceCodeVerifier::new(form["code_verifier"].clone());
        assert_eq!(
            PkceCodeChallenge::from_code_verifier_sha256(&verifier).as_str(),
            q["code_challenge"],
            "der Verifier gehört zur Challenge der Autorisierungs-URL"
        );

        assert!(
            oidc.finish(&cfg(&idp.issuer), REDIRECT, &state, "code-1")
                .await
                .is_err(),
            "State nur einmal"
        );
    }

    #[tokio::test]
    async fn wrong_nonce_is_rejected() {
        let idp = test_idp::start("suchlage").await;
        let oidc = OidcState::new();
        let (_, state) = oidc
            .begin(&cfg(&idp.issuer), REDIRECT, "/".into())
            .await
            .unwrap();
        *idp.nonce.lock().unwrap() = "fremd".into();
        let err = oidc
            .finish(&cfg(&idp.issuer), REDIRECT, &state, "c")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized), "{err:?}");
    }

    #[tokio::test]
    async fn unknown_state_is_rejected_without_calling_the_idp() {
        let idp = test_idp::start("suchlage").await;
        let oidc = OidcState::new();
        let err = oidc
            .finish(&cfg(&idp.issuer), REDIRECT, "erfunden", "c")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unauthorized), "{err:?}");
        assert_eq!(idp.token_request_count(), 0);
    }

    #[tokio::test]
    async fn discovery_rejects_plain_http_and_reports_an_unreachable_idp() {
        let oidc = OidcState::new();
        let err = oidc
            .check_discovery("http://id.example.org")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Validation(_)), "{err:?}");
        // Port 1 auf Loopback ist zu: schlägt sofort fehl statt nach dem Zeitlimit.
        let err = oidc
            .check_discovery("http://127.0.0.1:1")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)), "{err:?}");

        let idp = test_idp::start("suchlage").await;
        oidc.check_discovery(&format!("{}/", idp.issuer))
            .await
            .unwrap();
    }

    #[test]
    fn next_only_allows_local_paths() {
        assert_eq!(safe_next("/flights/3"), "/flights/3");
        assert_eq!(safe_next("//evil.example"), "/");
        assert_eq!(safe_next("https://evil.example"), "/");
        assert_eq!(safe_next("/\\evil"), "/");
        assert_eq!(safe_next(""), "/");
        assert_eq!(safe_next("/\t/evil.example"), "/");
        assert_eq!(safe_next("/\n/evil.example"), "/");
        assert_eq!(safe_next("/ /evil.example"), "/");
    }
}

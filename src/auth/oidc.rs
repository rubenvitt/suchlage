//! OIDC (Authorization Code + PKCE, State, Nonce) gegen Pocket ID. Muster aus Lifeline
//! `auth/oidc`, aber mit Einstellungen aus der DB statt Env und ohne OnceLock-Globals.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use futures::future::BoxFuture;
use openidconnect::core::{
    CoreAuthenticationFlow, CoreClient, CoreJsonWebKeySet, CoreProviderMetadata,
};
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
/// So lange gelten Discovery und JWKS, bevor sie neu geladen werden.
const DISCOVERY_TTL: Duration = Duration::from_secs(3600);
/// Höchstens so oft fragt Suchlage den Anbieter erneut: nach einem unbekannten `kid`, nach Ablauf
/// des JWKS und nach einem fehlgeschlagenen Abruf (auch der Discovery).
const MIN_REFRESH: Duration = Duration::from_secs(60);

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

struct CachedDiscovery {
    issuer: String,
    /// Zuletzt geladene Metadaten und wann.
    meta: Option<(CoreProviderMetadata, Instant)>,
    /// Letzter Fehlschlag: Bis `min_refresh` vergangen ist, fragt niemand erneut.
    failed_at: Option<Instant>,
}

/// JWKS eines Ausstellers. Abrufe sind gedrosselt (`attempted`), auch nach einem Fehlschlag; bis
/// dahin und bei einem Fehlschlag gelten die zuletzt geladenen Schlüssel weiter.
#[derive(Default)]
struct CachedJwks {
    issuer: String,
    /// Zuletzt erfolgreich geladene Schlüssel und wann.
    keys: Option<(CoreJsonWebKeySet, Instant)>,
    /// Beginn des letzten Abrufs, ob er geklappt hat oder nicht.
    attempted: Option<Instant>,
}

/// Was eine Anfrage nach dem JWKS tun muss.
enum JwksStep<'a> {
    Use(CoreJsonWebKeySet),
    /// Diese Anfrage ruft ab; die Sperre zeigt anderen, dass ein Abruf läuft.
    Fetch(tokio::sync::MutexGuard<'a, ()>),
    /// Keine Schlüssel und kein Abruf erlaubt: auf einen laufenden warten, sonst 503.
    Wait,
}

pub struct OidcState {
    pending: Mutex<HashMap<String, Pending>>,
    /// Die Sperre hält über den Abruf, damit gleichzeitige Anmeldungen den Anbieter nur einmal
    /// fragen. Zugriffstokens warten nur darauf, wenn sie selbst gerade abrufen.
    discovery: tokio::sync::Mutex<Option<CachedDiscovery>>,
    /// Schlüssel für die Zugriffstokens der Flugmappe (`auth::bearer`). Die Sperre ist kurz und
    /// hält nie über einen Abruf: Ein Treffer im Cache wartet nicht auf einen langsamen Anbieter.
    jwks: Mutex<CachedJwks>,
    /// Gehalten, solange ein JWKS-Abruf läuft (höchstens einer zugleich).
    jwks_fetch: tokio::sync::Mutex<()>,
    min_refresh: Duration,
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

fn discovery_failed() -> AppError {
    AppError::Unavailable("Anmeldedienst nicht erreichbar oder Issuer falsch".into())
}

fn keys_unavailable() -> AppError {
    AppError::Unavailable("Schlüssel des Anmeldedienstes nicht abrufbar".into())
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
            jwks: Mutex::new(CachedJwks::default()),
            jwks_fetch: tokio::sync::Mutex::new(()),
            min_refresh: MIN_REFRESH,
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

    /// Discovery mit Cache je Issuer; der zweite Wert sagt, ob sie eben erst geladen wurde. Ein
    /// Fehlschlag gilt `min_refresh` lang, ohne den Anbieter erneut zu fragen – außer mit `retry`
    /// (Prüfung beim Einrichten, Anmeldung im Browser).
    async fn metadata(
        &self,
        issuer: &str,
        now: Instant,
        retry: bool,
    ) -> Result<(CoreProviderMetadata, bool), AppError> {
        if !issuer_allowed(issuer) {
            return Err(AppError::Validation(
                "Issuer muss mit https:// beginnen (http nur für localhost)".into(),
            ));
        }
        let mut cache = self.discovery.lock().await;
        if let Some(c) = cache.as_ref().filter(|c| c.issuer == issuer) {
            if let Some((meta, at)) = &c.meta
                && now.saturating_duration_since(*at) < DISCOVERY_TTL
            {
                return Ok((meta.clone(), false));
            }
            if !retry
                && c.failed_at
                    .is_some_and(|at| now.saturating_duration_since(at) < self.min_refresh)
            {
                return Err(discovery_failed());
            }
        }
        let url = IssuerUrl::new(issuer.to_string())
            .map_err(|_| AppError::Validation("Issuer ist keine gültige URL".into()))?;
        let result = CoreProviderMetadata::discover_async(url, self.http(issuer)).await;
        let entry = match cache.take().filter(|c| c.issuer == issuer) {
            Some(c) => cache.insert(c),
            None => cache.insert(CachedDiscovery {
                issuer: issuer.to_string(),
                meta: None,
                failed_at: None,
            }),
        };
        match result {
            Ok(meta) => {
                entry.meta = Some((meta.clone(), now));
                entry.failed_at = None;
                Ok((meta, true))
            }
            Err(e) => {
                tracing::warn!(error = %e, issuer, "OIDC-Discovery fehlgeschlagen");
                entry.failed_at = Some(now);
                Err(discovery_failed())
            }
        }
    }

    /// Tests fragen den Anbieter ohne Drosselung erneut.
    #[cfg(test)]
    pub fn with_min_refresh(mut self, every: Duration) -> Self {
        self.min_refresh = every;
        self
    }

    /// JWKS des Ausstellers mit Cache je Aussteller; `now` kommt herein, damit Tests die Zeit
    /// vorstellen können. Abgerufen wird nach Ablauf (1 h) oder mit `refresh` (unbekannter `kid`),
    /// aber höchstens alle 60 s, auch nach einem Fehlschlag. Scheitert der Abruf, gelten die alten
    /// Schlüssel weiter; 503 nur, wenn es nie welche gab.
    pub async fn jwks(
        &self,
        issuer: &str,
        refresh: bool,
        now: Instant,
    ) -> Result<CoreJsonWebKeySet, AppError> {
        let issuer = issuer.trim_end_matches('/');
        let step = {
            let mut cache = self.jwks.lock().unwrap();
            if cache.issuer != issuer {
                *cache = CachedJwks {
                    issuer: issuer.to_string(),
                    ..CachedJwks::default()
                };
            }
            let fresh = cache
                .keys
                .as_ref()
                .is_some_and(|(_, at)| now.saturating_duration_since(*at) < DISCOVERY_TTL);
            let may_fetch = (refresh || !fresh)
                && cache
                    .attempted
                    .is_none_or(|at| now.saturating_duration_since(at) >= self.min_refresh);
            match (
                may_fetch.then(|| self.jwks_fetch.try_lock().ok()).flatten(),
                &cache.keys,
            ) {
                (Some(guard), _) => {
                    cache.attempted = Some(now);
                    JwksStep::Fetch(guard)
                }
                (None, Some((keys, _))) => JwksStep::Use(keys.clone()),
                (None, None) => JwksStep::Wait,
            }
        };
        let _fetching = match step {
            JwksStep::Use(keys) => return Ok(keys),
            JwksStep::Fetch(guard) => guard,
            JwksStep::Wait => {
                // Die erste Ladung läuft vielleicht gerade: auf sie warten statt 503.
                drop(self.jwks_fetch.lock().await);
                return self.cached_jwks(issuer).ok_or_else(keys_unavailable);
            }
        };
        let result = self.fetch_jwks(issuer, now).await;
        let mut cache = self.jwks.lock().unwrap();
        let same = cache.issuer == issuer;
        match result {
            Ok(keys) => {
                if same {
                    cache.keys = Some((keys.clone(), now));
                }
                Ok(keys)
            }
            Err(e) => match cache.keys.as_ref().filter(|_| same) {
                Some((keys, _)) => {
                    tracing::warn!(
                        error = %e,
                        issuer,
                        "JWKS nicht abrufbar – die bisherigen Schlüssel gelten weiter"
                    );
                    Ok(keys.clone())
                }
                None => Err(e),
            },
        }
    }

    fn cached_jwks(&self, issuer: &str) -> Option<CoreJsonWebKeySet> {
        let cache = self.jwks.lock().unwrap();
        cache
            .keys
            .as_ref()
            .filter(|_| cache.issuer == issuer)
            .map(|(keys, _)| keys.clone())
    }

    /// Bringt die Discovery den JWKS eben erst mit, gilt er; sonst wird `jwks_uri` direkt abgerufen
    /// (nie die alte Kopie aus der Discovery, sonst bliebe ein widerrufener Schlüssel gültig).
    async fn fetch_jwks(&self, issuer: &str, now: Instant) -> Result<CoreJsonWebKeySet, AppError> {
        let (meta, fresh) = self.metadata(issuer, now, false).await?;
        if fresh {
            return Ok(meta.jwks().clone());
        }
        CoreJsonWebKeySet::fetch_async(meta.jwks_uri(), self.http(issuer))
            .await
            .map_err(|e| {
                tracing::warn!(error = %e, issuer, "JWKS nicht abrufbar");
                keys_unavailable()
            })
    }

    pub async fn check_discovery(&self, issuer: &str) -> Result<(), AppError> {
        self.metadata(issuer.trim_end_matches('/'), Instant::now(), true)
            .await
            .map(|_| ())
    }

    /// Eine Anmeldung fragt nach einem Fehlschlag sofort neu: Die Drosselung gilt den
    /// Zugriffstokens und soll niemanden aussperren, wenn der Anbieter wieder da ist.
    async fn client(&self, cfg: &OidcSettings, redirect_url: &str) -> Result<Client, AppError> {
        let (meta, _) = self.metadata(&cfg.issuer, Instant::now(), true).await?;
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
    //! Er zählt Discovery- und JWKS-Abrufe und merkt sich jede Token-Anfrage, damit Tests PKCE und
    //! Redirect-URL prüfen können. Zugriffstokens wie Pocket ID (RFC 9068) stellt er direkt aus.

    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use axum::extract::{Form, State};
    use axum::http::StatusCode;
    use axum::routing::{get, post};
    use axum::{Json, Router};
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
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
    /// `kid` des einzigen Schlüssels.
    pub const KEY_ID: &str = "test";

    /// Der RSA-Schlüssel des Test-IdP (synthetisch, `tests/fixtures/oidc/test-rsa.pem`).
    pub fn signing_key() -> CoreRsaPrivateSigningKey {
        let pem = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/oidc/test-rsa.pem"
        ))
        .unwrap();
        CoreRsaPrivateSigningKey::from_pem(&pem, Some(JsonWebKeyId::new(KEY_ID.into()))).unwrap()
    }

    /// JWT mit beliebigem Kopf, immer mit RS256 und dem Testschlüssel signiert. So lassen sich auch
    /// Köpfe bauen, die lügen (`alg: HS256`).
    pub fn sign(header: &serde_json::Value, claims: &serde_json::Value) -> String {
        let input = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(header.to_string()),
            URL_SAFE_NO_PAD.encode(claims.to_string())
        );
        let signature = signing_key()
            .sign(
                &CoreJwsSigningAlgorithm::RsaSsaPkcs1V15Sha256,
                input.as_bytes(),
            )
            .unwrap();
        format!("{input}.{}", URL_SAFE_NO_PAD.encode(signature))
    }

    /// Zugriffstoken wie von Pocket ID; jedes Feld lässt sich für Fehlerfälle verbiegen.
    #[derive(Debug, Clone)]
    pub struct AccessToken {
        pub sub: String,
        /// Pocket ID: `[client_id, issuer]`; hier nur die Client-ID, den Aussteller ergänzt
        /// `access_token`.
        pub aud: serde_json::Value,
        pub ttl_s: i64,
        pub typ: String,
        pub kid: String,
        /// Zusätzliche oder ersetzte Claims (`null` entfernt einen).
        pub extra: serde_json::Value,
    }

    impl AccessToken {
        pub fn new(sub: &str, client_id: &str) -> Self {
            Self {
                sub: sub.into(),
                aud: serde_json::json!(client_id),
                ttl_s: 3600,
                typ: "at+jwt".into(),
                kid: KEY_ID.into(),
                extra: serde_json::json!({ "client_id": client_id }),
            }
        }
    }

    #[derive(Clone)]
    pub struct TestIdp {
        pub issuer: String,
        /// Nonce, die der nächste ID-Token trägt. Der Test übernimmt sie aus der Autorisierungs-URL.
        pub nonce: Arc<Mutex<String>>,
        discovery_requests: Arc<AtomicUsize>,
        jwks_requests: Arc<AtomicUsize>,
        token_requests: Arc<Mutex<Vec<HashMap<String, String>>>>,
        client_id: &'static str,
        key: Arc<CoreRsaPrivateSigningKey>,
        /// Ausfall: Discovery und JWKS antworten 503 (gezählt wird trotzdem).
        down: Arc<AtomicBool>,
        /// So lange lässt der JWKS-Endpunkt auf sich warten.
        jwks_delay_ms: Arc<AtomicU64>,
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

        pub fn jwks_count(&self) -> usize {
            self.jwks_requests.load(Ordering::SeqCst)
        }

        pub fn set_down(&self, down: bool) {
            self.down.store(down, Ordering::SeqCst);
        }

        pub fn set_jwks_delay(&self, delay: std::time::Duration) {
            self.jwks_delay_ms
                .store(delay.as_millis() as u64, Ordering::SeqCst);
        }

        /// Zugriffstoken (RS256, `typ: at+jwt`) mit den Claims von Pocket ID.
        pub fn access_token(&self, t: AccessToken) -> String {
            let now = chrono::Utc::now().timestamp();
            let aud = match t.aud {
                serde_json::Value::String(client) => serde_json::json!([client, self.issuer]),
                other => other,
            };
            let mut claims = serde_json::json!({
                "iss": self.issuer, "sub": t.sub, "aud": aud, "exp": now + t.ttl_s, "iat": now,
                "jti": uuid::Uuid::new_v4().to_string(), "scope": "openid profile",
                "scp": ["openid", "profile"],
            });
            for (k, v) in t.extra.as_object().into_iter().flatten() {
                if v.is_null() {
                    claims.as_object_mut().unwrap().remove(k);
                } else {
                    claims[k] = v.clone();
                }
            }
            sign(
                &serde_json::json!({"alg": "RS256", "typ": t.typ, "kid": t.kid}),
                &claims,
            )
        }

        pub fn access_token_for(&self, sub: &str, client_id: &str, ttl_s: i64) -> String {
            self.access_token(AccessToken {
                ttl_s,
                ..AccessToken::new(sub, client_id)
            })
        }
    }

    pub async fn start(client_id: &'static str) -> TestIdp {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let issuer = format!("http://{}", listener.local_addr().unwrap());
        let key = signing_key();
        let idp = TestIdp {
            issuer,
            nonce: Arc::new(Mutex::new(String::new())),
            discovery_requests: Arc::new(AtomicUsize::new(0)),
            jwks_requests: Arc::new(AtomicUsize::new(0)),
            token_requests: Arc::new(Mutex::new(Vec::new())),
            client_id,
            key: Arc::new(key),
            down: Arc::new(AtomicBool::new(false)),
            jwks_delay_ms: Arc::new(AtomicU64::new(0)),
        };
        let app = Router::new()
            .route("/.well-known/openid-configuration", get(discovery))
            .route("/jwks", get(jwks))
            .route("/token", post(token))
            .with_state(idp.clone());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        idp
    }

    async fn discovery(State(idp): State<TestIdp>) -> Result<Json<serde_json::Value>, StatusCode> {
        idp.discovery_requests.fetch_add(1, Ordering::SeqCst);
        if idp.down.load(Ordering::SeqCst) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        let i = &idp.issuer;
        Ok(Json(serde_json::json!({
            "issuer": i, "authorization_endpoint": format!("{i}/authorize"), "token_endpoint": format!("{i}/token"),
            "jwks_uri": format!("{i}/jwks"), "response_types_supported": ["code"], "subject_types_supported": ["public"],
            "id_token_signing_alg_values_supported": ["RS256"]
        })))
    }

    async fn jwks(State(idp): State<TestIdp>) -> Result<Json<CoreJsonWebKeySet>, StatusCode> {
        idp.jwks_requests.fetch_add(1, Ordering::SeqCst);
        let delay = idp.jwks_delay_ms.load(Ordering::SeqCst);
        if delay > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
        }
        if idp.down.load(Ordering::SeqCst) {
            return Err(StatusCode::SERVICE_UNAVAILABLE);
        }
        Ok(Json(CoreJsonWebKeySet::new(vec![
            idp.key.as_verification_key(),
        ])))
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

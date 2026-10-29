//! Anmeldung über HTTP: Optionen, Ersteinrichtung, OIDC, eigene Sitzung, Abmelden,
//! Großbild-Zugang, Dev-Login (Feature `dev`).

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Redirect;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};

use crate::app::AppState;
use crate::auth::oidc::{PENDING_TTL, STATE_COOKIE};
use crate::auth::session::{COOKIE, Viewer, secure_cookies, session_cookie};
use crate::contract::{
    AuthOptions, KioskEnroll, Me, OidcSettingsInput, Role, SetupRequest, SetupResult,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::store::{self, sessions::Principal};
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    let r = Router::new()
        .route("/auth/options", get(options))
        .route("/auth/logout", post(logout))
        .route("/auth/kiosk", post(kiosk_enroll))
        .route("/auth/oidc/start", get(oidc_start))
        .route("/auth/oidc/callback", get(oidc_callback))
        .route("/setup", post(setup))
        .route("/me", get(me));
    #[cfg(feature = "dev")]
    let r = r.route("/auth/dev-login", post(crate::auth::dev::login));
    r
}

async fn options(State(state): State<AppState>) -> Result<Json<AuthOptions>, AppError> {
    Ok(Json(AuthOptions {
        oidc: state.settings.current().oidc.is_some(),
        dev_login: cfg!(feature = "dev"),
        setup_required: crate::auth::setup::required(&state.pool, &state.settings).await?,
        redirect_url: crate::settings::redirect_url(&state.boot),
    }))
}

async fn setup(
    State(state): State<AppState>,
    JsonBody(body): JsonBody<SetupRequest>,
) -> Result<Json<SetupResult>, AppError> {
    if !crate::auth::setup::required(&state.pool, &state.settings).await? {
        return Err(AppError::Conflict(
            "Suchlage ist bereits eingerichtet".into(),
        ));
    }
    let input = OidcSettingsInput {
        issuer: body.issuer,
        client_id: body.client_id,
        client_secret: Some(body.client_secret),
        admin_group: body.admin_group,
        user_group: body.user_group,
    };
    // Token vor jedem ausgehenden Abruf prüfen, aber erst nach erfolgreicher Discovery verbrauchen:
    // Ohne gültigen Link löst niemand Anfragen des Servers aus, und ein Tippfehler im Issuer
    // kostet keinen neuen Link.
    crate::settings::validate_oidc(&input)?;
    crate::auth::setup::verify(&state.pool, &body.token, now_ms()).await?;
    state.oidc.check_discovery(input.issuer.trim()).await?;
    crate::auth::setup::consume(&state.pool, &body.token, now_ms()).await?;
    state.settings.update_oidc(input).await?;
    Ok(Json(SetupResult {
        login_url: "/api/auth/oidc/start?next=/".into(),
    }))
}

/// Pfad des Bindungscookies: Es geht nur an Start und Rücksprung.
const STATE_COOKIE_PATH: &str = "/api/auth/oidc";
/// Jeder Fehler der Anmeldung landet hier; die Details stehen nur im Log.
const LOGIN_FAILED: &str = "/anmelden?fehler=oidc";
/// Angemeldet, aber in keiner der beiden Pocket-ID-Gruppen.
const LOGIN_NO_GROUP: &str = "/anmelden?fehler=gruppe";
/// In einer Gruppe, aber in Suchlage gesperrt.
const LOGIN_BLOCKED: &str = "/anmelden?fehler=gesperrt";

#[derive(serde::Deserialize)]
struct StartQuery {
    next: Option<String>,
}

/// Der Browser kommt per Navigation, nicht per `fetch`. Deshalb antwortet auch ein Fehler mit
/// einem Rücksprung auf die Anmeldeseite statt mit JSON.
async fn oidc_start(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<StartQuery>,
) -> (CookieJar, Redirect) {
    match begin_login(&state, q.next.unwrap_or_else(|| "/".into())).await {
        Ok((url, csrf)) => {
            let cookie = Cookie::build((STATE_COOKIE, csrf))
                .path(STATE_COOKIE_PATH)
                .http_only(true)
                .same_site(SameSite::Lax)
                .secure(secure_cookies(&state.boot))
                .max_age(time::Duration::seconds(PENDING_TTL.as_secs() as i64))
                .build();
            (jar.add(cookie), Redirect::to(url.as_str()))
        }
        Err(e) => {
            tracing::warn!(error = ?e, "OIDC-Anmeldung nicht gestartet");
            (jar, Redirect::to(LOGIN_FAILED))
        }
    }
}

async fn begin_login(state: &AppState, next: String) -> Result<(url::Url, String), AppError> {
    let cfg = state
        .settings
        .current()
        .oidc
        .ok_or_else(|| AppError::Unavailable("Anmeldung ist noch nicht eingerichtet".into()))?;
    let redirect = crate::settings::redirect_url(&state.boot);
    state.oidc.begin(&cfg, &redirect, next).await
}

#[derive(serde::Deserialize)]
struct CallbackQuery {
    state: Option<String>,
    code: Option<String>,
}

async fn oidc_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(q): Query<CallbackQuery>,
) -> (CookieJar, Redirect) {
    let jar_out = jar
        .clone()
        .remove(Cookie::build(STATE_COOKIE).path(STATE_COOKIE_PATH));
    match complete_login(&state, &jar, q).await {
        Ok((token, next)) => {
            let session = session_cookie(
                token,
                store::sessions::USER_TTL_MS,
                secure_cookies(&state.boot),
            );
            (jar_out.add(session), Redirect::to(&next))
        }
        Err(LoginError::NoGroup) => (jar_out, Redirect::to(LOGIN_NO_GROUP)),
        Err(LoginError::Blocked) => (jar_out, Redirect::to(LOGIN_BLOCKED)),
        Err(LoginError::Failed(e)) => {
            tracing::warn!(error = ?e, "OIDC-Anmeldung abgebrochen");
            (jar_out, Redirect::to(LOGIN_FAILED))
        }
    }
}

/// Warum eine Anmeldung scheitert; jeder Fall hat seine eigene Meldung auf der Anmeldeseite.
enum LoginError {
    NoGroup,
    Blocked,
    Failed(AppError),
}

impl From<AppError> for LoginError {
    fn from(e: AppError) -> Self {
        LoginError::Failed(e)
    }
}

impl From<sqlx::Error> for LoginError {
    fn from(e: sqlx::Error) -> Self {
        LoginError::Failed(e.into())
    }
}

async fn complete_login(
    state: &AppState,
    jar: &CookieJar,
    q: CallbackQuery,
) -> Result<(String, String), LoginError> {
    let (Some(csrf), Some(code)) = (q.state, q.code) else {
        return Err(AppError::Unauthorized.into());
    };
    // Bindet den Rücksprung an den Browser, der die Anmeldung begonnen hat (Login-CSRF).
    if jar.get(STATE_COOKIE).map(|c| c.value()) != Some(csrf.as_str()) {
        return Err(AppError::Unauthorized.into());
    }
    let cfg = state
        .settings
        .current()
        .oidc
        .ok_or(AppError::Unauthorized)?;
    let redirect = crate::settings::redirect_url(&state.boot);
    let (identity, next) = state.oidc.finish(&cfg, &redirect, &csrf, &code).await?;
    let Some(role) = identity.role(&cfg) else {
        tracing::warn!(
            user = %identity.display_name,
            groups = ?identity.groups,
            "Anmeldung abgelehnt: in keiner Suchlage-Gruppe"
        );
        return Err(LoginError::NoGroup);
    };
    let now = now_ms();
    let user = store::users::upsert_oidc(
        &state.pool,
        &identity.issuer,
        &identity.subject,
        &identity.display_name,
        role,
        now,
    )
    .await?;
    if store::users::is_blocked(&state.pool, user.id).await? {
        tracing::warn!(user = %user.display_name, "Anmeldung abgelehnt: Konto gesperrt");
        return Err(LoginError::Blocked);
    }
    Ok((
        store::sessions::create_for_user(&state.pool, user.id, now).await?,
        next,
    ))
}

async fn me(Viewer(principal): Viewer) -> Json<Me> {
    Json(match principal {
        Principal::User(u) => Me {
            display_name: u.display_name,
            role: u.role,
        },
        Principal::Kiosk { label, .. } => Me {
            display_name: label,
            role: Role::Kiosk,
        },
    })
}

async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    if let Some(c) = jar.get(COOKIE) {
        store::sessions::delete(&state.pool, c.value()).await?;
    }
    // Pfad wie beim Setzen, sonst behält der Browser das Cookie.
    let removal = Cookie::build(COOKIE).path("/");
    Ok((jar.remove(removal), StatusCode::NO_CONTENT))
}

async fn kiosk_enroll(
    State(state): State<AppState>,
    jar: CookieJar,
    JsonBody(body): JsonBody<KioskEnroll>,
) -> Result<(CookieJar, StatusCode), AppError> {
    let Some(token_id) = store::kiosk::find_by_token(&state.pool, body.token.trim()).await? else {
        return Err(AppError::UnauthorizedWith(
            "Zugangslink ungültig oder widerrufen".into(),
        ));
    };
    let token = store::sessions::create_for_kiosk(&state.pool, token_id, now_ms()).await?;
    let cookie = session_cookie(
        token,
        store::sessions::KIOSK_TTL_MS,
        secure_cookies(&state.boot),
    );
    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::contract::Role;
    use crate::testing::{get, post_json, send, test_app, with_cookie};

    #[tokio::test]
    async fn me_requires_a_session() {
        let app = test_app().await;
        let (status, body) = send(&app.router, get("/api/me")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "Nicht angemeldet");
        let cookie = app.cookie(Role::Operator).await;
        let (status, body) = send(&app.router, with_cookie(get("/api/me"), &cookie)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["role"], "operator");
    }

    #[tokio::test]
    async fn kiosk_link_logs_in_as_kiosk_and_dies_with_revocation() {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        let (status, created) = send(
            &app.router,
            with_cookie(
                post_json("/api/kiosk-tokens", serde_json::json!({"label": "FüKw"})),
                &admin,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let url = created["url"].as_str().unwrap();
        let token = url
            .split_once("/kiosk#")
            .expect("Link auf /kiosk#<token>")
            .1;

        let res = tower::ServiceExt::oneshot(
            app.router.clone(),
            post_json("/api/auth/kiosk", serde_json::json!({"token": token})),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let set_cookie = res.headers()["set-cookie"].to_str().unwrap().to_string();
        assert!(set_cookie.contains("HttpOnly") && set_cookie.contains("SameSite=Lax"));
        let kiosk = set_cookie.split(';').next().unwrap().to_string();
        let (_, me) = send(&app.router, with_cookie(get("/api/me"), &kiosk)).await;
        assert_eq!(me["role"], "kiosk");
        assert_eq!(me["display_name"], "FüKw");

        let (status, _) = send(&app.router, with_cookie(get("/api/kiosk-tokens"), &kiosk)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "Großbild verwaltet nichts");

        let id = created["token"]["id"].as_i64().unwrap();
        let req = axum::http::Request::delete(format!("/api/kiosk-tokens/{id}"))
            .body(axum::body::Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &kiosk))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, body) = send(
            &app.router,
            post_json("/api/auth/kiosk", serde_json::json!({"token": token})),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["error"], "Zugangslink ungültig oder widerrufen");
    }

    #[tokio::test]
    async fn operator_cannot_manage_kiosk_tokens() {
        let app = test_app().await;
        let op = app.cookie(Role::Operator).await;
        let (status, _) = send(
            &app.router,
            with_cookie(
                post_json("/api/kiosk-tokens", serde_json::json!({"label": "x"})),
                &op,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn logout_ends_the_session() {
        let app = test_app().await;
        let c = app.cookie(Role::Operator).await;
        let res = tower::ServiceExt::oneshot(
            app.router.clone(),
            with_cookie(post_json("/api/auth/logout", serde_json::json!({})), &c),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let removal = res.headers()["set-cookie"].to_str().unwrap();
        assert!(
            removal.contains("Max-Age=0") && removal.contains("Path=/"),
            "Browser löscht das Cookie: {removal}"
        );
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &c)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    /// Ersteinrichtung über den Einmal-Link: Ein Tippfehler oder ein nicht erreichbarer Anbieter
    /// kostet den Link nicht, danach ist Suchlage eingerichtet und der Weg zu.
    #[cfg(not(feature = "dev"))]
    #[tokio::test]
    async fn setup_link_configures_oidc_exactly_once() {
        let idp = crate::auth::oidc::test_idp::start("suchlage").await;
        let app = test_app().await;
        let (_, options) = send(&app.router, get("/api/auth/options")).await;
        assert_eq!(options["setup_required"], true);
        assert_eq!(options["oidc"], false);
        assert_eq!(
            options["redirect_url"],
            "https://suchlage.test/api/auth/oidc/callback"
        );
        let link = crate::auth::setup::ensure_token(
            &app.state.pool,
            &app.state.settings,
            &app.state.boot,
            crate::time::now_ms(),
        )
        .await
        .unwrap()
        .expect("ohne OIDC und Konto gibt es einen Link");
        let token = link.split_once("/einrichtung#").unwrap().1;
        let request = |token: &str, issuer: &str| {
            post_json(
                "/api/setup",
                serde_json::json!({"token": token, "issuer": issuer, "client_id": "suchlage", "client_secret": "geheim", "admin_group": "suchlage-admin", "user_group": "suchlage"}),
            )
        };

        let (status, body) = send(&app.router, request("falsch", &idp.issuer)).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(
            body["error"].as_str().unwrap().contains("Einrichtungslink"),
            "{body}"
        );
        // Ohne gültigen Link fragt der Server keinen fremden Rechner ab (kein Port-Abtasten).
        let (status, _) = send(&app.router, request("falsch", "http://127.0.0.1:1")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(
            idp.discovery_count(),
            0,
            "ohne gültigen Link keine Discovery"
        );
        let (status, body) = send(&app.router, request(token, "id.example.org")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "Tippfehler im Issuer");
        assert!(body["error"].as_str().unwrap().contains("Issuer"), "{body}");
        // Port 1 auf Loopback ist zu: kein Anbieter antwortet.
        let (status, body) = send(&app.router, request(token, "http://127.0.0.1:1")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert!(
            body["error"].as_str().unwrap().contains("Anmeldedienst"),
            "{body}"
        );

        let issuer_with_slash = format!(" {}/ ", idp.issuer);
        let (status, body) = send(&app.router, request(token, &issuer_with_slash)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["login_url"], "/api/auth/oidc/start?next=/");
        let oidc = app.state.settings.current().oidc.unwrap();
        assert_eq!(oidc.issuer, idp.issuer);
        assert_eq!(oidc.client_secret, "geheim");
        let (_, options) = send(&app.router, get("/api/auth/options")).await;
        assert_eq!(options["setup_required"], false);
        assert_eq!(options["oidc"], true);

        let (status, _) = send(&app.router, request(token, "https://boese.example.org")).await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "eingerichtet ist eingerichtet"
        );
        assert_eq!(
            app.state.settings.current().oidc.unwrap().issuer,
            idp.issuer
        );
    }

    async fn app_with_idp() -> (
        crate::testing::TestApp,
        crate::auth::oidc::test_idp::TestIdp,
    ) {
        let idp = crate::auth::oidc::test_idp::start("suchlage").await;
        let app = test_app().await;
        app.state
            .settings
            .update_oidc(crate::contract::OidcSettingsInput {
                issuer: idp.issuer.clone(),
                client_id: "suchlage".into(),
                client_secret: Some("s".into()),
                admin_group: "suchlage-admin".into(),
                user_group: "suchlage".into(),
            })
            .await
            .unwrap();
        (app, idp)
    }

    fn location(res: &axum::response::Response) -> &str {
        res.headers()["location"].to_str().unwrap()
    }

    fn set_cookies(res: &axum::response::Response) -> Vec<String> {
        res.headers()
            .get_all("set-cookie")
            .iter()
            .map(|v| v.to_str().unwrap().to_string())
            .collect()
    }

    #[tokio::test]
    async fn oidc_login_end_to_end_takes_the_role_from_the_groups() {
        let (app, idp) = app_with_idp().await;
        *idp.groups.lock().unwrap() = vec!["suchlage".into(), "suchlage-admin".into()];

        let res = tower::ServiceExt::oneshot(
            app.router.clone(),
            get("/api/auth/oidc/start?next=/flights"),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        let authorize = url::Url::parse(location(&res)).unwrap();
        assert_eq!(authorize.path(), "/authorize");
        let q: std::collections::HashMap<_, _> = authorize.query_pairs().into_owned().collect();
        assert_eq!(
            q["redirect_uri"],
            "https://suchlage.test/api/auth/oidc/callback"
        );
        *idp.nonce.lock().unwrap() = q["nonce"].clone();
        let binding = set_cookies(&res).pop().unwrap();
        for attr in [
            "HttpOnly",
            "SameSite=Lax",
            "Secure",
            "Path=/api/auth/oidc",
            "Max-Age=600",
        ] {
            assert!(binding.contains(attr), "{attr} fehlt: {binding}");
        }
        let binding = binding.split(';').next().unwrap().to_string();
        assert_eq!(binding, format!("suchlage_oidc={}", q["state"]));

        let cb = format!("/api/auth/oidc/callback?state={}&code=c1", q["state"]);
        let res = tower::ServiceExt::oneshot(app.router.clone(), with_cookie(get(&cb), &binding))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(location(&res), "/flights");
        assert_eq!(idp.last_token_request()["code"], "c1");
        let cookies = set_cookies(&res);
        assert!(
            cookies
                .iter()
                .any(|c| c.starts_with("suchlage_oidc=") && c.contains("Max-Age=0")),
            "Bindungscookie wird gelöscht: {cookies:?}"
        );
        let session = cookies
            .iter()
            .find(|c| c.starts_with("suchlage_sid="))
            .expect("Sitzungscookie");
        let (_, me) = send(
            &app.router,
            with_cookie(get("/api/me"), session.split(';').next().unwrap()),
        )
        .await;
        assert_eq!(me["role"], "admin");
        assert_eq!(me["display_name"], crate::auth::oidc::test_idp::USERNAME);
    }

    /// Startet die Anmeldung und folgt dem Rücksprung; liefert die Antwort des Callbacks.
    async fn login(
        app: &crate::testing::TestApp,
        idp: &crate::auth::oidc::test_idp::TestIdp,
    ) -> axum::response::Response {
        let res = tower::ServiceExt::oneshot(app.router.clone(), get("/api/auth/oidc/start"))
            .await
            .unwrap();
        let authorize = url::Url::parse(location(&res)).unwrap();
        let q: std::collections::HashMap<_, _> = authorize.query_pairs().into_owned().collect();
        *idp.nonce.lock().unwrap() = q["nonce"].clone();
        let cb = format!("/api/auth/oidc/callback?state={}&code=c", q["state"]);
        let binding = format!("suchlage_oidc={}", q["state"]);
        tower::ServiceExt::oneshot(app.router.clone(), with_cookie(get(&cb), &binding))
            .await
            .unwrap()
    }

    fn session(res: &axum::response::Response) -> Option<String> {
        set_cookies(res)
            .into_iter()
            .find(|c| c.starts_with("suchlage_sid="))
            .map(|c| c.split(';').next().unwrap().to_string())
    }

    #[tokio::test]
    async fn login_without_a_suchlage_group_is_refused_and_the_role_follows_the_groups() {
        let (app, idp) = app_with_idp().await;
        *idp.groups.lock().unwrap() = vec!["andere".into()];
        let res = login(&app, &idp).await;
        assert_eq!(location(&res), "/anmelden?fehler=gruppe");
        assert_eq!(session(&res), None, "keine Sitzung");
        assert_eq!(
            crate::store::users::count(&app.state.pool).await.unwrap(),
            0
        );

        *idp.groups.lock().unwrap() = vec!["suchlage-admin".into()];
        let admin = session(&login(&app, &idp).await).expect("Sitzung");
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &admin))
                .await
                .1["role"],
            "admin"
        );

        *idp.groups.lock().unwrap() = vec!["suchlage".into()];
        let op = session(&login(&app, &idp).await).expect("Sitzung");
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &op)).await.1["role"],
            "operator"
        );
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &admin))
                .await
                .1["role"],
            "operator",
            "die Rolle hängt am Konto, auch ältere Sitzungen sind herabgestuft"
        );
    }

    #[tokio::test]
    async fn blocked_account_gets_no_session_until_unblocked() {
        let (app, idp) = app_with_idp().await;
        *idp.groups.lock().unwrap() = vec!["suchlage".into()];
        let old = session(&login(&app, &idp).await).expect("Sitzung");
        let user = crate::store::users::find_by_oidc(
            &app.state.pool,
            &idp.issuer,
            crate::auth::oidc::test_idp::SUBJECT,
        )
        .await
        .unwrap()
        .unwrap();
        crate::store::users::block(&app.state.pool, user.id, 1)
            .await
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(get("/api/me"), &old)).await.0,
            StatusCode::UNAUTHORIZED
        );

        let res = login(&app, &idp).await;
        assert_eq!(location(&res), "/anmelden?fehler=gesperrt");
        assert_eq!(session(&res), None, "trotz Gruppe keine Sitzung");

        crate::store::users::unblock(&app.state.pool, user.id)
            .await
            .unwrap();
        assert!(session(&login(&app, &idp).await).is_some());
    }

    #[tokio::test]
    async fn callback_without_matching_binding_cookie_is_refused() {
        let (app, idp) = app_with_idp().await;
        let res = tower::ServiceExt::oneshot(app.router.clone(), get("/api/auth/oidc/start"))
            .await
            .unwrap();
        let authorize = url::Url::parse(location(&res)).unwrap();
        let st = authorize
            .query_pairs()
            .find(|(k, _)| k == "state")
            .unwrap()
            .1
            .to_string();
        let cb = format!("/api/auth/oidc/callback?state={st}&code=c");
        for req in [
            get(&cb),
            with_cookie(get(&cb), "suchlage_oidc=fremd"),
            with_cookie(
                get("/api/auth/oidc/callback?code=c"),
                &format!("suchlage_oidc={st}"),
            ),
        ] {
            let res = tower::ServiceExt::oneshot(app.router.clone(), req)
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::SEE_OTHER);
            assert_eq!(location(&res), "/anmelden?fehler=oidc");
            assert!(
                !set_cookies(&res)
                    .iter()
                    .any(|c| c.starts_with("suchlage_sid=")),
                "keine Sitzung"
            );
        }
        assert_eq!(idp.token_request_count(), 0, "der Code wird nie eingelöst");
    }

    #[tokio::test]
    async fn start_without_oidc_settings_sends_the_browser_back_to_login() {
        let app = test_app().await;
        let res = tower::ServiceExt::oneshot(app.router.clone(), get("/api/auth/oidc/start"))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::SEE_OTHER);
        assert_eq!(location(&res), "/anmelden?fehler=oidc");
        assert!(set_cookies(&res).is_empty());
    }

    /// Review Focus 4: ohne Feature `dev` gibt es keinen Login ohne OIDC.
    #[cfg(not(feature = "dev"))]
    #[tokio::test]
    async fn dev_login_absent_without_feature() {
        let app = test_app().await;
        let (status, _) = send(
            &app.router,
            post_json("/api/auth/dev-login", serde_json::json!({})),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (_, options) = send(&app.router, get("/api/auth/options")).await;
        assert_eq!(options["dev_login"], false);
    }

    #[cfg(feature = "dev")]
    #[tokio::test]
    async fn dev_login_creates_an_admin_session() {
        let app = test_app().await;
        let res = tower::ServiceExt::oneshot(
            app.router.clone(),
            post_json("/api/auth/dev-login", serde_json::json!({})),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let cookie = res.headers()["set-cookie"]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();
        let (_, me) = send(&app.router, with_cookie(get("/api/me"), &cookie)).await;
        assert_eq!(me["role"], "admin");
    }
}

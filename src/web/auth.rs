//! Anmeldung über HTTP: Optionen, Ersteinrichtung, eigene Sitzung, Abmelden, Großbild-Zugang,
//! Dev-Login (Feature `dev`).

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::Cookie;

use crate::app::AppState;
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
    };
    // Erst prüfen, dann den Token verbrauchen: Ein Tippfehler im Issuer kostet keinen neuen Link.
    crate::settings::validate_oidc(&input)?;
    crate::auth::setup::consume(&state.pool, &body.token, now_ms()).await?;
    state.settings.update_oidc(input).await?;
    Ok(Json(SetupResult {
        login_url: "/api/auth/oidc/start?next=/".into(),
    }))
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

    /// Ersteinrichtung über den Einmal-Link: Ein Tippfehler kostet den Link nicht, danach ist
    /// Suchlage eingerichtet und der Weg zu.
    #[cfg(not(feature = "dev"))]
    #[tokio::test]
    async fn setup_link_configures_oidc_exactly_once() {
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
                serde_json::json!({"token": token, "issuer": issuer, "client_id": "suchlage", "client_secret": "geheim"}),
            )
        };

        let (status, body) = send(&app.router, request("falsch", "https://id.example.org")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert!(
            body["error"].as_str().unwrap().contains("Einrichtungslink"),
            "{body}"
        );
        let (status, body) = send(&app.router, request(token, "id.example.org")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "Tippfehler im Issuer");
        assert!(body["error"].as_str().unwrap().contains("Issuer"), "{body}");

        let (status, body) = send(&app.router, request(token, "https://id.example.org/")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["login_url"], "/api/auth/oidc/start?next=/");
        let oidc = app.state.settings.current().oidc.unwrap();
        assert_eq!(oidc.issuer, "https://id.example.org");
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
            "https://id.example.org"
        );
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

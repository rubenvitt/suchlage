//! Verwaltung für Admins: Einstellungen und Großbild-Tokens. B14 ergänzt die Fernsteuerungen.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Admin;
use crate::contract::{
    BrokerSettingsInput, DjiSettingsInput, GeneralSettings, KioskTokenCreate, KioskTokenCreated,
    KioskTokenView, MediaSettings, OidcSettingsInput, SettingsView,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::store;
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(settings_view))
        .route("/settings/{section}", put(settings_update))
        .route("/kiosk-tokens", get(list_kiosk).post(create_kiosk))
        .route("/kiosk-tokens/{id}", delete(delete_kiosk))
}

async fn settings_view(_: Admin, State(state): State<AppState>) -> Json<SettingsView> {
    Json(state.settings.view(&state.boot))
}

/// Ein Abschnitt je Anfrage. Der Body kommt als freies JSON an und wird erst je Abschnitt gelesen:
/// Ein unbekannter Abschnitt meldet 404, nicht die Felder eines anderen.
async fn settings_update(
    _: Admin,
    State(state): State<AppState>,
    Path(section): Path<String>,
    JsonBody(body): JsonBody<serde_json::Value>,
) -> Result<Json<SettingsView>, AppError> {
    fn parse<T: serde::de::DeserializeOwned>(v: serde_json::Value) -> Result<T, AppError> {
        serde_json::from_value(v)
            .map_err(|e| AppError::Validation(format!("Ungültige Anfrage: {e}")))
    }
    let s = &state.settings;
    match section.as_str() {
        "oidc" => s.update_oidc(parse::<OidcSettingsInput>(body)?).await?,
        "dji" => s.update_dji(parse::<DjiSettingsInput>(body)?).await?,
        "broker" => s.update_broker(parse::<BrokerSettingsInput>(body)?).await?,
        "media" => s.update_media(parse::<MediaSettings>(body)?).await?,
        "general" => s.update_general(parse::<GeneralSettings>(body)?).await?,
        _ => return Err(AppError::NotFound),
    }
    Ok(Json(s.view(&state.boot)))
}

async fn list_kiosk(
    _: Admin,
    State(state): State<AppState>,
) -> Result<Json<Vec<KioskTokenView>>, AppError> {
    Ok(Json(store::kiosk::list(&state.pool).await?))
}

async fn create_kiosk(
    _: Admin,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<KioskTokenCreate>,
) -> Result<Json<KioskTokenCreated>, AppError> {
    let label = body.label.trim();
    if label.is_empty() || label.chars().count() > 80 {
        return Err(AppError::Validation("Bezeichnung: 1 bis 80 Zeichen".into()));
    }
    let (token, secret) = store::kiosk::create(&state.pool, label, now_ms()).await?;
    let url = format!(
        "{}/kiosk#{secret}",
        state.boot.public_url.as_str().trim_end_matches('/')
    );
    Ok(Json(KioskTokenCreated { token, url }))
}

async fn delete_kiosk(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    if store::kiosk::delete(&state.pool, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    use crate::contract::Role;
    use crate::testing::{get, post_json, put_json, send, test_app, with_cookie};

    #[tokio::test]
    async fn admin_maintains_settings_operator_may_not() {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        let op = app.cookie(Role::Operator).await;
        let body = serde_json::json!({"host": "mqtt", "port": 1883, "admin_username": "suchlage-admin", "admin_password": "pw", "rc_url": "tcp://100.64.0.10:1883"});
        assert_eq!(
            send(
                &app.router,
                with_cookie(put_json("/api/settings/broker", body.clone()), &op)
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            send(&app.router, with_cookie(get("/api/settings"), &op))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        let (status, view) = send(
            &app.router,
            with_cookie(put_json("/api/settings/broker", body), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["broker"]["has_admin_password"], true);
        assert!(view["broker"].get("admin_password").is_none());
        assert!(
            !view.to_string().contains("\"pw\""),
            "Geheimnis bleibt im Server"
        );
        let (status, view) = send(&app.router, with_cookie(get("/api/settings"), &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(view["broker"]["rc_url"], "tcp://100.64.0.10:1883");
        assert_eq!(view["general"]["auto_live_on_takeoff"], true);
        assert_eq!(view["oidc"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn unknown_section_is_404_and_bad_value_is_400() {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        assert_eq!(
            send(
                &app.router,
                with_cookie(
                    put_json("/api/settings/wetter", serde_json::json!({})),
                    &admin
                )
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
        let media = serde_json::json!({"webrtc_url": "x", "hls_url": "http://a", "playback_url": "http://b", "rtmp_url": "rtmp://c"});
        let (status, body) = send(
            &app.router,
            with_cookie(put_json("/api/settings/media", media), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(body["error"].as_str().unwrap().contains("WebRTC"), "{body}");
        let (status, body) = send(
            &app.router,
            with_cookie(
                put_json(
                    "/api/settings/general",
                    serde_json::json!({"auto_live_on_takeoff": true}),
                ),
                &admin,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "fehlendes Feld");
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .starts_with("Ungültige Anfrage: "),
            "{body}"
        );
    }

    #[tokio::test]
    async fn settings_need_a_session() {
        let app = test_app().await;
        assert_eq!(
            send(&app.router, get("/api/settings")).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn kiosk_label_is_checked_and_unknown_token_is_404() {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        for label in ["   ", &"x".repeat(81)] {
            let req = post_json("/api/kiosk-tokens", serde_json::json!({ "label": label }));
            let (status, body) = send(&app.router, with_cookie(req, &admin)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{label:?}");
            assert_eq!(body["error"], "Bezeichnung: 1 bis 80 Zeichen");
        }
        let req = post_json(
            "/api/kiosk-tokens",
            serde_json::json!({ "label": "  ELW 1  " }),
        );
        let (status, created) = send(&app.router, with_cookie(req, &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(created["token"]["label"], "ELW 1", "Bezeichnung ohne Rand");
        assert!(
            created["url"]
                .as_str()
                .unwrap()
                .starts_with("https://suchlage.test/kiosk#")
        );

        let req = Request::delete("/api/kiosk-tokens/999")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn kiosk_tokens_need_a_session() {
        let app = test_app().await;
        let req = Request::get("/api/kiosk-tokens")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&app.router, req).await.0, StatusCode::UNAUTHORIZED);
    }
}

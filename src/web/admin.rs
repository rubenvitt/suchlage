//! Verwaltung für Admins: Einstellungen, Großbild-Tokens und Fernsteuerungen.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, put};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Admin;
use crate::contract::{
    BrokerSettingsInput, DjiSettingsInput, GeneralSettings, KioskTokenCreate, KioskTokenCreated,
    KioskTokenView, MediaSettings, OidcSettingsInput, PhotoStorageSettingsInput,
    RemoteControllerCreate, RemoteControllerCreated, RemoteControllerView, SettingsView,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::situation::observation::{Observation, Observed};
use crate::store;
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/settings", get(settings_view))
        .route("/settings/{section}", put(settings_update))
        .route("/kiosk-tokens", get(list_kiosk).post(create_kiosk))
        .route("/kiosk-tokens/{id}", delete(delete_kiosk))
        .route("/remote-controllers", get(list_rcs).post(create_rc))
        .route("/remote-controllers/{id}", delete(delete_rc))
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
        "photo_storage" => {
            s.update_photo_storage(parse::<PhotoStorageSettingsInput>(body)?)
                .await?
        }
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

async fn list_rcs(
    _: Admin,
    State(state): State<AppState>,
) -> Result<Json<Vec<RemoteControllerView>>, AppError> {
    let all = store::remote_controllers::list(&state.pool).await?;
    Ok(Json(
        all.iter().map(store::remote_controllers::view).collect(),
    ))
}

async fn create_rc(
    _: Admin,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<RemoteControllerCreate>,
) -> Result<Json<RemoteControllerCreated>, AppError> {
    let label = body.label.trim();
    if label.is_empty() || label.chars().count() > 80 {
        return Err(AppError::Validation("Bezeichnung: 1 bis 80 Zeichen".into()));
    }
    let (rc, code) = store::remote_controllers::create(&state.pool, label, now_ms()).await?;
    // Fragment statt Query: Der Code läuft nicht ab und gäbe sonst aus jedem Access-Log
    // (Cloudflare, Traefik, TraceLayer) Lizenz und Broker-Zugang frei.
    let pairing_url = format!(
        "{}/pilot#{code}",
        state.boot.public_url.as_str().trim_end_matches('/')
    );
    Ok(Json(RemoteControllerCreated {
        controller: store::remote_controllers::view(&rc),
        pairing_url,
    }))
}

async fn delete_rc(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let rc = store::remote_controllers::list(&state.pool)
        .await?
        .into_iter()
        .find(|r| r.id == id)
        .ok_or(AppError::NotFound)?;
    // Erst beim Broker widerrufen; scheitert das, bleibt der Eintrag (kein stiller Restzugang).
    if let Some(sn) = &rc.sn {
        crate::broker::revoke_rc(&state.mqtt, sn).await?;
    }
    store::remote_controllers::delete(&state.pool, id).await?;
    let labels = store::remote_controllers::labels(&state.pool).await?;
    state
        .situation
        .observe(Observed {
            at_ms: now_ms(),
            observation: Observation::Labels(labels),
        })
        .await;
    Ok(StatusCode::NO_CONTENT)
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

    #[tokio::test]
    async fn rc_lifecycle_create_once_visible_code_then_revoke_at_broker() {
        let mut app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        let req = post_json(
            "/api/remote-controllers",
            serde_json::json!({"label": "  RC Plus 1  "}),
        );
        let (status, created) = send(&app.router, with_cookie(req, &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            created["controller"]["label"], "RC Plus 1",
            "Bezeichnung ohne Rand"
        );
        let url = created["pairing_url"].as_str().unwrap();
        assert!(url.starts_with("https://suchlage.test/pilot#"));
        let (_, list) = send(
            &app.router,
            with_cookie(get("/api/remote-controllers"), &admin),
        )
        .await;
        assert_eq!(list.as_array().unwrap().len(), 1);
        assert!(
            !list.to_string().contains(url.split_once('#').unwrap().1),
            "Code steht nur in der Antwort beim Anlegen"
        );

        let id = created["controller"]["id"].as_i64().unwrap();
        crate::store::remote_controllers::bind(&app.state.pool, id, "RC1", "pw", "k", 1)
            .await
            .unwrap();
        let (_, list) = send(
            &app.router,
            with_cookie(get("/api/remote-controllers"), &admin),
        )
        .await;
        assert_eq!(list[0]["sn"], "RC1");
        assert!(
            !list.to_string().contains("\"pw\""),
            "Zugang bleibt im Server"
        );
        let mut rx = std::mem::replace(&mut app.mqtt_requests, crate::mqtt::MqttLink::channel().1);
        let revoked = tokio::spawn(async move {
            let mut cmds = vec![];
            while let Ok(Some(req)) =
                tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
            {
                let v: serde_json::Value = serde_json::from_slice(&req.payload).unwrap();
                cmds.push(v["commands"][0]["command"].as_str().unwrap().to_string());
                let _ = req.reply.send(Ok(serde_json::json!({})));
            }
            cmds
        });
        let req = Request::delete(format!("/api/remote-controllers/{id}"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NO_CONTENT
        );
        assert_eq!(
            revoked.await.unwrap(),
            vec!["deleteClient", "deleteRole", "removeRoleACL"]
        );
        let (_, list) = send(
            &app.router,
            with_cookie(get("/api/remote-controllers"), &admin),
        )
        .await;
        assert_eq!(list, serde_json::json!([]));
    }

    #[tokio::test]
    async fn revocation_is_refused_when_the_broker_is_unreachable() {
        let app = test_app().await; // niemand beantwortet MQTT-Anfragen → Zeitüberschreitung
        let admin = app.cookie(Role::Admin).await;
        let (rc, _) = crate::store::remote_controllers::create(&app.state.pool, "RC", 0)
            .await
            .unwrap();
        crate::store::remote_controllers::bind(&app.state.pool, rc.id, "RC1", "pw", "k", 1)
            .await
            .unwrap();
        let req = Request::delete(format!("/api/remote-controllers/{}", rc.id))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(
            crate::store::remote_controllers::find_by_sn(&app.state.pool, "RC1")
                .await
                .unwrap()
                .is_some(),
            "nicht still gelöscht"
        );
    }

    #[tokio::test]
    async fn unpaired_rc_is_deleted_without_the_broker_and_label_is_checked() {
        let app = test_app().await; // ohne SN gibt es nichts zu widerrufen, MQTT bleibt unberührt
        let admin = app.cookie(Role::Admin).await;
        let op = app.cookie(Role::Operator).await;
        let (rc, _) = crate::store::remote_controllers::create(&app.state.pool, "RC", 0)
            .await
            .unwrap();
        let list = with_cookie(get("/api/remote-controllers"), &op);
        assert_eq!(send(&app.router, list).await.0, StatusCode::FORBIDDEN);
        for label in ["   ", &"x".repeat(81)] {
            let req = post_json(
                "/api/remote-controllers",
                serde_json::json!({ "label": label }),
            );
            let (status, body) = send(&app.router, with_cookie(req, &admin)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{label:?}");
            assert_eq!(body["error"], "Bezeichnung: 1 bis 80 Zeichen");
        }
        let req = Request::delete(format!("/api/remote-controllers/{}", rc.id))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NO_CONTENT
        );
        let req = Request::delete(format!("/api/remote-controllers/{}", rc.id))
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NOT_FOUND
        );
    }
}

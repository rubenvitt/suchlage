//! Öffentliche Pilot-Seite: Ohne gültigen Kopplungscode geben weder die Seite noch ihre zwei
//! Endpunkte etwas Geheimes heraus (Review Focus 1).

use axum::extract::State;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;

use crate::app::AppState;
use crate::contract::{DeviceLogEntry, DeviceLogLevel, DeviceLogSource};
use crate::dji::devlog::mask_passwords;
use crate::dji::pilot::{INDEX_HTML, PILOT_JS, PilotConnection, PilotLicense};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::runtime::streams::publish_url;
use crate::situation::observation::{Observation, Observed};
use crate::sn::is_valid_sn;
use crate::store::remote_controllers::{self as rcs, RemoteController};
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/pilot", get(|| async { asset("text/html", INDEX_HTML) }))
        .route(
            "/pilot/pilot.js",
            get(|| async { asset("text/javascript", PILOT_JS) }),
        )
        .route("/pilot/api/license", post(license))
        .route("/pilot/api/connect", post(connect))
        .route("/pilot/api/log", post(log))
}

fn asset(mime: &str, body: &'static str) -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, format!("{mime}; charset=utf-8")),
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        body,
    )
}

#[derive(Deserialize)]
struct LicenseRequest {
    code: String,
}

#[derive(Deserialize)]
struct ConnectRequest {
    code: String,
    rc_sn: String,
}

#[derive(Deserialize)]
struct LogRequest {
    code: String,
    lines: Vec<LogLine>,
}

#[derive(Deserialize)]
struct LogLine {
    /// Uhr der Fernsteuerung; weicht sie stark ab, gilt die Empfangszeit.
    at_ms: i64,
    level: DeviceLogLevel,
    text: String,
}

/// Mehr Zeilen schickt die Seite nie auf einmal.
const LOG_BATCH: usize = 50;
const LOG_LINE_CHARS: usize = 500;
const CLOCK_SKEW_MS: i64 = 10 * 60_000;

async fn by_code(state: &AppState, code: &str) -> Result<RemoteController, AppError> {
    let rejected = || {
        AppError::UnauthorizedWith(
            "Kopplungscode ungültig – in Suchlage unter Einstellungen → Fernsteuerungen einen neuen erzeugen"
                .into(),
        )
    };
    let code = code.trim();
    // Eine Datenbank aus der Entwicklung kann den festen Simulator-Code noch enthalten.
    if code.is_empty() || (!cfg!(feature = "dev") && code == "dev-simulator") {
        return Err(rejected());
    }
    rcs::find_by_code(&state.pool, code)
        .await?
        .ok_or_else(rejected)
}

async fn license(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<LicenseRequest>,
) -> Result<Json<PilotLicense>, AppError> {
    let rc = by_code(&state, &req.code).await?;
    let dji = state.settings.current().dji.ok_or_else(|| {
        AppError::Unavailable("DJI-App-Daten fehlen (Einstellungen → DJI)".into())
    })?;
    Ok(Json(PilotLicense {
        app_id: dji.app_id,
        app_key: dji.app_key,
        license: dji.license,
        workspace_id: dji.workspace_id,
        rc_label: rc.label,
    }))
}

async fn connect(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<ConnectRequest>,
) -> Result<Json<PilotConnection>, AppError> {
    if !is_valid_sn(&req.rc_sn) {
        return Err(AppError::Validation(
            "Seriennummer der Fernsteuerung ungültig".into(),
        ));
    }
    let rc = by_code(&state, &req.code).await?;
    let settings = state.settings.current();
    let broker = settings
        .broker
        .ok_or_else(|| AppError::Unavailable("Broker ist nicht eingerichtet".into()))?;
    let media = settings
        .media
        .ok_or_else(|| AppError::Unavailable("MediaMTX ist nicht eingerichtet".into()))?;
    let (password, stream_key) = credentials(&state, &rc, &req.rc_sn).await?;
    // Idempotent: legt nach einem Broker-Neuaufbau fehlende Zugänge wieder an.
    crate::broker::ensure_rc(&state.mqtt, &req.rc_sn, &password).await?;
    // Erst nach dem Binden: Ein Token gilt nur für eine gekoppelte Fernsteuerung.
    let api_token = crate::token::generate();
    rcs::set_api_token(&state.pool, rc.id, &api_token).await?;
    Ok(Json(PilotConnection {
        mqtt_host: broker.rc_url,
        mqtt_user: crate::broker::dynsec::rc_username(&req.rc_sn),
        mqtt_pass: password,
        rtmp_url: publish_url(&media, &req.rc_sn, &stream_key),
        api_host: state
            .boot
            .public_url
            .as_str()
            .trim_end_matches('/')
            .to_string(),
        ws_host: ws_url(&state.boot.public_url),
        api_token,
        media: settings.photo_storage.is_some(),
    }))
}

/// Protokollzeilen der Pilot-Seite ins Funkprotokoll: So sieht die Systemseite, was die JSBridge
/// meldet (Verbindung, Livebild), auch wenn MQTT gerade nicht läuft.
async fn log(
    State(state): State<AppState>,
    JsonBody(req): JsonBody<LogRequest>,
) -> Result<StatusCode, AppError> {
    if req.lines.len() > LOG_BATCH {
        return Err(AppError::Validation(format!(
            "Höchstens {LOG_BATCH} Zeilen je Anfrage"
        )));
    }
    let rc = by_code(&state, &req.code).await?;
    let now = now_ms();
    for line in req.lines {
        let text: String = line.text.chars().take(LOG_LINE_CHARS).collect();
        let summary = match &rc.sn {
            Some(_) => mask_passwords(&text),
            None => format!("{}: {}", rc.label, mask_passwords(&text)),
        };
        state.devlog.push(DeviceLogEntry {
            seq: 0,
            at_ms: if (line.at_ms - now).abs() < CLOCK_SKEW_MS {
                line.at_ms
            } else {
                now
            },
            source: DeviceLogSource::Pilot,
            level: line.level,
            device_sn: rc.sn.clone(),
            topic: None,
            method: None,
            summary,
            payload: None,
        });
    }
    Ok(StatusCode::NO_CONTENT)
}

/// `https://host` → `wss://host/pilot/ws`, `http://host` → `ws://host/pilot/ws`.
fn ws_url(public: &url::Url) -> String {
    let mut url = public.clone();
    let scheme = if public.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    url.set_scheme(scheme)
        .expect("http(s) und ws(s) sind beide besondere Schemata");
    url.set_path("/pilot/ws");
    url.set_query(None);
    url.set_fragment(None);
    url.to_string()
}

/// MQTT-Passwort und Stream-Schlüssel: gespeichert, wenn die SN schon gebunden ist, sonst beim
/// ersten Koppeln erzeugt. Ein Code gehört genau einer Fernsteuerung.
async fn credentials(
    state: &AppState,
    rc: &RemoteController,
    sn: &str,
) -> Result<(String, String), AppError> {
    match (&rc.sn, &rc.mqtt_password, &rc.stream_key) {
        (Some(bound), Some(p), Some(k)) if bound == sn => return Ok((p.clone(), k.clone())),
        (Some(_), _, _) => {
            return Err(AppError::Conflict(
                "Kopplungscode gehört zu einer anderen Fernsteuerung".into(),
            ));
        }
        _ => {}
    }
    let (password, stream_key) = (crate::token::generate(), crate::token::generate());
    if !rcs::bind(&state.pool, rc.id, sn, &password, &stream_key, now_ms()).await? {
        return Err(AppError::Conflict(
            "Die Fernsteuerung wurde gerade gekoppelt – bitte erneut verbinden".into(),
        ));
    }
    let labels = rcs::labels(&state.pool).await?;
    state
        .situation
        .observe(Observed {
            at_ms: now_ms(),
            observation: Observation::Labels(labels),
        })
        .await;
    Ok((password, stream_key))
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::testing::{TestApp, post_json, send, test_app};

    async fn configured(app: &TestApp) -> String {
        let s = &app.state.settings;
        s.update_dji(crate::contract::DjiSettingsInput {
            app_id: "APPID".into(),
            app_key: Some("APPKEY-GEHEIM".into()),
            license: Some("LIZENZ-GEHEIM".into()),
            workspace_id: "768d4a2e-1f34-477c-9fcc-872285e895f6".into(),
        })
        .await
        .unwrap();
        s.update_broker(crate::contract::BrokerSettingsInput {
            host: "mqtt".into(),
            port: 1883,
            admin_username: "a".into(),
            admin_password: Some("p".into()),
            rc_url: "tcp://100.64.0.10:1883".into(),
        })
        .await
        .unwrap();
        s.update_media(crate::contract::MediaSettings {
            webrtc_url: "http://m:8889".into(),
            hls_url: "http://m:8888".into(),
            playback_url: "http://m:9996".into(),
            rtmp_url: "rtmp://100.64.0.10:1935".into(),
        })
        .await
        .unwrap();
        let (_, code) = crate::store::remote_controllers::create(&app.state.pool, "RC Plus 1", 0)
            .await
            .unwrap();
        code
    }

    /// Beantwortet jeden dynsec-Befehl ohne Fehler.
    fn broker_ok(app: &mut TestApp) {
        let mut rx = std::mem::replace(&mut app.mqtt_requests, crate::mqtt::MqttLink::channel().1);
        tokio::spawn(async move {
            while let Some(req) = rx.recv().await {
                let _ = req.reply.send(Ok(json!({"command": "x"})));
            }
        });
    }

    /// Review Focus 1: ohne gültigen Code nichts Geheimes, auch nicht in der Fehlermeldung – an beiden Endpunkten.
    #[tokio::test]
    async fn endpoints_without_valid_code_return_no_secrets() {
        let app = test_app().await;
        configured(&app).await;
        if !cfg!(feature = "dev") {
            crate::store::remote_controllers::create_with_code(
                &app.state.pool,
                "Alt",
                "dev-simulator",
                0,
            )
            .await
            .unwrap();
        }
        let bodies = [
            json!({"code": "falsch", "rc_sn": "RC1"}),
            json!({"code": "", "rc_sn": "RC1"}),
            json!({"rc_sn": "RC1"}),
            json!({"code": "dev-simulator", "rc_sn": "RC1"}),
        ];
        for path in ["/pilot/api/license", "/pilot/api/connect"] {
            for body in &bodies {
                let (status, res) = send(&app.router, post_json(path, body.clone())).await;
                assert!(
                    status == StatusCode::UNAUTHORIZED || status == StatusCode::BAD_REQUEST,
                    "{path}: {status}"
                );
                let text = res.to_string();
                for secret in [
                    "APPKEY-GEHEIM",
                    "LIZENZ-GEHEIM",
                    "appKey",
                    "mqttPass",
                    "rtmp://",
                ] {
                    assert!(!text.contains(secret), "{path}: {secret} in {text}");
                }
            }
        }
    }

    #[tokio::test]
    async fn license_step_carries_no_broker_or_video_access() {
        let app = test_app().await;
        let code = configured(&app).await;
        let (status, lic) = send(
            &app.router,
            post_json("/pilot/api/license", json!({"code": code})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{lic}");
        assert_eq!(
            (lic["appKey"].as_str(), lic["rcLabel"].as_str()),
            (Some("APPKEY-GEHEIM"), Some("RC Plus 1"))
        );
        let text = lic.to_string();
        assert!(!text.contains("mqtt") && !text.contains("rtmp"), "{text}");
    }

    #[tokio::test]
    async fn valid_code_binds_the_rc_and_returns_its_own_credentials() {
        let mut app = test_app().await;
        let code = configured(&app).await;
        broker_ok(&mut app);
        let connect =
            |sn: &str| post_json("/pilot/api/connect", json!({"code": code, "rc_sn": sn}));
        let (status, cfg) = send(&app.router, connect("RC1")).await;
        assert_eq!(status, StatusCode::OK, "{cfg}");
        assert_eq!(cfg["mqttUser"], "rc-RC1");
        assert_eq!(cfg["mqttHost"], "tcp://100.64.0.10:1883");
        assert!(cfg.get("appKey").is_none(), "Lizenz nur im ersten Schritt");
        assert!(
            cfg["rtmpUrl"]
                .as_str()
                .unwrap()
                .starts_with("rtmp://100.64.0.10:1935/live/RC1?user=rc-RC1&pass=")
        );
        let pass = cfg["mqttPass"].as_str().unwrap().to_string();
        assert_eq!(cfg["apiHost"], "https://suchlage.test");
        assert_eq!(cfg["wsHost"], "wss://suchlage.test/pilot/ws");
        assert_eq!(cfg["media"], false, "ohne Fotospeicher kein Modul media");
        let first_token = cfg["apiToken"].as_str().unwrap().to_string();
        let rc_of = |token: String| {
            let pool = app.state.pool.clone();
            async move {
                crate::store::remote_controllers::find_by_api_token(&pool, &token)
                    .await
                    .unwrap()
                    .map(|rc| rc.label)
            }
        };
        assert_eq!(
            rc_of(first_token.clone()).await.as_deref(),
            Some("RC Plus 1")
        );

        let (_, again) = send(&app.router, connect("RC1")).await;
        assert_eq!(
            again["mqttPass"],
            pass.as_str(),
            "stabil über Neustarts von Pilot 2"
        );
        assert_eq!(rc_of(first_token).await, None, "neues Koppeln, neuer Token");
        assert!(
            rc_of(again["apiToken"].as_str().unwrap().into())
                .await
                .is_some()
        );
        app.state
            .settings
            .update_photo_storage(crate::contract::PhotoStorageSettingsInput {
                s3_url: "http://fotos:3900".into(),
                rc_url: "http://100.64.0.10:3900".into(),
                admin_url: "http://fotos:3903".into(),
                bucket: "suchlage-fotos".into(),
                region: "garage".into(),
                access_key_id: "GK1".into(),
                secret_access_key: Some("s".into()),
                admin_token: Some("t".into()),
            })
            .await
            .unwrap();
        let (_, with_photos) = send(&app.router, connect("RC1")).await;
        assert_eq!(with_photos["media"], true);
        let (status, _) = send(&app.router, connect("RC2")).await;
        assert_eq!(status, StatusCode::CONFLICT, "Code gehört zu RC1");
        let bound = crate::store::remote_controllers::find_by_sn(&app.state.pool, "RC1")
            .await
            .unwrap();
        assert!(bound.is_some(), "RC1 bleibt gekoppelt");
    }

    #[tokio::test]
    async fn pilot_log_lines_reach_the_device_log_only_with_a_valid_code() {
        let app = test_app().await;
        let code = configured(&app).await;
        let line =
            |text: &str| json!({"at_ms": crate::time::now_ms(), "level": "warn", "text": text});
        let (status, _) = send(
            &app.router,
            post_json(
                "/pilot/api/log",
                json!({"code": "falsch", "lines": [line("x")]}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = send(
            &app.router,
            post_json(
                "/pilot/api/log",
                json!({"code": code, "lines": [line("Verbindung zu Suchlage: getrennt"), line("url pass=GEHEIM")]}),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (history, _) = app.state.devlog.subscribe();
        let texts: Vec<_> = history.iter().map(|e| e.summary.as_str()).collect();
        assert_eq!(
            texts,
            [
                "RC Plus 1: Verbindung zu Suchlage: getrennt",
                "RC Plus 1: url pass=***"
            ]
        );
        assert_eq!(history[0].source, crate::contract::DeviceLogSource::Pilot);
        let many: Vec<_> = (0..51).map(|_| line("x")).collect();
        let (status, _) = send(
            &app.router,
            post_json("/pilot/api/log", json!({"code": code, "lines": many})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn invalid_serial_number_is_rejected_before_anything_else() {
        let app = test_app().await;
        let code = configured(&app).await;
        let (status, _) = send(
            &app.router,
            post_json("/pilot/api/connect", json!({"code": code, "rc_sn": "../x"})),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn page_is_served_without_secrets() {
        let app = test_app().await;
        configured(&app).await;
        let (status, html) = send(&app.router, crate::testing::get("/pilot")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(!html.to_string().contains("APPKEY-GEHEIM"));
        let (status, js) = send(&app.router, crate::testing::get("/pilot/pilot.js")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(js.as_str().unwrap().contains("export async function pair"));
    }
}

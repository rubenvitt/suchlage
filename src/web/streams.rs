//! Livebild-Befehle der Bedienung. Der Ablauf steht in `runtime::streams`.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Operator;
use crate::contract::{StreamLensRequest, StreamStartRequest, StreamState, StreamStopRequest};
use crate::error::AppError;
use crate::extract::JsonBody;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/streams/start", post(start))
        .route("/streams/stop", post(stop))
        .route("/streams/lens", post(lens))
}

async fn start(
    _: Operator,
    State(s): State<AppState>,
    JsonBody(b): JsonBody<StreamStartRequest>,
) -> Result<Json<StreamState>, AppError> {
    Ok(Json(s.streams.start(&b.gateway_sn, b.video_id).await?))
}

async fn stop(
    _: Operator,
    State(s): State<AppState>,
    JsonBody(b): JsonBody<StreamStopRequest>,
) -> Result<Json<StreamState>, AppError> {
    Ok(Json(s.streams.stop(&b.gateway_sn).await?))
}

async fn lens(
    _: Operator,
    State(s): State<AppState>,
    JsonBody(b): JsonBody<StreamLensRequest>,
) -> Result<Json<StreamState>, AppError> {
    Ok(Json(s.streams.lens(&b.gateway_sn, b.lens).await?))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use serde_json::{Value, json};
    use tokio::sync::mpsc;

    use crate::contract::{Lens, MediaSettings, Role, StreamPhase, examples};
    use crate::mqtt::{LinkError, LinkRequest};
    use crate::situation::observation::{Observation, Observed};
    use crate::testing::{TestApp, post_json, send, test_app, with_cookie};

    const WAIT: Duration = Duration::from_secs(5);

    /// Fernsteuerung meldet ein Fluggerät mit seinen Videoquellen.
    async fn connect(app: &TestApp) {
        let s = &app.state.situation;
        s.observe(Observed {
            at_ms: 1,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: Some("AC1".into()),
            },
        })
        .await;
        s.observe(Observed {
            at_ms: 2,
            observation: Observation::VideoSources {
                gateway_sn: "RC1".into(),
                sources: examples::video_sources(),
            },
        })
        .await;
    }

    async fn pair(app: &TestApp) {
        let pool = &app.state.pool;
        let (rc, _) = crate::store::remote_controllers::create(pool, "RC Plus 1", 0)
            .await
            .unwrap();
        crate::store::remote_controllers::bind(pool, rc.id, "RC1", "pw", "streamkey", 1)
            .await
            .unwrap();
    }

    async fn configure_media(app: &TestApp) {
        app.state
            .settings
            .update_media(MediaSettings {
                webrtc_url: "http://media:8889".into(),
                hls_url: "http://media:8888".into(),
                playback_url: "http://media:9996".into(),
                rtmp_url: "rtmp://100.64.0.10:1935".into(),
            })
            .await
            .unwrap();
    }

    /// Fernsteuerung gekoppelt, Fluggerät mit Quellen gemeldet, MediaMTX eingerichtet; liefert
    /// das Cookie einer Bedienung.
    async fn ready(app: &TestApp) -> String {
        pair(app).await;
        configure_media(app).await;
        connect(app).await;
        operator(app).await
    }

    async fn operator(app: &TestApp) -> String {
        app.cookie(Role::Admin).await;
        app.cookie(Role::Operator).await
    }

    fn result(code: i64) -> Result<Value, LinkError> {
        Ok(json!({"method": "x", "result": code}))
    }

    /// Nimmt den nächsten MQTT-Befehl an, beantwortet ihn und liefert Topic und Nutzlast.
    async fn answer_next(
        rx: &mut mpsc::Receiver<LinkRequest>,
        reply: Result<Value, LinkError>,
    ) -> (String, Value) {
        let req = tokio::time::timeout(WAIT, rx.recv())
            .await
            .expect("kein MQTT-Befehl gesendet")
            .expect("Kanal offen");
        let _ = req.reply.send(reply);
        (req.topic, serde_json::from_slice(&req.payload).unwrap())
    }

    /// POST an die API; den MQTT-Befehl, den die Anfrage auslöst, beantwortet `reply`.
    async fn post_answered(
        app: &mut TestApp,
        cookie: &str,
        uri: &str,
        body: Value,
        reply: Result<Value, LinkError>,
    ) -> ((StatusCode, Value), (String, Value)) {
        let request = with_cookie(post_json(uri, body), cookie);
        tokio::join!(
            send(&app.router, request),
            answer_next(&mut app.mqtt_requests, reply)
        )
    }

    async fn post_refused(app: &mut TestApp, cookie: &str, uri: &str, body: Value) -> StatusCode {
        let (status, body) = send(&app.router, with_cookie(post_json(uri, body), cookie)).await;
        assert!(body["error"].is_string(), "{body}");
        assert!(
            app.mqtt_requests.try_recv().is_err(),
            "abgelehnt, ohne einen Befehl an Pilot 2 zu senden"
        );
        status
    }

    fn start_default() -> Value {
        json!({"gateway_sn": "RC1", "video_id": null})
    }

    async fn phase(app: &TestApp) -> StreamPhase {
        app.state
            .situation
            .stream("RC1")
            .await
            .unwrap()
            .unwrap()
            .phase
    }

    #[tokio::test]
    async fn start_sends_live_start_push_with_rtmp_target_and_reports_live() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let ((status, body), (topic, payload)) = post_answered(
            &mut app,
            &op,
            "/api/streams/start",
            start_default(),
            result(0),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["phase"], "live");
        assert_eq!(topic, "thing/product/RC1/services");
        assert_eq!(payload["method"], "live_start_push");
        assert_eq!(
            payload["data"]["video_id"],
            format!("{}/61-0-0/normal-0", examples::AIRCRAFT_SN),
            "Standard: erste Nutzlastkamera"
        );
        assert_eq!(
            payload["data"]["url"],
            "rtmp://100.64.0.10:1935/live/RC1?user=rc-RC1&pass=streamkey"
        );
    }

    #[tokio::test]
    async fn chosen_video_id_is_sent_unchanged() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let fpv = format!("{}/39-0-7/normal-0", examples::AIRCRAFT_SN);
        let body = json!({"gateway_sn": "RC1", "video_id": fpv});
        let ((status, _), (_, payload)) =
            post_answered(&mut app, &op, "/api/streams/start", body, result(0)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(payload["data"]["video_id"], fpv);
    }

    #[tokio::test]
    async fn dji_rejection_is_422_and_stream_fails() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let ((status, body), _) = post_answered(
            &mut app,
            &op,
            "/api/streams/start",
            start_default(),
            result(513011),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert!(body["error"].as_str().unwrap().contains("513011"));
        assert_eq!(phase(&app).await, StreamPhase::Failed);
    }

    #[tokio::test]
    async fn unreachable_broker_is_503_and_stream_fails() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let ((status, body), _) = post_answered(
            &mut app,
            &op,
            "/api/streams/start",
            start_default(),
            Err(LinkError::NotConnected),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(body["error"].as_str().unwrap().contains("Broker"), "{body}");
        let stream = app.state.situation.stream("RC1").await.unwrap().unwrap();
        assert_eq!(
            stream.phase,
            StreamPhase::Failed,
            "nicht hängend in starting"
        );
        assert!(
            !stream.error.unwrap().contains("streamkey"),
            "kein Schlüssel im Fehlertext"
        );
    }

    #[tokio::test]
    async fn unknown_gateway_and_foreign_video_id_are_refused() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let unknown = json!({"gateway_sn": "RCX", "video_id": null});
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/start", unknown).await,
            StatusCode::CONFLICT
        );
        let foreign = json!({"gateway_sn": "RC1", "video_id": "AC1/99-0-0/normal-0"});
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/start", foreign).await,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }

    #[tokio::test]
    async fn gateway_without_aircraft_or_sources_is_refused() {
        let mut app = test_app().await;
        pair(&app).await;
        configure_media(&app).await;
        let op = operator(&app).await;
        let s = app.state.situation.clone();
        s.observe(Observed {
            at_ms: 1,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: None,
            },
        })
        .await;
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/start", start_default()).await,
            StatusCode::CONFLICT,
            "Fluggerät aus"
        );
        s.observe(Observed {
            at_ms: 2,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: Some("AC1".into()),
            },
        })
        .await;
        let (status, body) = send(
            &app.router,
            with_cookie(post_json("/api/streams/start", start_default()), &op),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body["error"],
            "Keine Videoquelle gemeldet – ist das Fluggerät eingeschaltet?"
        );
    }

    #[tokio::test]
    async fn unpaired_gateway_and_missing_media_are_refused() {
        let mut app = test_app().await;
        connect(&app).await;
        let op = operator(&app).await;
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/start", start_default()).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "MediaMTX nicht eingerichtet"
        );
        configure_media(&app).await;
        let (status, body) = send(
            &app.router,
            with_cookie(post_json("/api/streams/start", start_default()), &op),
        )
        .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(body["error"], "Fernsteuerung ist nicht gekoppelt");
    }

    #[tokio::test]
    async fn second_start_while_live_is_refused_and_keeps_the_stream() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        post_answered(
            &mut app,
            &op,
            "/api/streams/start",
            start_default(),
            result(0),
        )
        .await;
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/start", start_default()).await,
            StatusCode::CONFLICT
        );
        assert_eq!(phase(&app).await, StreamPhase::Live);
    }

    #[tokio::test]
    async fn stop_and_lens_address_the_running_video() {
        let mut app = test_app().await;
        let op = ready(&app).await;
        let rc1 = json!({"gateway_sn": "RC1"});
        assert_eq!(
            post_refused(&mut app, &op, "/api/streams/stop", rc1.clone()).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            "es läuft kein Livebild"
        );
        post_answered(
            &mut app,
            &op,
            "/api/streams/start",
            start_default(),
            result(0),
        )
        .await;
        let video_id = format!("{}/61-0-0/normal-0", examples::AIRCRAFT_SN);

        let lens = json!({"gateway_sn": "RC1", "lens": "thermal"});
        let ((status, body), (_, payload)) =
            post_answered(&mut app, &op, "/api/streams/lens", lens, result(0)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["lens"], "thermal");
        assert_eq!(payload["method"], "live_lens_change");
        assert_eq!(
            payload["data"],
            json!({"video_id": video_id, "video_type": "thermal"})
        );

        let wide = json!({"gateway_sn": "RC1", "lens": "wide"});
        let ((status, _), _) =
            post_answered(&mut app, &op, "/api/streams/lens", wide, result(513012)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let stream = app.state.situation.stream("RC1").await.unwrap().unwrap();
        assert_eq!(
            (stream.phase, stream.lens),
            (StreamPhase::Live, Some(Lens::Thermal)),
            "abgelehnter Linsenwechsel lässt das Bild laufen"
        );

        let ((status, body), (_, payload)) =
            post_answered(&mut app, &op, "/api/streams/stop", rc1, result(0)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["phase"], "idle");
        assert_eq!(payload["method"], "live_stop_push");
        assert_eq!(payload["data"], json!({"video_id": video_id}));
    }

    #[tokio::test]
    async fn kiosk_cannot_send_commands() {
        let mut app = test_app().await;
        ready(&app).await;
        let kiosk = app.cookie(Role::Kiosk).await;
        assert_eq!(
            post_refused(&mut app, &kiosk, "/api/streams/start", start_default()).await,
            StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn auto_request_after_takeoff_starts_the_default_stream() {
        let mut app = test_app().await;
        ready(&app).await;
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(crate::runtime::streams::run_auto(
            rx,
            app.state.streams.clone(),
        ));
        tx.send("RC1".into()).unwrap();
        let (_, payload) = answer_next(&mut app.mqtt_requests, result(0)).await;
        assert_eq!(payload["method"], "live_start_push");
    }
}

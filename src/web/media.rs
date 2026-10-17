//! Video-Routen: Auth-Hook für MediaMTX und die Proxies für den Browser (ADR 0003).

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Viewer;
use crate::dji::topic::is_valid_sn;
use crate::error::AppError;
use crate::media::{hook, proxy};
use crate::store;

/// Unter `/api`.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/video/{gateway_sn}/whep", post(whep))
        .route("/video/{gateway_sn}/hls/{*file}", get(hls))
}

/// Außerhalb von `/api`; in Traefik nicht nach außen routen (ADR 0003).
pub fn internal_routes() -> Router<AppState> {
    Router::new().route("/internal/media/auth", post(auth_hook))
}

async fn auth_hook(
    State(state): State<AppState>,
    Json(req): Json<hook::AuthRequest>,
) -> StatusCode {
    let key = match (req.action.as_str(), req.gateway_sn()) {
        ("publish", Some(sn)) => store::remote_controllers::find_by_sn(&state.pool, sn)
            .await
            .ok()
            .flatten()
            .and_then(|rc| rc.stream_key),
        _ => None,
    };
    if hook::decide(
        &req,
        &state.settings.current().internal_secret,
        key.as_deref(),
    ) {
        StatusCode::OK
    } else {
        tracing::debug!(action = %req.action, path = %req.path, protocol = %req.protocol, "MediaMTX-Zugriff abgelehnt");
        StatusCode::UNAUTHORIZED
    }
}

async fn whep(
    _: Viewer,
    State(state): State<AppState>,
    Path(sn): Path<String>,
    body: Bytes,
) -> Result<Response, AppError> {
    if !is_valid_sn(&sn) {
        return Err(AppError::NotFound);
    }
    proxy::whep(&state, &sn, body).await
}

async fn hls(
    _: Viewer,
    State(state): State<AppState>,
    Path((sn, file)): Path<(String, String)>,
    RawQuery(query): RawQuery,
) -> Result<Response, AppError> {
    if !is_valid_sn(&sn) {
        return Err(AppError::NotFound);
    }
    proxy::hls(&state, &sn, &file, query.as_deref().unwrap_or("")).await
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use axum::body::Body;
    use axum::http::{HeaderMap, Request, StatusCode};
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::contract::Role;
    use crate::testing::{TestApp, get, post_json, send, test_app, with_cookie};

    /// MediaMTX-Double: WHEP, HLS mit Cookie-Check-Umleitung, Playback /list und /get.
    async fn fake_mediamtx(seen_auth: Arc<Mutex<Vec<String>>>) -> String {
        let s1 = seen_auth.clone();
        let s2 = seen_auth;
        let record = |h: &HeaderMap, seen: &Arc<Mutex<Vec<String>>>| {
            seen.lock().unwrap().push(
                h.get("authorization")
                    .map(|v| v.to_str().unwrap().to_string())
                    .unwrap_or_default(),
            );
        };
        let app = axum::Router::new()
            .route(
                "/live/RC1/whep",
                axum::routing::post(move |h: HeaderMap, body: String| {
                    let s = s1.clone();
                    async move {
                        record(&h, &s);
                        assert!(body.starts_with("v=0"));
                        (
                            StatusCode::CREATED,
                            [
                                ("content-type", "application/sdp"),
                                ("location", "/live/RC1/whep/abc"),
                            ],
                            "v=0 answer",
                        )
                            .into_response()
                    }
                }),
            )
            .route(
                "/live/RC1/index.m3u8",
                axum::routing::get(move |h: HeaderMap, q: axum::extract::RawQuery| {
                    let s = s2.clone();
                    async move {
                        record(&h, &s);
                        if q.0.as_deref() != Some("cookieCheck=1") {
                            return (
                                StatusCode::FOUND,
                                [
                                    ("location", "/live/RC1/index.m3u8?cookieCheck=1"),
                                    ("set-cookie", "cookieCheck=1; Secure"),
                                ],
                            )
                                .into_response();
                        }
                        (
                            [("content-type", "application/vnd.apple.mpegurl")],
                            "#EXTM3U\nmain_stream.m3u8?session=x\n",
                        )
                            .into_response()
                    }
                }),
            )
            // Ein falsch eingetragenes internes Geheimnis: MediaMTX lehnt den Proxy ab.
            .route(
                "/live/RC2/index.m3u8",
                axum::routing::get(|| async { StatusCode::UNAUTHORIZED }),
            )
            // Aufnahme beginnt 30 s nach dem Flugstart (Auto-Livebild nach dem Abheben).
            .route(
                "/list",
                axum::routing::get(|| async {
                    axum::Json(
                        serde_json::json!([{"start": "2026-10-16T11:27:10Z", "duration": 1050.0}]),
                    )
                }),
            )
            .route(
                "/get",
                axum::routing::get(|h: HeaderMap, q: axum::extract::RawQuery| async move {
                    let range = h
                        .get("range")
                        .map(|v| v.to_str().unwrap().to_string())
                        .unwrap_or_default();
                    (
                        [("content-type", "video/mp4"), ("accept-ranges", "bytes")],
                        format!("MP4 {} range={range}", q.0.unwrap_or_default()),
                    )
                        .into_response()
                }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        base
    }

    async fn with_media(app: &TestApp, base: &str) {
        app.state
            .settings
            .update_media(crate::contract::MediaSettings {
                webrtc_url: base.into(),
                hls_url: base.into(),
                playback_url: base.into(),
                rtmp_url: "rtmp://127.0.0.1:1935".into(),
            })
            .await
            .unwrap();
    }

    /// Adresse, an der sicher niemand lauscht: Port gebunden und gleich wieder freigegeben.
    async fn closed_port() -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        drop(listener);
        base
    }

    #[tokio::test]
    async fn hook_allows_the_rc_publish_and_the_internal_reader() {
        let app = test_app().await;
        let (rc, _) = crate::store::remote_controllers::create(&app.state.pool, "RC", 0)
            .await
            .unwrap();
        crate::store::remote_controllers::bind(&app.state.pool, rc.id, "RC1", "pw", "key", 1)
            .await
            .unwrap();
        let ok = serde_json::json!({"action": "publish", "path": "live/RC1", "protocol": "rtmp", "query": "user=rc-RC1&pass=key", "user": "", "password": ""});
        assert_eq!(
            send(&app.router, post_json("/internal/media/auth", ok))
                .await
                .0,
            StatusCode::OK
        );
        let bad = serde_json::json!({"action": "publish", "path": "live/RC1", "user": "rc-RC1", "password": "nein"});
        assert_eq!(
            send(&app.router, post_json("/internal/media/auth", bad))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let unknown = serde_json::json!({"action": "publish", "path": "live/RC9", "user": "rc-RC9", "password": "key"});
        assert_eq!(
            send(&app.router, post_json("/internal/media/auth", unknown))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
        let secret = app.state.settings.current().internal_secret;
        let read = serde_json::json!({"action": "read", "path": "live/RC1", "user": "suchlage-internal", "password": secret});
        assert_eq!(
            send(&app.router, post_json("/internal/media/auth", read))
                .await
                .0,
            StatusCode::OK
        );
    }

    #[tokio::test]
    async fn whep_is_proxied_with_internal_credentials_and_kiosk_may_watch() {
        let seen = Arc::new(Mutex::new(vec![]));
        let base = fake_mediamtx(seen.clone()).await;
        let app = test_app().await;
        with_media(&app, &base).await;
        let (_, secret) = crate::store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        let tid = crate::store::kiosk::find_by_token(&app.state.pool, &secret)
            .await
            .unwrap()
            .unwrap();
        let kiosk = format!(
            "suchlage_sid={}",
            crate::store::sessions::create_for_kiosk(&app.state.pool, tid, crate::time::now_ms())
                .await
                .unwrap()
        );

        let req = Request::post("/api/video/RC1/whep")
            .header("content-type", "application/sdp")
            .body(Body::from("v=0 offer"))
            .unwrap();
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(req, &kiosk))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        assert_eq!(res.headers()["content-type"], "application/sdp");
        assert!(
            res.headers().get("location").is_none(),
            "interne Adresse bleibt intern"
        );
        assert_eq!(
            res.into_body().collect().await.unwrap().to_bytes(),
            "v=0 answer"
        );
        let header = seen.lock().unwrap()[0].clone();
        assert!(header.starts_with("Basic "), "{header}");

        let unauth = Request::post("/api/video/RC1/whep")
            .body(Body::from("v=0"))
            .unwrap();
        assert_eq!(
            app.router.clone().oneshot(unauth).await.unwrap().status(),
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn hls_follows_the_cookie_check_and_strips_cookies() {
        let base = fake_mediamtx(Arc::new(Mutex::new(vec![]))).await;
        let app = test_app().await;
        with_media(&app, &base).await;
        app.cookie(Role::Admin).await;
        let c = app.cookie(Role::Operator).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(get("/api/video/RC1/hls/index.m3u8"), &c))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert!(res.headers().get("set-cookie").is_none());
        let body = res.into_body().collect().await.unwrap().to_bytes();
        assert!(String::from_utf8_lossy(&body).contains("main_stream.m3u8?session=x"));
    }

    #[tokio::test]
    async fn rejected_internal_reader_is_a_server_fault_not_a_logout() {
        let base = fake_mediamtx(Arc::new(Mutex::new(vec![]))).await;
        let app = test_app().await;
        with_media(&app, &base).await;
        let c = app.cookie(Role::Admin).await;
        let (status, body) = send(
            &app.router,
            with_cookie(get("/api/video/RC2/hls/index.m3u8"), &c),
        )
        .await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "ein 401 von MediaMTX hieße für den Browser „abgemeldet“"
        );
        assert_eq!(body["error"], "Videoserver lehnt den Zugang des Servers ab");
    }

    #[test]
    fn hls_file_names_cannot_escape_the_stream_directory() {
        assert!(crate::media::proxy::safe_file("main_stream.m3u8"));
        assert!(crate::media::proxy::safe_file("seg_12.mp4"));
        assert!(!crate::media::proxy::safe_file("../secret"));
        assert!(!crate::media::proxy::safe_file("a//b"));
        assert!(!crate::media::proxy::safe_file("x?y"));
    }

    #[tokio::test]
    async fn landed_flight_has_video_from_the_playback_window() {
        let base = fake_mediamtx(Arc::new(Mutex::new(vec![]))).await;
        let app = test_app().await;
        with_media(&app, &base).await;
        let flight = crate::contract::examples::flight_landed(4, 1_792_150_000_000);
        crate::store::flights::insert(&app.state.pool, &flight)
            .await
            .unwrap();
        let admin = app.cookie(Role::Admin).await;
        let (_, detail) = send(&app.router, with_cookie(get("/api/flights/4"), &admin)).await;
        assert_eq!(detail["video"]["play_url"], "/api/flights/4/video");
        assert_eq!(
            detail["video"]["download_url"],
            "/api/flights/4/video?download=1"
        );
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(get("/api/flights/4/video?download=1"), &admin))
            .await
            .unwrap();
        assert!(
            res.headers()["content-disposition"]
                .to_str()
                .unwrap()
                .contains("attachment")
        );
        let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        assert!(
            body.contains("path=live%2FRCSIM0001") && body.contains("format=mp4"),
            "{body}"
        );
        assert!(
            body.contains("start=2026-10-16T11%3A27%3A10Z") && body.contains("duration=1050"),
            "beginnt mit der Aufnahme, nicht mit dem Flug: {body}"
        );

        let ranged = Request::get("/api/flights/4/video")
            .header("range", "bytes=100-")
            .body(Body::empty())
            .unwrap();
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(ranged, &admin))
            .await
            .unwrap();
        assert_eq!(res.headers()["accept-ranges"], "bytes");
        assert_eq!(res.headers()["content-disposition"], "inline");
        let body = String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec())
            .unwrap();
        assert!(
            body.contains("range=bytes=100-"),
            "Range geht an MediaMTX: {body}"
        );
    }

    #[tokio::test]
    async fn unreachable_mediamtx_keeps_the_detail_and_answers_the_video_with_503() {
        let app = test_app().await;
        with_media(&app, &closed_port().await).await;
        crate::store::flights::insert(
            &app.state.pool,
            &crate::contract::examples::flight_landed(4, 1_792_150_000_000),
        )
        .await
        .unwrap();
        let admin = app.cookie(Role::Admin).await;
        let (status, detail) = send(&app.router, with_cookie(get("/api/flights/4"), &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["video"], serde_json::Value::Null);
        let (status, body) = send(
            &app.router,
            with_cookie(get("/api/flights/4/video"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(body["error"], "Videoserver nicht erreichbar");
        let (status, _) = send(
            &app.router,
            with_cookie(get("/api/video/RC1/hls/index.m3u8"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn open_flight_has_no_video_yet() {
        let base = fake_mediamtx(Arc::new(Mutex::new(vec![]))).await;
        let app = test_app().await;
        with_media(&app, &base).await;
        let mut flight = crate::contract::examples::flight_landed(5, 1_792_150_000_000);
        flight.ended_at_ms = None;
        flight.end_reason = None;
        crate::store::flights::insert(&app.state.pool, &flight)
            .await
            .unwrap();
        let admin = app.cookie(Role::Admin).await;
        let (_, detail) = send(&app.router, with_cookie(get("/api/flights/5"), &admin)).await;
        assert_eq!(detail["video"], serde_json::Value::Null);
        let (status, body) = send(
            &app.router,
            with_cookie(get("/api/flights/5/video"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body["error"],
            "Flug läuft noch – das Video gibt es nach der Landung"
        );
    }
}

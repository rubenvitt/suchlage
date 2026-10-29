//! Funkprotokoll per SSE für die Systemseite (nur Admins). Wie `/api/live` nie komprimiert.

use std::convert::Infallible;
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum_extra::extract::CookieJar;
use futures::{Stream, StreamExt, stream};
use tokio::sync::broadcast::{self, error::RecvError};

use crate::app::AppState;
use crate::auth::session::{self, Admin};
use crate::contract::{DeviceLogEntry, DeviceLogEvent};

const KEEP_ALIVE: Duration = Duration::from_secs(15);

pub fn routes() -> Router<AppState> {
    Router::new().route("/device-log", get(device_log))
}

/// Zuerst der Verlauf, dann jeder neue Eintrag. Wer nicht hinterherkommt, bekommt `missed` und
/// liest ab dem aktuellen Stand weiter.
async fn device_log(
    _: Admin,
    State(state): State<AppState>,
    jar: CookieJar,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (entries, rx) = state.devlog.subscribe();
    let first = stream::once(async move { sse(&DeviceLogEvent::History { entries }) });
    let devlog = state.devlog.clone();
    let rest = stream::unfold(rx, next_event).take_until(async move { devlog.stopped().await });
    // Abmelden, Sperren oder Herabstufen beenden auch den offenen Strom. `Admin` hat das Cookie
    // eben geprüft; fehlt es trotzdem, endet der Strom beim ersten Prüfen.
    let token = session::token(&jar).unwrap_or_default();
    let revoked = session::ended(
        state.pool.clone(),
        token,
        session::STREAM_CHECK,
        session::is_admin,
    );
    Sse::new(first.chain(rest).take_until(revoked))
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
}

async fn next_event(
    mut rx: broadcast::Receiver<DeviceLogEntry>,
) -> Option<(
    Result<Event, Infallible>,
    broadcast::Receiver<DeviceLogEntry>,
)> {
    let event = match rx.recv().await {
        Ok(entry) => DeviceLogEvent::Entry { entry },
        Err(RecvError::Lagged(count)) => DeviceLogEvent::Missed { count },
        Err(RecvError::Closed) => return None,
    };
    Some((sse(&event), rx))
}

fn sse(event: &DeviceLogEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(event)
        .expect("DeviceLogEvent ist serialisierbar"))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::contract::{DeviceLogLevel, Role};
    use crate::testing::{get, send, test_app, with_cookie};

    async fn next_data(body: &mut Body) -> serde_json::Value {
        loop {
            let frame = tokio::time::timeout(std::time::Duration::from_secs(5), body.frame())
                .await
                .expect("SSE-Ereignis innerhalb von 5 s")
                .unwrap()
                .unwrap();
            if let Some(bytes) = frame.data_ref() {
                let text = String::from_utf8_lossy(bytes);
                if let Some(data) = text.lines().find_map(|l| l.strip_prefix("data: ")) {
                    return serde_json::from_str(data).unwrap();
                }
            }
        }
    }

    #[tokio::test]
    async fn admin_gets_history_then_new_entries() {
        let app = test_app().await;
        app.state
            .devlog
            .system(DeviceLogLevel::Info, Some("RC1"), "vorher".into());
        let cookie = app.cookie(Role::Admin).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(get("/api/device-log"), &cookie))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "text/event-stream");
        let mut body = res.into_body();
        let history = next_data(&mut body).await;
        assert_eq!(history["type"], "history");
        assert_eq!(history["entries"][0]["summary"], "vorher");

        app.state
            .devlog
            .system(DeviceLogLevel::Error, Some("RC1"), "nachher".into());
        let entry = next_data(&mut body).await;
        assert_eq!(entry["type"], "entry");
        assert_eq!(entry["entry"]["level"], "error");
        assert_eq!(entry["entry"]["seq"], 2);
    }

    #[tokio::test]
    async fn operators_do_not_see_the_device_log() {
        let app = test_app().await;
        let cookie = app.cookie(Role::Operator).await;
        let (status, _) = send(&app.router, with_cookie(get("/api/device-log"), &cookie)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = send(&app.router, get("/api/device-log")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn shutdown_ends_the_stream() {
        let app = test_app().await;
        let cookie = app.cookie(Role::Admin).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(get("/api/device-log"), &cookie))
            .await
            .unwrap();
        let mut body = res.into_body();
        assert_eq!(next_data(&mut body).await["type"], "history");
        app.stop_situation();
        let end = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(frame) = body.frame().await {
                frame.unwrap();
            }
        })
        .await;
        assert!(end.is_ok(), "Strom endet mit dem Herunterfahren");
    }
}

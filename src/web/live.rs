//! Live-Strom per SSE (ADR 0001). Nie komprimieren – Cloudflare puffert sonst.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::{Json, Router};
use futures::{Stream, StreamExt, stream};
use tokio::sync::broadcast::{self, error::RecvError};

use crate::app::AppState;
use crate::auth::session::Viewer;
use crate::contract::{LiveEvent, SituationSnapshot};
use crate::error::AppError;
use crate::runtime::service::SituationHandle;

const KEEP_ALIVE: Duration = Duration::from_secs(15);

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/situation", get(situation))
        .route("/live", get(live))
}

async fn situation(
    _: Viewer,
    State(state): State<AppState>,
) -> Result<Json<SituationSnapshot>, AppError> {
    Ok(Json(state.situation.snapshot().await?))
}

/// Zuerst ein `snapshot`, dann die Ereignisse des Hubs. Kommt der Strom nicht hinterher, folgt statt
/// der verpassten Ereignisse ein neuer `snapshot`.
async fn live(
    _: Viewer,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let (snapshot, rx) = state.situation.subscribe().await?;
    let first = stream::once(async move { sse(&LiveEvent::Snapshot { snapshot }) });
    // shortcut: Abmelden oder ein widerrufener Großbild-Token beenden laufende Ströme erst beim
    // nächsten Neuverbinden; reicht für M1, sonst die Sitzung im Strom regelmäßig neu prüfen.
    let rest = stream::unfold((rx, state.situation.clone()), next_event);
    Ok(Sse::new(first.chain(rest)).keep_alive(KeepAlive::new().interval(KEEP_ALIVE)))
}

type Feed = (broadcast::Receiver<LiveEvent>, SituationHandle);

async fn next_event((mut rx, handle): Feed) -> Option<(Result<Event, Infallible>, Feed)> {
    match rx.recv().await {
        Ok(event) => Some((sse(&event), (rx, handle))),
        Err(RecvError::Lagged(missed)) => {
            tracing::debug!(missed, "Live-Strom überholt – neuer Snapshot");
            let (snapshot, fresh) = handle.subscribe().await.ok()?;
            Some((sse(&LiveEvent::Snapshot { snapshot }), (fresh, handle)))
        }
        Err(RecvError::Closed) => None,
    }
}

fn sse(event: &LiveEvent) -> Result<Event, Infallible> {
    Ok(Event::default()
        .json_data(event)
        .expect("LiveEvent ist serialisierbar"))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::contract::Role;
    use crate::situation::observation::{Observation, Observed};
    use crate::testing::{TestApp, get, send, test_app, with_cookie};

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

    fn topology(at_ms: i64, aircraft_sn: Option<String>) -> Observed {
        Observed {
            at_ms,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn,
            },
        }
    }

    async fn open_live(app: &TestApp, role: Role) -> Body {
        let cookie = app.cookie(role).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(get("/api/live"), &cookie))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "text/event-stream");
        assert!(res.headers().get("content-encoding").is_none());
        res.into_body()
    }

    #[tokio::test]
    async fn live_starts_with_snapshot_then_streams_events() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        assert_eq!(next_data(&mut body).await["type"], "snapshot");

        app.state.situation.observe(topology(1, None)).await;
        let event = next_data(&mut body).await;
        assert_eq!(event["type"], "gateway");
        assert_eq!(event["gateway"]["sn"], "RC1");
    }

    #[tokio::test]
    async fn overflowed_stream_resumes_with_a_fresh_snapshot() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        assert_eq!(next_data(&mut body).await["type"], "snapshot");

        // Mehr Ereignisse als der Hub puffert, ohne dass der Strom liest.
        for at in 1..=1_100 {
            app.state
                .situation
                .observe(topology(at, Some(format!("AC{at}"))))
                .await;
        }
        // Der Befehlskanal ist eine Reihe: Antwortet die Abfrage, sind alle Observations verarbeitet.
        app.state.situation.snapshot().await.unwrap();

        let resumed = next_data(&mut body).await;
        assert_eq!(resumed["type"], "snapshot");
        assert_eq!(resumed["snapshot"]["gateways"][0]["aircraft_sn"], "AC1100");

        app.state.situation.observe(topology(1_101, None)).await;
        let event = next_data(&mut body).await;
        assert_eq!(event["type"], "gateway");
        assert_eq!(event["gateway"]["aircraft_sn"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn kiosk_follows_the_situation() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Kiosk).await;
        assert_eq!(next_data(&mut body).await["type"], "snapshot");

        app.state.situation.observe(topology(1, None)).await;
        let kiosk = app.cookie(Role::Kiosk).await;
        let (status, snapshot) =
            send(&app.router, with_cookie(get("/api/situation"), &kiosk)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(snapshot["gateways"][0]["sn"], "RC1");
    }

    #[tokio::test]
    async fn live_and_situation_require_a_session() {
        let app = test_app().await;
        for uri in ["/api/live", "/api/situation"] {
            let (status, body) = send(&app.router, get(uri)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
            assert_eq!(body["error"], "Nicht angemeldet");
        }
    }
}

//! Live-Strom per SSE (ADR 0001). Nie komprimieren – Cloudflare puffert sonst.

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::get;
use axum::{Json, Router};
use futures::{Stream, StreamExt, stream};
use tokio::sync::broadcast::{self, error::RecvError};
use tokio::sync::oneshot;

use crate::app::AppState;
use crate::auth::session::Viewer;
use crate::contract::{LiveEvent, SituationSnapshot};
use crate::error::AppError;
use crate::map::{MapChange, MapHub};
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

/// Zuerst ein `snapshot` und alle Kartenelemente, dann die Ereignisse beider Hubs. Kommt der Strom
/// nicht hinterher, folgt statt der verpassten Ereignisse ein neuer `snapshot` bzw. die ganze Liste.
async fn live(
    _: Viewer,
    State(state): State<AppState>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let (snapshot, rx) = state.situation.subscribe().await?;
    // Vor dem Lesen abonnieren: Keine Änderung fällt zwischen Liste und Strom heraus. Doppelt
    // ankommende sind harmlos, der Browser legt nach ID ab.
    let map_rx = state.map.subscribe();
    let elements = state.map.list().await?;
    let first = stream::iter([
        sse(&LiveEvent::Snapshot { snapshot }),
        sse(&LiveEvent::MapElements { elements }),
    ]);
    // shortcut: Abmelden oder ein widerrufener Großbild-Token beenden laufende Ströme erst beim
    // nächsten Neuverbinden; reicht für M1, sonst die Sitzung im Strom regelmäßig neu prüfen.
    // Der Lage-Strom endet mit dem Aktor beim Herunterfahren; der Kartenstrom endet mit ihm.
    let (ended, on_end) = oneshot::channel::<()>();
    let situation = stream::unfold((rx, state.situation.clone()), next_event).chain(
        stream::once(async move {
            let _ = ended.send(());
        })
        .filter_map(|()| async { None }),
    );
    let map = stream::unfold((map_rx, state.map.clone()), next_map_event).take_until(on_end);
    Ok(Sse::new(first.chain(stream::select(situation, map)))
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE)))
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

type MapFeed = (broadcast::Receiver<MapChange>, MapHub);

async fn next_map_event((mut rx, hub): MapFeed) -> Option<(Result<Event, Infallible>, MapFeed)> {
    let event = match rx.recv().await {
        Ok(MapChange::Created(element) | MapChange::Updated(element)) => {
            LiveEvent::MapElement { element }
        }
        Ok(MapChange::Deleted { id }) => LiveEvent::MapElementDeleted { id },
        Err(RecvError::Lagged(missed)) => {
            tracing::debug!(missed, "Kartenstrom überholt – neue Liste");
            let elements = hub
                .list()
                .await
                .inspect_err(|e| tracing::warn!(error = ?e, "Kartenelemente nicht lesbar"))
                .ok()?;
            LiveEvent::MapElements { elements }
        }
        Err(RecvError::Closed) => return None,
    };
    Some((sse(&event), (rx, hub)))
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

    /// Snapshot und Kartenliste kommen immer zuerst.
    async fn opening(body: &mut Body) {
        assert_eq!(next_data(body).await["type"], "snapshot");
        assert_eq!(next_data(body).await["type"], "map_elements");
    }

    #[tokio::test]
    async fn live_starts_with_snapshot_then_streams_events() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        opening(&mut body).await;

        app.state.situation.observe(topology(1, None)).await;
        let event = next_data(&mut body).await;
        assert_eq!(event["type"], "gateway");
        assert_eq!(event["gateway"]["sn"], "RC1");
    }

    #[tokio::test]
    async fn overflowed_stream_resumes_with_a_fresh_snapshot() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        opening(&mut body).await;

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
        opening(&mut body).await;

        app.state.situation.observe(topology(1, None)).await;
        let kiosk = app.cookie(Role::Kiosk).await;
        let (status, snapshot) =
            send(&app.router, with_cookie(get("/api/situation"), &kiosk)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(snapshot["gateways"][0]["sn"], "RC1");
    }

    #[tokio::test]
    async fn live_ends_when_the_situation_stops() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        opening(&mut body).await;

        app.stop_situation();
        // Endet der Strom nicht, wartet `serve` beim Herunterfahren bis zum SIGKILL.
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(frame) = body.frame().await {
                frame.unwrap();
            }
        })
        .await
        .expect("Strom endet innerhalb von 5 s");
    }

    fn pin(lat: f64) -> crate::map::NewElement {
        crate::map::NewElement {
            id: None,
            name: "Pilot 1".into(),
            kind: crate::contract::MapElementKind::Point,
            points: vec![crate::contract::MapPoint {
                lat,
                lon: 10.0,
                alt_m: None,
            }],
            color: "#E23C39".into(),
            clamp_to_ground: true,
            author: "RC Plus 1".into(),
            origin: crate::contract::MapElementOrigin::Pilot,
        }
    }

    #[tokio::test]
    async fn map_changes_reach_the_browser() {
        let app = test_app().await;
        let created = app.state.map.create(pin(52.0), 1).await.unwrap();
        let mut body = open_live(&app, Role::Kiosk).await;
        assert_eq!(next_data(&mut body).await["type"], "snapshot");
        let list = next_data(&mut body).await;
        assert_eq!(list["type"], "map_elements");
        assert_eq!(list["elements"][0]["id"], created.id.as_str());

        let second = app.state.map.create(pin(52.1), 2).await.unwrap();
        let event = next_data(&mut body).await;
        assert_eq!(event["type"], "map_element");
        assert_eq!(event["element"]["id"], second.id.as_str());

        app.state.map.delete(&created.id).await.unwrap();
        let event = next_data(&mut body).await;
        assert_eq!(event["type"], "map_element_deleted");
        assert_eq!(event["id"], created.id.as_str());
    }

    #[tokio::test]
    async fn overflowed_map_stream_resumes_with_the_whole_list() {
        let app = test_app().await;
        let mut body = open_live(&app, Role::Operator).await;
        opening(&mut body).await;
        // Mehr Änderungen, als der Hub je Empfänger puffert, ohne dass der Strom liest.
        for i in 0..300 {
            app.state
                .map
                .create(pin(50.0 + f64::from(i) / 1_000.0), i64::from(i))
                .await
                .unwrap();
        }
        let resumed = next_data(&mut body).await;
        assert_eq!(resumed["type"], "map_elements");
        assert_eq!(resumed["elements"].as_array().unwrap().len(), 300);
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

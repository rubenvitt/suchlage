//! Flugliste und Flugdetail. Nur für Konten: das Großbild zeigt die Lage, nicht das Archiv.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::{Admin, Operator};
use crate::contract::{FlightDetail, FlightSummary, FlightVideo, TrackPoint};
use crate::error::AppError;
use crate::situation::track::TrackBuilder;
use crate::store;
use crate::web::missions::parse_scope;

const LIST_LIMIT: i64 = 200;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/flights", get(list))
        .route("/flights/{id}", get(get_one).delete(delete_flight))
        .route("/flights/{id}/video", get(video).delete(delete_video))
}

#[derive(serde::Deserialize)]
struct ListQuery {
    mission: Option<String>,
}

async fn list(
    _: Operator,
    State(state): State<AppState>,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<FlightSummary>>, AppError> {
    let mut flights = match parse_scope(q.mission.as_deref())? {
        Some(mission) => store::flights::list_in(&state.pool, mission, LIST_LIMIT).await?,
        None => store::flights::list(&state.pool, LIST_LIMIT).await?,
    };
    for f in flights.iter_mut().filter(|f| f.ended_at_ms.is_none()) {
        let segments = store::flights::segments(&state.pool, f.id).await?;
        with_track_so_far(f, &segments);
    }
    Ok(Json(flights))
}

/// Strecke und Höhe schreibt der Speicher erst beim Schließen; für einen laufenden Flug kommen sie
/// aus den bisher gespeicherten Spurpunkten, nach derselben Regel wie im Kern.
fn with_track_so_far(f: &mut FlightSummary, segments: &[Vec<TrackPoint>]) {
    let track = TrackBuilder::from_segments(segments.to_vec());
    f.distance_m = track.distance_m();
    f.max_elevation_m = track.max_elevation_m();
}

async fn get_one(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<FlightDetail>, AppError> {
    Ok(Json(detail(&state, id).await?))
}

/// Flug mit Spur, Video und Fotos.
async fn delete_flight(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let d = state.deleter.clone();
    crate::deletion::detached(async move { d.delete_flight(id).await }).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn delete_video(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let d = state.deleter.clone();
    crate::deletion::detached(async move { d.delete_flight_video(id).await }).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(serde::Deserialize)]
struct VideoQuery {
    #[serde(default)]
    download: u8,
}

/// Aufnahme des Flugs aus MediaMTX, zum Ansehen oder mit `download=1` als Datei.
async fn video(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<VideoQuery>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let flight = store::flights::get(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let range = headers.get(header::RANGE).cloned();
    crate::media::proxy::flight_video(&state, &flight, q.download == 1, range).await
}

/// Flug samt Spur und, nach der Landung, dem Video, sofern MediaMTX eine Aufnahme meldet.
pub async fn detail(state: &AppState, id: i64) -> Result<FlightDetail, AppError> {
    let mut flight = store::flights::get(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let segments = store::flights::segments(&state.pool, id).await?;
    if flight.ended_at_ms.is_none() {
        with_track_so_far(&mut flight, &segments);
    }
    let video_start = match flight.ended_at_ms {
        Some(_) => crate::media::proxy::flight_video_start(state, &flight).await,
        None => None,
    };
    let video = video_start.map(|started_at_ms| FlightVideo {
        play_url: format!("/api/flights/{id}/video"),
        download_url: format!("/api/flights/{id}/video?download=1"),
        started_at_ms,
    });
    Ok(FlightDetail {
        flight,
        segments,
        video,
    })
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;

    use crate::contract::{Role, TrackPoint, examples};
    use crate::store;
    use crate::testing::{get, send, test_app, with_cookie};

    #[tokio::test]
    async fn detail_returns_track_segments_and_404_for_unknown() {
        let app = test_app().await;
        let f = examples::flight_landed(3, 1_000);
        store::flights::insert(&app.state.pool, &f).await.unwrap();
        let p = |t| TrackPoint {
            t_ms: t,
            lat: 52.0,
            lon: 10.0,
            elevation_m: None,
        };
        store::flights::append_points(&app.state.pool, 3, &[(0, p(1_000)), (1, p(9_000))])
            .await
            .unwrap();
        let admin = app.cookie(Role::Admin).await;

        let (status, body) = send(&app.router, with_cookie(get("/api/flights/3"), &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["segments"].as_array().unwrap().len(), 2);
        assert_eq!(body["flight"]["end_reason"], "landed");
        assert_eq!(body["video"], serde_json::Value::Null);
        let (status, missing) =
            send(&app.router, with_cookie(get("/api/flights/99"), &admin)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(missing["error"], "Nicht gefunden");

        let (status, list) = send(&app.router, with_cookie(get("/api/flights"), &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list[0]["id"], 3);
    }

    #[tokio::test]
    async fn flights_are_filtered_by_mission() {
        let app = test_app().await;
        let mission = app
            .state
            .missions
            .create(
                crate::contract::MissionCreate {
                    name: "A".into(),
                    location: None,
                },
                "Operator",
                1,
            )
            .await
            .unwrap()
            .id;
        for id in [1, 2] {
            let f = examples::flight_landed(id, 1_000 * id);
            store::flights::insert(&app.state.pool, &f).await.unwrap();
        }
        assert!(
            store::missions::assign_flight(&app.state.pool, 1, mission)
                .await
                .unwrap()
        );
        let admin = app.cookie(Role::Admin).await;
        let ids = |uri: String| {
            let router = app.router.clone();
            let admin = admin.clone();
            async move {
                let (status, list) = send(&router, with_cookie(get(&uri), &admin)).await;
                assert_eq!(status, StatusCode::OK, "{list}");
                let mut ids: Vec<i64> = list
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|f| f["id"].as_i64().unwrap())
                    .collect();
                ids.sort();
                ids
            }
        };
        assert_eq!(ids(format!("/api/flights?mission={mission}")).await, [1]);
        assert_eq!(ids("/api/flights?mission=none".into()).await, [2]);
        assert_eq!(ids("/api/flights".into()).await, [1, 2]);
        let (status, _) = send(
            &app.router,
            with_cookie(get("/api/flights?mission=abc"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn running_flight_shows_its_track_so_far() {
        let app = test_app().await;
        let mut f = examples::flight_running();
        (f.distance_m, f.max_elevation_m) = (0.0, None); // so steht er bis zum Schließen im Speicher
        store::flights::insert(&app.state.pool, &f).await.unwrap();
        let p = |t, lat, e| TrackPoint {
            t_ms: t,
            lat,
            lon: 10.0,
            elevation_m: Some(e),
        };
        // Zwei Segmente; die Lücke zählt nicht zur Strecke.
        store::flights::append_points(
            &app.state.pool,
            f.id,
            &[
                (0, p(0, 52.0, 5.0)),
                (0, p(2_000, 52.001, 30.0)),
                (1, p(99_000, 52.1, 60.0)),
            ],
        )
        .await
        .unwrap();
        let admin = app.cookie(Role::Admin).await;

        let (_, detail) = send(&app.router, with_cookie(get("/api/flights/7"), &admin)).await;
        let (_, list) = send(&app.router, with_cookie(get("/api/flights"), &admin)).await;
        for flight in [&detail["flight"], &list[0]] {
            let distance = flight["distance_m"].as_f64().unwrap();
            assert!((distance - 111.2).abs() < 1.0, "{distance}");
            assert_eq!(flight["max_elevation_m"], 60.0);
        }
    }

    #[tokio::test]
    async fn flights_are_for_accounts_only() {
        let app = test_app().await;
        let kiosk = app.cookie(Role::Kiosk).await;
        for uri in ["/api/flights", "/api/flights/1", "/api/flights/1/video"] {
            assert_eq!(
                send(&app.router, get(uri)).await.0,
                StatusCode::UNAUTHORIZED,
                "{uri}"
            );
            let (status, _) = send(&app.router, with_cookie(get(uri), &kiosk)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        }
    }
}

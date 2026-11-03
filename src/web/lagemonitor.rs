//! Schnittstelle für den Lagemonitor im FüKw (`/api/lagemonitor/v1`). Angemeldet wird mit einem
//! Großbild-Token als `Authorization: Bearer …`, nicht mit einem Sitzungscookie: Der Server des
//! Lagemonitors fragt selbst, der Kiosk-Browser sieht Suchlage nie. Nur lesen, kein CORS.

use std::collections::HashMap;

use axum::body::Bytes;
use axum::extract::{FromRef, FromRequestParts, Path, Query, RawQuery, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;
use tower_http::compression::CompressionLayer;

use crate::app::AppState;
use crate::contract::{
    AircraftState, FLIGHT_CLOSE_AFTER_MS, FRESHNESS_LOST_MS, FlightPhase, FmFeatureCollection,
    FmFeatureCollectionType, LAGEMONITOR_API_VERSION, LmAircraft, LmLage, LmVideo,
    SituationSnapshot, StreamPhase,
};
use crate::error::AppError;
use crate::media::proxy;
use crate::sn::is_valid_sn;
use crate::store;

const _: () = assert!(FRESHNESS_LOST_MS < FLIGHT_CLOSE_AFTER_MS);

pub fn routes() -> Router<AppState> {
    Router::new()
        // gzip, wenn der Lagemonitor es kann: Flugspuren sind Koordinatenlisten und schrumpfen auf
        // ein Fünftel, und der FüKw hängt am Mobilfunk. Video bleibt unberührt.
        .route("/lage", get(lage).layer(CompressionLayer::new().gzip(true)))
        .route("/video/{gateway_sn}/whep", post(whep))
        .route("/video/{gateway_sn}/hls/{*file}", get(hls))
}

/// Ein gültiger Großbild-Token. Widerrufen wirkt sofort, denn jede Anfrage prüft neu.
pub struct LagemonitorClient;

impl<S: Send + Sync> FromRequestParts<S> for LagemonitorClient
where
    SqlitePool: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        let secret = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or(AppError::Unauthorized)?;
        store::kiosk::find_by_token(&SqlitePool::from_ref(state), secret.trim())
            .await?
            .map(|_| LagemonitorClient)
            .ok_or(AppError::Unauthorized)
    }
}

/// Drohnen mit Position, deren Fernsteuerung sich innerhalb der Verlustschwelle gemeldet hat.
pub fn aircraft_of(
    snapshot: &SituationSnapshot,
    labels: &HashMap<String, String>,
) -> Vec<LmAircraft> {
    let fresh = |a: &&AircraftState| snapshot.server_time_ms - a.last_seen_ms <= FRESHNESS_LOST_MS;
    snapshot
        .aircraft
        .iter()
        .filter(fresh)
        .filter_map(|a| {
            let t = &a.telemetry;
            Some(LmAircraft {
                gateway_sn: a.gateway_sn.clone(),
                label: labels.get(&a.gateway_sn).cloned(),
                position: t.position?,
                heading_deg: t.heading_deg,
                elevation_takeoff_m: t.elevation_takeoff_m,
                battery_percent: t.battery.as_ref().and_then(|b| b.percent),
                airborne: a.phase == FlightPhase::Airborne,
                flight_id: a.flight_id,
            })
        })
        .collect()
}

/// Laufende Livebilder, in der Reihenfolge der Lage.
pub fn videos_of(snapshot: &SituationSnapshot, labels: &HashMap<String, String>) -> Vec<LmVideo> {
    snapshot
        .streams
        .iter()
        .filter(|s| s.phase == StreamPhase::Live)
        .map(|s| LmVideo {
            gateway_sn: s.gateway_sn.clone(),
            label: labels.get(&s.gateway_sn).cloned(),
        })
        .collect()
}

#[derive(Deserialize)]
pub struct LageQuery {
    /// `aus` oder die `map_etag` der letzten Antwort (siehe [`LmLage::map_etag`]).
    karte: Option<String>,
}

/// Fassung der Karte: die ersten 8 Byte des SHA-256 ihrer JSON-Form.
pub fn map_etag(map: &FmFeatureCollection) -> String {
    let json = serde_json::to_vec(map).unwrap_or_default();
    hex::encode(&Sha256::digest(&json)[..8])
}

/// Die Karte, wenn der Lagemonitor sie braucht, und immer ihre Fassung. Der Lagemonitor fragt alle
/// 5 s nach den Drohnen, die Flugspuren aber nur selten und nur bei einer Änderung.
pub fn map_for(
    map: FmFeatureCollection,
    karte: Option<&str>,
) -> (Option<FmFeatureCollection>, Option<String>) {
    let etag = map_etag(&map);
    let send = match karte {
        None => true,
        Some("aus") => false,
        Some(known) => known != etag,
    };
    (send.then_some(map), Some(etag))
}

async fn lage(
    _: LagemonitorClient,
    State(state): State<AppState>,
    Query(query): Query<LageQuery>,
) -> Result<Json<LmLage>, AppError> {
    let snapshot = state.situation.snapshot().await?;
    let karte = query.karte.as_deref();
    let Some((mission, map)) = state.missions.lagemonitor().await? else {
        let (map, map_etag) = map_for(
            FmFeatureCollection {
                kind: FmFeatureCollectionType::FeatureCollection,
                features: Vec::new(),
            },
            karte,
        );
        return Ok(Json(LmLage {
            api_version: LAGEMONITOR_API_VERSION,
            server_time_ms: snapshot.server_time_ms,
            mission: None,
            map,
            map_etag,
            aircraft: Vec::new(),
            videos: Vec::new(),
        }));
    };
    let (map, map_etag) = map_for(map, karte);
    let labels: HashMap<String, String> = store::remote_controllers::labels(&state.pool)
        .await?
        .into_iter()
        .collect();
    Ok(Json(LmLage {
        api_version: LAGEMONITOR_API_VERSION,
        server_time_ms: snapshot.server_time_ms,
        mission: Some(mission),
        map,
        map_etag,
        aircraft: aircraft_of(&snapshot, &labels),
        videos: videos_of(&snapshot, &labels),
    }))
}

async fn whep(
    _: LagemonitorClient,
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
    _: LagemonitorClient,
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
    use axum::http::{Request, StatusCode};
    use serde_json::Value;

    use super::*;
    use crate::contract::{
        FlightSummary, GeoPoint, MapElementKind, MapElementOrigin, MapPoint, MissionCreate,
        StreamState, Telemetry, TrackPoint,
    };
    use crate::map::NewElement;
    use crate::testing::{get, send, test_app};

    fn bearer(token: &str) -> Request<axum::body::Body> {
        let mut req = get("/api/lagemonitor/v1/lage");
        req.headers_mut()
            .insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        req
    }

    #[tokio::test]
    async fn without_or_with_an_unknown_token_is_401() {
        let app = test_app().await;
        let (status, _) = send(&app.router, get("/api/lagemonitor/v1/lage")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, _) = send(&app.router, bearer("falsch")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn a_revoked_token_stops_at_once() {
        let app = test_app().await;
        let (view, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        assert_eq!(send(&app.router, bearer(&secret)).await.0, StatusCode::OK);
        store::kiosk::delete(&app.state.pool, view.id)
            .await
            .unwrap();
        assert_eq!(
            send(&app.router, bearer(&secret)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn without_a_running_mission_everything_is_empty() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        let m = app
            .state
            .missions
            .create(
                MissionCreate {
                    name: "Vorbereitet".into(),
                    location: None,
                },
                "Test",
                1,
            )
            .await
            .unwrap();
        let (status, json) = send(&app.router, bearer(&secret)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["api_version"], 1);
        assert_eq!(
            json["mission"],
            Value::Null,
            "vorbereitet ({}) zählt nicht",
            m.id
        );
        assert_eq!(json["map"]["features"], serde_json::json!([]));
        assert_eq!(json["videos"], serde_json::json!([]));
    }

    fn bearer_at(token: &str, path: &str) -> Request<axum::body::Body> {
        let mut req = get(path);
        req.headers_mut()
            .insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        req
    }

    #[test]
    fn the_map_comes_without_karte_or_when_it_changed() {
        let map = || FmFeatureCollection {
            kind: FmFeatureCollectionType::FeatureCollection,
            features: Vec::new(),
        };
        let etag = map_etag(&map());
        assert_eq!(map_for(map(), None), (Some(map()), Some(etag.clone())));
        assert_eq!(
            map_for(map(), Some("alt")),
            (Some(map()), Some(etag.clone()))
        );
        assert_eq!(map_for(map(), Some(&etag)), (None, Some(etag.clone())));
        assert_eq!(map_for(map(), Some("aus")), (None, Some(etag)));
    }

    #[tokio::test]
    async fn karte_drops_the_unchanged_map_and_old_clients_still_get_it() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        let (_, json) = send(&app.router, bearer(&secret)).await;
        let etag = json["map_etag"].as_str().unwrap().to_string();
        assert!(json["map"].is_object(), "ohne `karte` wie bisher");
        let path = format!("/api/lagemonitor/v1/lage?karte={etag}");
        let (_, json) = send(&app.router, bearer_at(&secret, &path)).await;
        assert!(json.get("map").is_none());
        assert_eq!(json["map_etag"], etag.as_str());
        let (_, json) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/lage?karte=aus"),
        )
        .await;
        assert!(json.get("map").is_none());
    }

    #[tokio::test]
    async fn lage_is_gzipped_when_asked() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        let mut req = bearer(&secret);
        req.headers_mut()
            .insert(axum::http::header::ACCEPT_ENCODING, "gzip".parse().unwrap());
        let res = tower::ServiceExt::oneshot(app.router.clone(), req)
            .await
            .unwrap();
        assert_eq!(res.headers()[axum::http::header::CONTENT_ENCODING], "gzip");
    }

    #[tokio::test]
    async fn a_running_mission_brings_areas_and_all_tracks() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        let hub = &app.state.missions;
        let m = hub
            .create(
                MissionCreate {
                    name: "Vermisste Person Oldenstadt".into(),
                    location: None,
                },
                "Test",
                1,
            )
            .await
            .unwrap();
        hub.start(m.id, 2).await.unwrap();
        let p = |lat, lon| MapPoint {
            lat,
            lon,
            alt_m: None,
        };
        app.state
            .map
            .create(
                NewElement {
                    id: None,
                    name: "Suchgebiet A".into(),
                    kind: MapElementKind::Polygon,
                    points: vec![p(52.97, 10.55), p(52.98, 10.55), p(52.98, 10.57)],
                    color: "#2D6CDF".into(),
                    clamp_to_ground: true,
                    author: "Test".into(),
                    origin: MapElementOrigin::Web,
                    mission_id: Some(m.id),
                },
                3,
            )
            .await
            .unwrap();
        for id in [1, 2] {
            let flight = FlightSummary {
                id,
                aircraft_sn: "AC1".into(),
                gateway_sn: "RC1".into(),
                started_at_ms: 10 * id,
                ended_at_ms: (id == 1).then_some(15),
                end_reason: (id == 1).then_some(crate::contract::FlightEndReason::Landed),
                max_elevation_m: None,
                distance_m: 0.0,
            };
            store::flights::insert(&app.state.pool, &flight)
                .await
                .unwrap();
            let point = |t_ms, lat| {
                (
                    0,
                    TrackPoint {
                        t_ms,
                        lat,
                        lon: 10.56,
                        elevation_m: None,
                    },
                )
            };
            store::flights::append_points(
                &app.state.pool,
                id,
                &[point(10 * id, 52.97), point(10 * id + 1, 52.975)],
            )
            .await
            .unwrap();
        }
        let (status, json) = send(&app.router, bearer(&secret)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["mission"]["name"], "Vermisste Person Oldenstadt");
        let kinds: Vec<&str> = json["map"]["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["properties"]["kind"].as_str().unwrap())
            .collect();
        assert_eq!(kinds, ["map_element", "flight_track", "flight_track"]);
    }

    fn snapshot() -> SituationSnapshot {
        let mut s = crate::contract::examples::snapshot_flying();
        s.server_time_ms = 100_000;
        s
    }

    #[test]
    fn aircraft_need_a_position_and_a_recent_message() {
        let mut s = snapshot();
        let template = s.aircraft[0].clone();
        let at = |sn: &str, seen: i64, position: Option<GeoPoint>| AircraftState {
            gateway_sn: sn.into(),
            last_seen_ms: seen,
            telemetry: Telemetry {
                position,
                ..template.telemetry.clone()
            },
            ..template.clone()
        };
        let here = Some(GeoPoint {
            lat: 52.9,
            lon: 10.5,
        });
        s.aircraft = vec![
            at("RC-FRISCH", 100_000 - FRESHNESS_LOST_MS, here),
            at("RC-VERLOREN", 100_000 - FRESHNESS_LOST_MS - 1, here),
            at("RC-OHNE", 100_000, None),
        ];
        let labels = HashMap::from([("RC-FRISCH".to_string(), "Drohne 1".to_string())]);
        let out = aircraft_of(&s, &labels);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].gateway_sn, "RC-FRISCH");
        assert_eq!(out[0].label.as_deref(), Some("Drohne 1"));
    }

    #[test]
    fn only_live_streams_are_videos() {
        let mut s = snapshot();
        let stream = |sn: &str, phase| StreamState {
            gateway_sn: sn.into(),
            phase,
            video_id: None,
            lens: None,
            error: None,
            updated_at_ms: 0,
        };
        s.streams = vec![
            stream("RC1", StreamPhase::Live),
            stream("RC2", StreamPhase::Starting),
        ];
        let out = videos_of(&s, &HashMap::new());
        assert_eq!(
            out,
            vec![LmVideo {
                gateway_sn: "RC1".into(),
                label: None
            }]
        );
    }
}

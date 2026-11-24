//! Schnittstelle für den Lagemonitor im FüKw (`/api/lagemonitor/v1`). Angemeldet wird mit einem
//! Großbild-Token als `Authorization: Bearer …`, nicht mit einem Sitzungscookie: Der Server des
//! Lagemonitors fragt selbst, der Kiosk-Browser sieht Suchlage nie. Nur lesen, kein CORS.
//!
//! Lifeline Hub fragt genauso (SUL-49) und bekommt zusätzlich die Vorschaubilder des laufenden
//! Einsatzes (`/photos`), aber nur mit einem Zugang, der dafür angelegt wurde
//! (`kiosk_tokens.photos`); jeder andere bekommt dort 403. Originale gehen nie hinaus.

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
    FmFeatureCollectionType, LAGEMONITOR_API_VERSION, LmAircraft, LmLage, LmPhoto, LmPhotoShot,
    LmVideo, PhotoShot, SituationSnapshot, StreamPhase,
};
use crate::error::AppError;
use crate::media::proxy;
use crate::sn::is_valid_sn;
use crate::store;

const _: () = assert!(FRESHNESS_LOST_MS < FLIGHT_CLOSE_AFTER_MS);

/// Cache für Vorschaubilder an Lifeline Hub: kurz, damit ein beendeter Einsatz oder ein
/// widerrufener Zugang bald auch aus einem Zwischenspeicher verschwindet.
pub const THUMB_CACHE_CONTROL: &str = "private, max-age=300";

pub fn routes() -> Router<AppState> {
    Router::new()
        // gzip, wenn der Lagemonitor es kann: Flugspuren sind Koordinatenlisten und schrumpfen auf
        // ein Fünftel, und der FüKw hängt am Mobilfunk. Video bleibt unberührt.
        .route("/lage", get(lage).layer(CompressionLayer::new().gzip(true)))
        .route(
            "/photos",
            get(photos).layer(CompressionLayer::new().gzip(true)),
        )
        .route("/photos/{id}/thumb", get(photo_thumb))
        .route("/video/{gateway_sn}/whep", post(whep))
        .route("/video/{gateway_sn}/hls/{*file}", get(hls))
}

/// Ein gültiger Großbild-Token. Widerrufen wirkt sofort, denn jede Anfrage prüft neu.
pub struct LagemonitorClient {
    /// Zugang für Lifeline Hub: darf die Vorschaubilder des laufenden Einsatzes abrufen.
    photos: bool,
}

/// Ein gültiger Großbild-Token mit Fotos (`kiosk_tokens.photos`), sonst 403.
pub struct LagemonitorPhotosClient;

impl<S: Send + Sync> FromRequestParts<S> for LagemonitorPhotosClient
where
    SqlitePool: FromRef<S>,
{
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        let client = LagemonitorClient::from_request_parts(parts, state).await?;
        if client.photos {
            Ok(LagemonitorPhotosClient)
        } else {
            Err(AppError::Forbidden)
        }
    }
}

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
        store::kiosk::photos_by_token(&SqlitePool::from_ref(state), secret.trim())
            .await?
            .map(|photos| LagemonitorClient { photos })
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
    client: LagemonitorClient,
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
            photos_latest_ms: None,
        }));
    };
    let (map, map_etag) = map_for(map, karte);
    let labels: HashMap<String, String> = store::remote_controllers::labels(&state.pool)
        .await?
        .into_iter()
        .collect();
    let photos_latest_ms = if client.photos {
        store::photos::latest_shot_with_thumb(&state.pool, mission.id).await?
    } else {
        None
    };
    Ok(Json(LmLage {
        api_version: LAGEMONITOR_API_VERSION,
        server_time_ms: snapshot.server_time_ms,
        mission: Some(mission),
        map,
        map_etag,
        aircraft: aircraft_of(&snapshot, &labels),
        videos: videos_of(&snapshot, &labels),
        photos_latest_ms,
    }))
}

#[derive(Deserialize)]
pub struct PhotosQuery {
    /// Nur Auslösungen ab dieser Zeit (einschließlich); fehlt sie, alle des laufenden Einsatzes.
    since_ms: Option<i64>,
}

/// Auslösungen für Lifeline Hub: nur mit Position und fertigem Vorschaubild, ohne Seriennummern
/// und ohne Originale. Die Vorschau hängt am selben Bearer-Token wie die Liste.
pub fn lm_shots(shots: Vec<PhotoShot>, since_ms: i64) -> Vec<LmPhotoShot> {
    shots
        .into_iter()
        .filter(|s| s.taken_at_ms >= since_ms)
        .filter_map(|s| {
            let position = s.position?;
            let id = s.photos.iter().map(|p| p.id).min()?;
            let photos: Vec<LmPhoto> = s
                .photos
                .iter()
                .filter_map(|p| {
                    // Dieselbe Version wie in der App: Sie gilt nur für den Cache.
                    let url = p.thumb_url.as_deref()?;
                    let version = url.split_once('?').map_or("", |(_, q)| q);
                    let sep = if version.is_empty() { "" } else { "?" };
                    Some(LmPhoto {
                        lens: p.lens,
                        thumb_url: format!(
                            "/api/lagemonitor/v1/photos/{}/thumb{sep}{version}",
                            p.id
                        ),
                    })
                })
                .collect();
            (!photos.is_empty()).then_some(LmPhotoShot {
                id,
                taken_at_ms: s.taken_at_ms,
                position,
                relative_altitude_m: s.relative_altitude_m,
                gimbal_yaw_deg: s.gimbal_yaw_deg,
                photos,
            })
        })
        .collect()
}

async fn photos(
    _: LagemonitorPhotosClient,
    State(state): State<AppState>,
    Query(query): Query<PhotosQuery>,
) -> Result<Json<Vec<LmPhotoShot>>, AppError> {
    let Some(mission) = store::missions::running_id(&state.pool).await? else {
        return Ok(Json(Vec::new()));
    };
    let shots = state.photos.in_mission(mission).await?;
    Ok(Json(lm_shots(shots, query.since_ms.unwrap_or(i64::MIN))))
}

/// Vorschaubild eines Fotos des laufenden Einsatzes; jedes andere ist für Lifeline Hub 404.
async fn photo_thumb(
    _: LagemonitorPhotosClient,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    let running = store::missions::running_id(&state.pool).await?;
    let mission = store::photos::get(&state.pool, id)
        .await?
        .and_then(|p| p.mission_id);
    if running.is_none() || mission != running {
        return Err(AppError::NotFound);
    }
    crate::web::photos::serve_thumb(&state, id, THUMB_CACHE_CONTROL).await
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
        PhotoLens, StreamState, Telemetry, TrackPoint,
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

    async fn hub_token(app: &crate::testing::TestApp) -> String {
        store::kiosk::create_with(&app.state.pool, "Lifeline Hub", false, true, 0)
            .await
            .unwrap()
            .1
    }

    async fn running(app: &crate::testing::TestApp, name: &str) -> i64 {
        let hub = &app.state.missions;
        let m = hub
            .create(
                MissionCreate {
                    name: name.into(),
                    location: None,
                },
                "Test",
                1,
            )
            .await
            .unwrap();
        hub.start(m.id, 2).await.unwrap();
        m.id
    }

    /// Ein Foto im laufenden Einsatz mit fertigem Vorschaubild, ohne Fotospeicher.
    async fn ready_photo(
        app: &crate::testing::TestApp,
        name: &str,
        taken_at_ms: i64,
        position: Option<(f64, f64)>,
    ) -> i64 {
        let photo = crate::store::photos::NewPhoto {
            object_key: format!("fotos/RCSN1/{name}"),
            name: name.into(),
            fingerprint: None,
            tiny_fingerprint: None,
            shot_key: crate::photos::rules::shot_key("RCSN1", name),
            lens: crate::photos::rules::lens_of(name),
            gateway_sn: "RCSN1".into(),
            aircraft_sn: Some("ACSN1".into()),
            flight_id: None,
            taken_at_ms: Some(taken_at_ms),
            uploaded_at_ms: taken_at_ms + 5,
            lat: position.map(|p| p.0),
            lon: position.map(|p| p.1),
            absolute_altitude_m: Some(120.0),
            relative_altitude_m: Some(60.0),
            gimbal_yaw_deg: Some(-90.0),
            payload_model_key: None,
        };
        let id = store::photos::upsert(&app.state.pool, &photo)
            .await
            .unwrap();
        let path = app.state.photos.thumb_path(id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, crate::photos::thumb::sample_jpeg(64, 48))
            .await
            .unwrap();
        store::photos::set_thumb_ready(&app.state.pool, id, &photo.object_key)
            .await
            .unwrap();
        id
    }

    #[tokio::test]
    async fn a_token_without_photos_gets_403_while_lage_stays_200() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "FüKw", 0)
            .await
            .unwrap();
        running(&app, "Übung").await;
        let id = ready_photo(&app, "DJI_1_T.JPG", 10, Some((52.96, 10.56))).await;
        for path in [
            "/api/lagemonitor/v1/photos".to_string(),
            format!("/api/lagemonitor/v1/photos/{id}/thumb"),
        ] {
            let (status, _) = send(&app.router, bearer_at(&secret, &path)).await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{path}");
        }
        let (status, json) = send(&app.router, bearer(&secret)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["photos_latest_ms"], Value::Null, "nur mit Fotos");
    }

    #[tokio::test]
    async fn a_revoked_photos_token_gets_401_at_once() {
        let app = test_app().await;
        let (view, secret) = store::kiosk::create_with(&app.state.pool, "Hub", false, true, 0)
            .await
            .unwrap();
        running(&app, "Übung").await;
        let id = ready_photo(&app, "DJI_1_T.JPG", 10, Some((52.96, 10.56))).await;
        let paths = [
            "/api/lagemonitor/v1/photos".to_string(),
            format!("/api/lagemonitor/v1/photos/{id}/thumb"),
        ];
        for path in &paths {
            let res = tower::ServiceExt::oneshot(app.router.clone(), bearer_at(&secret, path))
                .await
                .unwrap();
            assert_eq!(res.status(), StatusCode::OK, "{path}");
        }
        store::kiosk::delete(&app.state.pool, view.id)
            .await
            .unwrap();
        for path in &paths {
            let (status, _) = send(&app.router, bearer_at(&secret, path)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{path}");
        }
        let (status, _) = send(&app.router, get("/api/lagemonitor/v1/photos")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "ohne Token");
    }

    #[tokio::test]
    async fn without_a_running_mission_there_are_no_photos() {
        let app = test_app().await;
        let secret = hub_token(&app).await;
        let id = ready_photo(&app, "DJI_1_T.JPG", 10, Some((52.96, 10.56))).await;
        let (status, json) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/photos"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json, serde_json::json!([]));
        let path = format!("/api/lagemonitor/v1/photos/{id}/thumb");
        let (status, _) = send(&app.router, bearer_at(&secret, &path)).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "Foto ohne Einsatz");
        let (_, lage) = send(&app.router, bearer(&secret)).await;
        assert_eq!(lage["photos_latest_ms"], Value::Null);
    }

    #[tokio::test]
    async fn only_shots_of_the_running_mission_with_a_position_go_out() {
        let app = test_app().await;
        let secret = hub_token(&app).await;
        let old = running(&app, "Gestern").await;
        let foreign = ready_photo(&app, "DJI_1_T.JPG", 10, Some((52.9, 10.5))).await;
        app.state.missions.close(old, 20).await.unwrap();
        running(&app, "Heute").await;
        let thermal = ready_photo(&app, "DJI_2_T.JPG", 100, Some((52.96, 10.56))).await;
        let wide = ready_photo(&app, "DJI_2_W.JPG", 100, Some((52.96, 10.56))).await;
        ready_photo(&app, "DJI_3_T.JPG", 200, None).await;
        let later = ready_photo(&app, "DJI_4_T.JPG", 300, Some((52.97, 10.57))).await;

        let (status, json) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/photos"),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let shots: Vec<LmPhotoShot> = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(
            shots.iter().map(|s| s.id).collect::<Vec<_>>(),
            [thermal, later],
            "ohne fremden Einsatz und ohne Position: {json}"
        );
        assert_eq!(shots[0].taken_at_ms, 100);
        assert_eq!(shots[0].relative_altitude_m, Some(60.0));
        assert_eq!(shots[0].gimbal_yaw_deg, Some(-90.0));
        assert_eq!(
            shots[0].photos.iter().map(|p| p.lens).collect::<Vec<_>>(),
            [PhotoLens::Thermal, PhotoLens::Wide]
        );
        assert_eq!(
            shots[0].photos[1].thumb_url,
            format!("/api/lagemonitor/v1/photos/{wide}/thumb?v=105")
        );
        let text = json.to_string();
        assert!(!text.contains("RCSN1") && !text.contains("ACSN1"), "{text}");
        assert!(!text.contains("original"), "{text}");

        let (_, json) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/photos?since_ms=300"),
        )
        .await;
        assert_eq!(json.as_array().unwrap().len(), 1, "{json}");
        let (_, lage) = send(&app.router, bearer(&secret)).await;
        assert_eq!(lage["photos_latest_ms"], 300);

        let res = tower::ServiceExt::oneshot(
            app.router.clone(),
            bearer_at(&secret, &shots[0].photos[1].thumb_url),
        )
        .await
        .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "image/jpeg");
        assert_eq!(res.headers()["cache-control"], THUMB_CACHE_CONTROL);
        let path = format!("/api/lagemonitor/v1/photos/{foreign}/thumb");
        let (status, _) = send(&app.router, bearer_at(&secret, &path)).await;
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "Foto eines anderen Einsatzes"
        );
        let (status, _) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/photos/999/thumb"),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn photos_are_gzipped_when_asked() {
        let app = test_app().await;
        let secret = hub_token(&app).await;
        running(&app, "Übung").await;
        // Eine leere Liste ist zu klein für gzip.
        for n in 1..=5 {
            ready_photo(&app, &format!("DJI_{n}_T.JPG"), n, Some((52.96, 10.56))).await;
        }
        let mut req = bearer_at(&secret, "/api/lagemonitor/v1/photos");
        req.headers_mut()
            .insert(axum::http::header::ACCEPT_ENCODING, "gzip".parse().unwrap());
        let res = tower::ServiceExt::oneshot(app.router.clone(), req)
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[axum::http::header::CONTENT_ENCODING], "gzip");
    }

    #[tokio::test]
    async fn the_thumbnail_for_lifeline_hub_carries_no_exif() {
        use http_body_util::BodyExt;

        use crate::photos::thumb::{has_exif, sample_jpeg_with_gps};

        let app = test_app().await;
        let seen = app.with_photo_storage().await;
        let secret = hub_token(&app).await;
        running(&app, "Übung").await;
        let original = sample_jpeg_with_gps(1600, 1200);
        assert!(has_exif(&original));
        seen.lock().unwrap().objects.insert(
            format!("/{}/fotos/RC1/DJI_1_T.JPG", crate::storage::fake::BUCKET),
            original,
        );
        let mut shots = app.state.photos.subscribe();
        let photo = crate::store::photos::NewPhoto {
            object_key: "fotos/RC1/DJI_1_T.JPG".into(),
            name: "DJI_1_T.JPG".into(),
            fingerprint: None,
            tiny_fingerprint: None,
            shot_key: crate::photos::rules::shot_key("RC1", "DJI_1_T.JPG"),
            lens: PhotoLens::Thermal,
            gateway_sn: "RC1".into(),
            aircraft_sn: None,
            flight_id: None,
            taken_at_ms: Some(10),
            uploaded_at_ms: 10,
            lat: Some(52.96),
            lon: Some(10.56),
            absolute_altitude_m: None,
            relative_altitude_m: None,
            gimbal_yaw_deg: None,
            payload_model_key: None,
        };
        app.state.photos.accept(photo).await.unwrap();
        let _ = shots.recv().await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(20), shots.recv())
            .await
            .expect("Vorschaubild innerhalb von 20 s")
            .unwrap();
        let (_, json) = send(
            &app.router,
            bearer_at(&secret, "/api/lagemonitor/v1/photos"),
        )
        .await;
        let url = json[0]["photos"][0]["thumb_url"]
            .as_str()
            .unwrap()
            .to_string();
        let res = tower::ServiceExt::oneshot(app.router.clone(), bearer_at(&secret, &url))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert!(!has_exif(&bytes), "Vorschaubild ohne EXIF und GPS");
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (640, 480));
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

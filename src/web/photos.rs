//! Fotos für den Browser (M3): Auslösungen der Lagekarte und eines Flugs, Vorschaubild und
//! Original. Der Browser spricht nie mit dem Fotospeicher: Er liegt im VPN, mobil fehlt das oft.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::app::AppState;
use crate::auth::session::{Admin, Operator, Viewer};
use crate::contract::PhotoShot;
use crate::error::AppError;
use crate::time::now_ms;
use crate::web::missions::parse_scope;

/// So weit zurück zeigt die Lagekarte Fotos, wenn der Browser nichts anderes verlangt.
pub const RECENT_MS: i64 = 24 * 3_600_000;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/photos", get(recent))
        .route("/photos/{id}/thumb", get(thumb))
        .route("/photos/{id}/original", get(original))
        .route(
            "/photos/shots/{shot_id}",
            axum::routing::delete(delete_shot),
        )
        .route("/flights/{id}/photos", get(of_flight))
}

#[derive(Deserialize)]
struct RecentQuery {
    since_ms: Option<i64>,
    mission: Option<String>,
}

/// Auslösungen für die Lagekarte, auch für das Großbild.
async fn recent(
    _: Viewer,
    State(state): State<AppState>,
    Query(q): Query<RecentQuery>,
) -> Result<Json<Vec<PhotoShot>>, AppError> {
    let since = q.since_ms.unwrap_or_else(|| now_ms() - RECENT_MS);
    Ok(Json(match parse_scope(q.mission.as_deref())? {
        Some(Some(mission)) => state.photos.in_mission(mission).await?,
        Some(None) => state.photos.recent_unassigned(since).await?,
        None => state.photos.recent(since).await?,
    }))
}

/// Alle Fotos eines Flugs; wie das Flugdetail nur für Konten.
async fn of_flight(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<PhotoShot>>, AppError> {
    Ok(Json(state.photos.of_flight(id).await?))
}

async fn thumb(
    _: Viewer,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Response, AppError> {
    // Ein Job kann nach dem Löschen noch eine Datei schreiben, auch für eine neu vergebene ID; die
    // gilt erst, wenn der Job dieser Zeile sie als fertig markiert hat.
    if !state.photos.thumb_ready(id).await? {
        return Err(AppError::NotFound);
    }
    let bytes = match tokio::fs::read(state.photos.thumb_path(id)).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(AppError::NotFound),
        Err(e) => return Err(AppError::Internal(format!("Vorschaubild {id}: {e}"))),
    };
    Response::builder()
        .header(header::CONTENT_TYPE, "image/jpeg")
        // Ein Vorschaubild ändert sich nie; nur die Sitzung entscheidet, wer es sieht.
        .header(header::CACHE_CONTROL, "private, max-age=86400, immutable")
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Alle Dateien einer Auslösung. Die ID enthält `/` (`RC1/DJI_…`) und kommt URL-kodiert.
async fn delete_shot(
    _: Admin,
    State(state): State<AppState>,
    Path(shot_id): Path<String>,
) -> Result<StatusCode, AppError> {
    let d = state.deleter.clone();
    crate::deletion::detached(async move { d.delete_shot(&shot_id).await }).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct OriginalQuery {
    #[serde(default)]
    download: u8,
}

/// Nur passive Rasterformate zeigt der Browser direkt an, alles andere wird Download.
fn inline_type(name: &str) -> Option<&'static str> {
    let ext = name.rsplit_once('.')?.1.to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        _ => None,
    }
}

/// Dateiname für `Content-Disposition`: nur unverfängliche ASCII-Zeichen.
fn safe_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-') {
                c
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.trim_matches('.').is_empty() {
        "foto.jpg".into()
    } else {
        cleaned
    }
}

/// Original aus dem Fotospeicher als Strom, zum Ansehen oder mit `download=1` als Datei.
async fn original(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<OriginalQuery>,
) -> Result<Response, AppError> {
    let (object_key, name) = state.photos.original_of(id).await?;
    let res = state.photos.original(&object_key).await?;
    // Typ und Inhalt bestimmt die Fernsteuerung; nosniff und sandbox halten ein untergeschobenes
    // SVG oder HTML davon ab, im Ursprung der App zu laufen.
    let inline = inline_type(&name);
    let mut out = Response::builder()
        .header(
            header::CONTENT_TYPE,
            inline.unwrap_or("application/octet-stream"),
        )
        .header(header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(
            header::CONTENT_SECURITY_POLICY,
            "default-src 'none'; sandbox",
        )
        .header(header::CACHE_CONTROL, "private, max-age=3600");
    if let Some(len) = res.content_length() {
        out = out.header(header::CONTENT_LENGTH, len);
    }
    if q.download == 1 || inline.is_none() {
        out = out.header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", safe_filename(&name)),
        );
    }
    out.body(Body::from_stream(res.bytes_stream()))
        .map_err(|e| AppError::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use axum::http::StatusCode;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;
    use crate::contract::{PhotoStorageSettingsInput, Role};
    use crate::photos::thumb::sample_jpeg;
    use crate::storage::fake;
    use crate::store::photos::NewPhoto;
    use crate::testing::{TestApp, get, send, test_app, with_cookie};

    async fn with_storage() -> (TestApp, fake::Shared) {
        let app = test_app().await;
        let (url, seen) = fake::start().await;
        let cfg = fake::settings(&url);
        app.state
            .settings
            .update_photo_storage(PhotoStorageSettingsInput {
                s3_url: cfg.s3_url,
                rc_url: cfg.rc_url,
                admin_url: cfg.admin_url,
                bucket: cfg.bucket,
                region: cfg.region,
                access_key_id: cfg.access_key_id,
                secret_access_key: Some(cfg.secret_access_key),
                admin_token: Some(cfg.admin_token),
            })
            .await
            .unwrap();
        (app, seen)
    }

    fn photo(name: &str, flight_id: Option<i64>, taken_at_ms: i64) -> NewPhoto {
        NewPhoto {
            object_key: format!("fotos/RC1/{name}"),
            name: name.into(),
            fingerprint: None,
            tiny_fingerprint: None,
            shot_key: crate::photos::rules::shot_key("RC1", name),
            lens: crate::photos::rules::lens_of(name),
            gateway_sn: "RC1".into(),
            aircraft_sn: None,
            flight_id,
            taken_at_ms: Some(taken_at_ms),
            uploaded_at_ms: taken_at_ms,
            lat: Some(52.96),
            lon: Some(10.56),
            absolute_altitude_m: None,
            relative_altitude_m: None,
            gimbal_yaw_deg: None,
            payload_model_key: None,
        }
    }

    #[tokio::test]
    async fn the_thumbnail_follows_the_photo_and_the_original_streams_through() {
        let (app, seen) = with_storage().await;
        let original = sample_jpeg(1600, 1200);
        seen.lock().unwrap().objects.insert(
            format!("/{}/fotos/RC1/DJI_1_T.JPG", fake::BUCKET),
            original.clone(),
        );
        let mut shots = app.state.photos.subscribe();
        let at = now_ms();
        let (id, first) = app
            .state
            .photos
            .accept(photo("DJI_1_T.JPG", None, at))
            .await
            .unwrap();
        assert_eq!(
            first.photos[0].thumb_url, None,
            "Vorschaubild entsteht noch"
        );
        let _ = shots.recv().await.unwrap();
        let ready = tokio::time::timeout(Duration::from_secs(20), shots.recv())
            .await
            .expect("Vorschaubild innerhalb von 20 s")
            .unwrap();
        assert_eq!(
            ready.photos[0].thumb_url.as_deref(),
            Some(format!("/api/photos/{id}/thumb?v={at}").as_str())
        );

        // Die Version in der Adresse ist nur für den Browser-Cache; die Handler übergehen sie.
        let viewer = app.cookie(Role::Kiosk).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(
                get(ready.photos[0].thumb_url.as_deref().unwrap()),
                &viewer,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()["content-type"], "image/jpeg");
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        let img = image::load_from_memory(&bytes).unwrap();
        assert_eq!((img.width(), img.height()), (640, 480));

        let (status, _) = send(
            &app.router,
            with_cookie(get(&format!("/api/photos/{id}/original")), &viewer),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "Original nur für Konten");
        let admin = app.cookie(Role::Admin).await;
        let res = app
            .router
            .clone()
            .oneshot(with_cookie(
                get(&format!("{}&download=1", ready.photos[0].original_url)),
                &admin,
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(
            res.headers()["content-disposition"],
            "attachment; filename=\"DJI_1_T.JPG\""
        );
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.to_vec(), original);
    }

    #[tokio::test]
    async fn lists_by_time_and_flight_with_their_roles() {
        let (app, _) = with_storage().await;
        let now = now_ms();
        crate::store::flights::insert(
            &app.state.pool,
            &crate::contract::FlightSummary {
                id: 6,
                aircraft_sn: "AC1".into(),
                gateway_sn: "RC1".into(),
                started_at_ms: now - 2 * RECENT_MS - 60_000,
                ended_at_ms: Some(now - 2 * RECENT_MS + 60_000),
                end_reason: Some(crate::contract::FlightEndReason::Landed),
                max_elevation_m: None,
                distance_m: 0.0,
            },
        )
        .await
        .unwrap();
        let photos = &app.state.photos;
        photos
            .accept(photo("DJI_1_T.JPG", Some(6), now - 2 * RECENT_MS))
            .await
            .unwrap();
        photos
            .accept(photo("DJI_2_T.JPG", None, now - 60_000))
            .await
            .unwrap();
        photos
            .accept(photo("DJI_2_W.JPG", None, now - 60_000))
            .await
            .unwrap();
        let kiosk = app.cookie(Role::Kiosk).await;
        let (status, recent) = send(&app.router, with_cookie(get("/api/photos"), &kiosk)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(recent.as_array().unwrap().len(), 1, "letzte 24 h: {recent}");
        assert_eq!(recent[0]["id"], "RC1/DJI_2");
        assert_eq!(recent[0]["photos"].as_array().unwrap().len(), 2);
        assert_eq!(recent[0]["photos"][0]["lens"], "thermal");
        let (_, all) = send(
            &app.router,
            with_cookie(get("/api/photos?since_ms=0"), &kiosk),
        )
        .await;
        assert_eq!(all.as_array().unwrap().len(), 2);

        let (status, _) = send(
            &app.router,
            with_cookie(get("/api/flights/6/photos"), &kiosk),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "Flugdetail nur für Konten");
        let admin = app.cookie(Role::Admin).await;
        let (status, flight) = send(
            &app.router,
            with_cookie(get("/api/flights/6/photos"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(flight[0]["id"], "RC1/DJI_1");
        assert_eq!(flight[0]["photos"][0]["lens"], "thermal");
        assert_eq!(
            send(&app.router, get("/api/photos")).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn photos_of_a_mission_ignore_the_24h_window() {
        let (app, _) = with_storage().await;
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
        let now = now_ms();
        let photos = &app.state.photos;
        let (old, _) = photos
            .accept(photo("DJI_1_T.JPG", None, now - 3 * RECENT_MS))
            .await
            .unwrap();
        photos
            .accept(photo("DJI_2_T.JPG", None, now - 60_000))
            .await
            .unwrap();
        photos
            .accept(photo("DJI_3_T.JPG", None, now - 2 * RECENT_MS))
            .await
            .unwrap();
        sqlx::query("UPDATE photos SET mission_id = ? WHERE id = ?")
            .bind(mission)
            .bind(old)
            .execute(&app.state.pool)
            .await
            .unwrap();
        let kiosk = app.cookie(Role::Kiosk).await;
        let ids = |uri: String| {
            let router = app.router.clone();
            let kiosk = kiosk.clone();
            async move {
                let (status, list) = send(&router, with_cookie(get(&uri), &kiosk)).await;
                assert_eq!(status, StatusCode::OK, "{list}");
                list.as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s["id"].as_str().unwrap().to_string())
                    .collect::<Vec<_>>()
            }
        };
        assert_eq!(
            ids(format!("/api/photos?mission={mission}")).await,
            ["RC1/DJI_1"]
        );
        assert_eq!(ids("/api/photos?mission=none".into()).await, ["RC1/DJI_2"]);
        assert_eq!(ids("/api/photos".into()).await, ["RC1/DJI_2"]);
    }

    #[tokio::test]
    async fn missing_thumbnail_or_photo_is_404() {
        let (app, _) = with_storage().await;
        let admin = app.cookie(Role::Admin).await;
        for uri in ["/api/photos/99/thumb", "/api/photos/99/original"] {
            assert_eq!(
                send(&app.router, with_cookie(get(uri), &admin)).await.0,
                StatusCode::NOT_FOUND,
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn a_stale_thumbnail_file_is_not_served_before_its_row_is_ready() {
        let (app, _) = with_storage().await;
        // Wie nach einem späten Job für eine gelöschte, neu vergebene ID: Datei da, Zeile nicht fertig.
        let id = crate::store::photos::upsert(&app.state.pool, &photo("DJI_9_T.JPG", None, 1))
            .await
            .unwrap();
        let path = app.state.photos.thumb_path(id);
        tokio::fs::create_dir_all(path.parent().unwrap())
            .await
            .unwrap();
        tokio::fs::write(&path, b"alt").await.unwrap();
        let viewer = app.cookie(Role::Kiosk).await;
        let (status, _) = send(
            &app.router,
            with_cookie(get(&format!("/api/photos/{id}/thumb")), &viewer),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[test]
    fn only_raster_originals_show_inline() {
        assert_eq!(inline_type("DJI_1_T.JPG"), Some("image/jpeg"));
        assert_eq!(inline_type("a.png"), Some("image/png"));
        assert_eq!(inline_type("DJI_1.svg"), None);
        assert_eq!(inline_type("DJI_1.DNG"), None);
        assert_eq!(inline_type("ohne_endung"), None);
    }

    #[test]
    fn download_names_stay_harmless() {
        assert_eq!(safe_filename("DJI_1_T.JPG"), "DJI_1_T.JPG");
        assert_eq!(safe_filename("a\"b\r\nc.jpg"), "a_b__c.jpg");
        assert_eq!(safe_filename(".."), "foto.jpg");
    }
}

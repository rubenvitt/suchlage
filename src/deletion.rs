//! Löschen von Einsätzen, Flügen und Medien (nur Admin). Erst die Dateien (Fotospeicher,
//! Vorschaubilder, Video), dann die Datenbankzeilen in einer Transaktion: Scheitert eine Datei mit
//! etwas anderem als „nicht vorhanden“, bleibt die Datenbank unverändert, ein zweiter Versuch
//! räumt den Rest.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

use crate::contract::{FlightSummary, MissionContents, MissionState};
use crate::error::AppError;
use crate::map::{MapChange, MapHub};
use crate::media::recordings;
use crate::missions::MissionHub;
use crate::photos::PhotoHub;
use crate::store;

#[derive(Clone)]
pub struct Deleter {
    pub pool: SqlitePool,
    pub photos: PhotoHub,
    pub map: MapHub,
    pub missions: MissionHub,
    /// `SUCHLAGE_DATA_DIR/video`, dasselbe Volume wie `/recordings` bei MediaMTX.
    pub video_dir: PathBuf,
}

/// Alle Dateien einer Löschung, gesammelt, bevor die erste fällt.
struct Files {
    /// Foto-ID und Objektschlüssel im Fotospeicher.
    photos: Vec<(i64, String)>,
    videos: Vec<PathBuf>,
}

impl Files {
    fn photo_ids(&self) -> Vec<i64> {
        self.photos.iter().map(|(id, _)| *id).collect()
    }
}

impl Deleter {
    /// Mengen für die Rückfrage vor dem Löschen eines Einsatzes.
    pub async fn contents(&self, mission_id: i64) -> Result<MissionContents, AppError> {
        self.missions.get(mission_id).await?;
        let (flights, photos, map_elements) =
            store::missions::counts(&self.pool, mission_id).await?;
        let videos = self
            .videos(&store::flights::of_mission(&self.pool, mission_id).await?)
            .await?;
        Ok(MissionContents {
            flights,
            photos,
            map_elements,
            video_segments: videos.len() as u32,
        })
    }

    pub async fn delete_mission(&self, id: i64) -> Result<(), AppError> {
        // Die ganze Löschung unter der Einsatzsperre: Kein Start und keine Zuordnung dazwischen.
        let _serial = self.missions.serial().await;
        if self.missions.get(id).await?.state == MissionState::Running {
            return Err(AppError::Unprocessable(
                "Ein laufender Einsatz lässt sich nicht löschen".into(),
            ));
        }
        let flights = store::flights::of_mission(&self.pool, id).await?;
        ensure_ended(&flights)?;
        // Wie `store::missions::delete_rows`: Fotos des Einsatzes und Fotos seiner Flüge.
        let mut photo_ids: BTreeSet<i64> = store::photos::of_mission_all(&self.pool, id)
            .await?
            .iter()
            .map(|p| p.id)
            .collect();
        for f in &flights {
            photo_ids.extend(
                store::photos::of_flight(&self.pool, f.id)
                    .await?
                    .iter()
                    .map(|p| p.id),
            );
        }
        let files = self.collect(photo_ids, &flights).await?;
        self.remove(&files).await?;
        let elements = store::missions::delete_rows(&self.pool, id, &files.photo_ids())
            .await?
            .ok_or(AppError::NotFound)?;
        for id in elements {
            self.map.publish(MapChange::Deleted { id });
        }
        self.missions.changed();
        Ok(())
    }

    /// Flug mit Spur, Video und seinen Fotos.
    pub async fn delete_flight(&self, id: i64) -> Result<(), AppError> {
        let flight = self.ended_flight(id).await?;
        let photo_ids = store::photos::of_flight(&self.pool, id)
            .await?
            .iter()
            .map(|p| p.id)
            .collect();
        let files = self.collect(photo_ids, &[flight]).await?;
        self.remove(&files).await?;
        if !store::flights::delete_rows(&self.pool, id, &files.photo_ids()).await? {
            return Err(AppError::NotFound);
        }
        self.missions.changed();
        Ok(())
    }

    /// Nur die Aufzeichnung; der Flug bleibt.
    pub async fn delete_flight_video(&self, id: i64) -> Result<(), AppError> {
        let flight = self.ended_flight(id).await?;
        for path in self.videos(&[flight]).await? {
            remove_file(&path).await?;
        }
        self.missions.changed();
        Ok(())
    }

    /// Alle Dateien einer Auslösung (alle Linsen).
    pub async fn delete_shot(&self, shot_key: &str) -> Result<(), AppError> {
        let photo_ids: BTreeSet<i64> = store::photos::of_shot(&self.pool, shot_key)
            .await?
            .iter()
            .map(|p| p.id)
            .collect();
        if photo_ids.is_empty() {
            return Err(AppError::NotFound);
        }
        let files = self.collect(photo_ids, &[]).await?;
        self.remove(&files).await?;
        store::photos::delete_ids(&self.pool, &files.photo_ids()).await?;
        self.missions.changed();
        Ok(())
    }

    async fn ended_flight(&self, id: i64) -> Result<FlightSummary, AppError> {
        let flight = store::flights::get(&self.pool, id)
            .await?
            .ok_or(AppError::NotFound)?;
        ensure_ended(std::slice::from_ref(&flight))?;
        Ok(flight)
    }

    /// Alles, was scheitern kann, bevor etwas gelöscht ist: Fotospeicher eingerichtet,
    /// Objektschlüssel, Videoablage eingehängt.
    async fn collect(
        &self,
        photo_ids: BTreeSet<i64>,
        flights: &[FlightSummary],
    ) -> Result<Files, AppError> {
        if !photo_ids.is_empty() {
            self.photos.storage()?;
        }
        let mut photos = Vec::new();
        for id in photo_ids {
            if let Some(key) = store::photos::object_key(&self.pool, id).await? {
                photos.push((id, key));
            }
        }
        Ok(Files {
            photos,
            videos: self.videos(flights).await?,
        })
    }

    /// Fotospeicher, Vorschaubilder, Video – in dieser Reihenfolge, beim ersten Fehler Schluss.
    async fn remove(&self, files: &Files) -> Result<(), AppError> {
        for (_, key) in &files.photos {
            self.photos.delete_object(key).await?;
        }
        for (id, _) in &files.photos {
            remove_file(&self.photos.thumb_path(*id)).await?;
        }
        for path in &files.videos {
            remove_file(path).await?;
        }
        Ok(())
    }

    /// Segmente, die mit den Flügen gehen. Je Fernsteuerung bleibt, was ein anderer Flug noch
    /// braucht, auch ein laufender, dessen Segment MediaMTX gerade schreibt.
    async fn videos(&self, deleting: &[FlightSummary]) -> Result<Vec<PathBuf>, AppError> {
        let window = |f: &FlightSummary| (f.started_at_ms, f.ended_at_ms.unwrap_or(i64::MAX));
        let controllers: BTreeSet<&str> = deleting.iter().map(|f| f.gateway_sn.as_str()).collect();
        let mut out = Vec::new();
        for rc in controllers {
            let segments = recordings::scan(&self.video_dir, rc).map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => {
                    AppError::Unavailable("Videoablage nicht eingehängt".into())
                }
                _ => AppError::Internal(format!("Videoablage {rc}: {e}")),
            })?;
            let delete: Vec<_> = deleting
                .iter()
                .filter(|f| f.gateway_sn == rc)
                .map(window)
                .collect();
            // Nach ID, nicht nach Datenbank: Beim Löschen nur des Videos bleibt der Flug stehen.
            let keep: Vec<_> = store::flights::of_gateway(&self.pool, rc)
                .await?
                .iter()
                .filter(|f| !deleting.iter().any(|d| d.id == f.id))
                .map(window)
                .collect();
            out.extend(recordings::to_delete(&segments, &delete, &keep));
        }
        Ok(out)
    }
}

fn ensure_ended(flights: &[FlightSummary]) -> Result<(), AppError> {
    if flights.iter().any(|f| f.ended_at_ms.is_none()) {
        return Err(AppError::Unprocessable(
            "Ein laufender Flug lässt sich nicht löschen".into(),
        ));
    }
    Ok(())
}

/// Fehlt die Datei schon, ist das auch erledigt.
async fn remove_file(path: &Path) -> Result<(), AppError> {
    match tokio::fs::remove_file(path).await {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(AppError::Internal(format!(
            "{} nicht gelöscht: {e}",
            path.display()
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use axum::http::StatusCode;
    use serde_json::json;

    use crate::contract::{
        FlightEndReason, FlightSummary, MapElementKind, MapElementOrigin, MapPoint, MissionCreate,
        Role,
    };
    use crate::map::{MapChange, NewElement};
    use crate::storage::fake;
    use crate::store::photos::NewPhoto;
    use crate::testing::{TestApp, delete, get, put_json, send, test_app, with_cookie};

    const MIN: i64 = 60_000;
    /// 18.10.2026 12:00:00 UTC
    const T: i64 = 1_792_324_800_000;

    fn flight(id: i64, rc: &str, start: i64, end: Option<i64>) -> FlightSummary {
        FlightSummary {
            id,
            aircraft_sn: format!("AC{id}"),
            gateway_sn: rc.into(),
            started_at_ms: start,
            ended_at_ms: end,
            end_reason: end.map(|_| FlightEndReason::Landed),
            max_elevation_m: None,
            distance_m: 0.0,
        }
    }

    /// Segmentdatei wie MediaMTX sie schreibt; liefert den Pfad.
    fn segment(app: &TestApp, rc: &str, start_ms: i64) -> std::path::PathBuf {
        let dir = app.state.boot.video_dir().join("live").join(rc);
        std::fs::create_dir_all(&dir).unwrap();
        let name = chrono::DateTime::from_timestamp_millis(start_ms)
            .unwrap()
            .format("%Y-%m-%d_%H-%M-%S-%6f.mp4")
            .to_string();
        let path = dir.join(name);
        std::fs::write(&path, b"mp4").unwrap();
        path
    }

    /// Foto mit Objekt im Fake-Speicher und Vorschaubild; liefert die ID.
    async fn photo(app: &TestApp, seen: &fake::Shared, name: &str, flight: Option<i64>) -> i64 {
        let object_key = format!("fotos/RC1/{name}");
        let id = crate::store::photos::upsert(
            &app.state.pool,
            &NewPhoto {
                object_key: object_key.clone(),
                name: name.into(),
                fingerprint: None,
                tiny_fingerprint: None,
                shot_key: crate::photos::rules::shot_key("RC1", name),
                lens: crate::photos::rules::lens_of(name),
                gateway_sn: "RC1".into(),
                aircraft_sn: None,
                flight_id: flight,
                taken_at_ms: Some(T + 5 * MIN),
                uploaded_at_ms: T + 5 * MIN,
                lat: None,
                lon: None,
                absolute_altitude_m: None,
                relative_altitude_m: None,
                gimbal_yaw_deg: None,
                payload_model_key: None,
            },
        )
        .await
        .unwrap();
        seen.lock()
            .unwrap()
            .objects
            .insert(format!("/{}/{object_key}", fake::BUCKET), b"jpg".to_vec());
        let thumb = app.state.photos.thumb_path(id);
        std::fs::create_dir_all(thumb.parent().unwrap()).unwrap();
        std::fs::write(&thumb, b"thumb").unwrap();
        id
    }

    async fn mission(app: &TestApp, name: &str) -> i64 {
        app.state
            .missions
            .create(
                MissionCreate {
                    name: name.into(),
                    location: None,
                },
                "u",
                1,
            )
            .await
            .unwrap()
            .id
    }

    struct Filled {
        mission: i64,
        photos: Vec<i64>,
        element: String,
        /// Segmente bei 0 und 10 min (gehen mit Flug 1), 20 min (kein Flug), 40 min (Flug 2).
        segments: Vec<std::path::PathBuf>,
    }

    /// Abgeschlossener Einsatz mit Flug 1 (RC1, 2–15 min), Foto mit und ohne Flug, Kartenelement;
    /// daneben Flug 2 ohne Einsatz (RC1, 40–45 min).
    async fn filled(app: &TestApp, seen: &fake::Shared) -> Filled {
        let id = mission(app, "Wald").await;
        app.state.missions.start(id, T).await.unwrap();
        crate::store::flights::insert(
            &app.state.pool,
            &flight(1, "RC1", T + 2 * MIN, Some(T + 15 * MIN)),
        )
        .await
        .unwrap();
        let photos = vec![
            photo(app, seen, "DJI_1_T.JPG", Some(1)).await,
            photo(app, seen, "DJI_2_W.JPG", None).await,
        ];
        let element = app
            .state
            .map
            .create(
                NewElement {
                    id: None,
                    name: "Ziel".into(),
                    kind: MapElementKind::Point,
                    points: vec![MapPoint {
                        lat: 52.0,
                        lon: 10.0,
                        alt_m: None,
                    }],
                    color: "#ff0000".into(),
                    clamp_to_ground: true,
                    author: "u".into(),
                    origin: MapElementOrigin::Web,
                    mission_id: Some(id),
                },
                T,
            )
            .await
            .unwrap()
            .id;
        app.state.missions.close(id, T + 30 * MIN).await.unwrap();
        crate::store::flights::insert(
            &app.state.pool,
            &flight(2, "RC1", T + 40 * MIN, Some(T + 45 * MIN)),
        )
        .await
        .unwrap();
        let segments = [0, 10, 20, 40]
            .iter()
            .map(|m| segment(app, "RC1", T + m * MIN))
            .collect();
        Filled {
            mission: id,
            photos,
            element,
            segments,
        }
    }

    async fn as_admin() -> (TestApp, fake::Shared, String) {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        let seen = app.with_photo_storage().await;
        (app, seen, admin)
    }

    async fn del(app: &TestApp, cookie: &str, uri: &str) -> (StatusCode, serde_json::Value) {
        send(&app.router, with_cookie(delete(uri), cookie)).await
    }

    fn exists(p: &Path) -> bool {
        p.exists()
    }

    #[tokio::test]
    async fn operator_and_kiosk_cannot_delete() {
        let (app, _, _) = as_admin().await;
        for role in [Role::Operator, Role::Kiosk] {
            let cookie = app.cookie(role).await;
            for uri in [
                "/api/missions/1",
                "/api/flights/1",
                "/api/flights/1/video",
                "/api/photos/shots/RC1%2FDJI_1",
            ] {
                assert_eq!(
                    del(&app, &cookie, uri).await.0,
                    StatusCode::FORBIDDEN,
                    "{role:?} {uri}"
                );
            }
            let (status, _) = send(
                &app.router,
                with_cookie(get("/api/missions/1/contents"), &cookie),
            )
            .await;
            assert_eq!(status, StatusCode::FORBIDDEN, "{role:?} contents");
        }
    }

    #[tokio::test]
    async fn running_mission_or_flight_is_422() {
        let (app, _, admin) = as_admin().await;
        let id = mission(&app, "A").await;
        app.state.missions.start(id, T).await.unwrap();
        crate::store::flights::insert(&app.state.pool, &flight(1, "RC1", T, None))
            .await
            .unwrap();
        for uri in [
            format!("/api/missions/{id}"),
            "/api/flights/1".into(),
            "/api/flights/1/video".into(),
        ] {
            assert_eq!(
                del(&app, &admin, &uri).await.0,
                StatusCode::UNPROCESSABLE_ENTITY,
                "{uri}"
            );
        }
        for uri in [
            "/api/missions/99",
            "/api/flights/99",
            "/api/flights/99/video",
            "/api/photos/shots/RC1%2Fx",
        ] {
            assert_eq!(
                del(&app, &admin, uri).await.0,
                StatusCode::NOT_FOUND,
                "{uri}"
            );
        }
    }

    #[tokio::test]
    async fn deleting_a_mission_removes_rows_photos_thumbs_and_videos() {
        let (app, seen, admin) = as_admin().await;
        let f = filled(&app, &seen).await;
        seen.lock()
            .unwrap()
            .objects
            .insert(format!("/{}/fotos/fremd.JPG", fake::BUCKET), vec![]);
        let mut map = app.state.map.subscribe();
        let mut changed = app.state.missions.subscribe();
        let (status, _) = del(&app, &admin, &format!("/api/missions/{}", f.mission)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert!(app.state.missions.get(f.mission).await.is_err());
        assert_eq!(
            crate::store::flights::get(&app.state.pool, 1)
                .await
                .unwrap(),
            None
        );
        assert!(
            crate::store::flights::get(&app.state.pool, 2)
                .await
                .unwrap()
                .is_some()
        );
        for id in &f.photos {
            assert_eq!(
                crate::store::photos::get(&app.state.pool, *id)
                    .await
                    .unwrap(),
                None
            );
            assert!(!exists(&app.state.photos.thumb_path(*id)));
        }
        assert_eq!(
            seen.lock()
                .unwrap()
                .objects
                .keys()
                .cloned()
                .collect::<Vec<_>>(),
            [format!("/{}/fotos/fremd.JPG", fake::BUCKET)]
        );
        let left: Vec<bool> = f.segments.iter().map(|p| exists(p)).collect();
        assert_eq!(left, [false, false, true, true]);
        assert_eq!(
            map.try_recv().unwrap(),
            MapChange::Deleted { id: f.element }
        );
        assert!(changed.try_recv().is_ok(), "missions_changed");
        let (status, _) = del(&app, &admin, &format!("/api/missions/{}", f.mission)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn storage_down_deletes_nothing() {
        let (app, seen, admin) = as_admin().await;
        let f = filled(&app, &seen).await;
        seen.lock().unwrap().down = true;
        let (status, _) = del(&app, &admin, &format!("/api/missions/{}", f.mission)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            crate::store::missions::counts(&app.state.pool, f.mission)
                .await
                .unwrap(),
            (1, 2, 1)
        );
        assert!(
            f.photos
                .iter()
                .all(|id| exists(&app.state.photos.thumb_path(*id)))
        );
        assert!(f.segments.iter().all(|p| exists(p)));
        let (status, _) = del(&app, &admin, "/api/flights/1").await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert!(
            crate::store::flights::get(&app.state.pool, 1)
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn missing_video_dir_is_503_only_when_flight_has_video() {
        let (app, seen, admin) = as_admin().await;
        let f = filled(&app, &seen).await;
        std::fs::remove_dir_all(app.state.boot.video_dir()).unwrap();
        for uri in [
            format!("/api/missions/{}", f.mission),
            "/api/flights/1".into(),
            "/api/flights/1/video".into(),
        ] {
            let (status, body) = del(&app, &admin, &uri).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            assert_eq!(body["error"], "Videoablage nicht eingehängt");
        }
        assert_eq!(
            seen.lock().unwrap().objects.len(),
            2,
            "Fotospeicher unberührt"
        );
        let empty = mission(&app, "Leer").await;
        let (status, _) = del(&app, &admin, &format!("/api/missions/{empty}")).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = del(&app, &admin, "/api/photos/shots/RC1%2FDJI_2").await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn contents_counts() {
        let (app, seen, admin) = as_admin().await;
        let f = filled(&app, &seen).await;
        let (status, body) = send(
            &app.router,
            with_cookie(
                get(&format!("/api/missions/{}/contents", f.mission)),
                &admin,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({"flights": 1, "photos": 2, "map_elements": 1, "video_segments": 2})
        );
        let (status, _) = send(
            &app.router,
            with_cookie(get("/api/missions/99/contents"), &admin),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn deleting_a_flight_keeps_the_segment_a_running_flight_writes() {
        let (app, seen, admin) = as_admin().await;
        crate::store::flights::insert(
            &app.state.pool,
            &flight(1, "RC1", T + 2 * MIN, Some(T + 12 * MIN)),
        )
        .await
        .unwrap();
        crate::store::flights::insert(&app.state.pool, &flight(2, "RC1", T + 11 * MIN, None))
            .await
            .unwrap();
        let photo = photo(&app, &seen, "DJI_1_T.JPG", Some(1)).await;
        let segs = [segment(&app, "RC1", T), segment(&app, "RC1", T + 10 * MIN)];
        let (status, _) = del(&app, &admin, "/api/flights/1").await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!((exists(&segs[0]), exists(&segs[1])), (false, true));
        assert_eq!(
            crate::store::photos::get(&app.state.pool, photo)
                .await
                .unwrap(),
            None
        );
        assert!(seen.lock().unwrap().objects.is_empty());
        assert_eq!(
            crate::store::flights::get(&app.state.pool, 1)
                .await
                .unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn flight_video_goes_but_the_flight_stays() {
        let (app, _, admin) = as_admin().await;
        crate::store::flights::insert(
            &app.state.pool,
            &flight(1, "RC1", T + 2 * MIN, Some(T + 8 * MIN)),
        )
        .await
        .unwrap();
        let seg = segment(&app, "RC1", T);
        let other = segment(&app, "RC2", T);
        let mut changed = app.state.missions.subscribe();
        let (status, _) = del(&app, &admin, "/api/flights/1/video").await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!((exists(&seg), exists(&other)), (false, true));
        assert!(
            crate::store::flights::get(&app.state.pool, 1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(changed.try_recv().is_ok());
    }

    #[tokio::test]
    async fn a_shot_is_deleted_by_its_encoded_id() {
        let (app, seen, admin) = as_admin().await;
        let a = photo(&app, &seen, "DJI_1_T.JPG", None).await;
        let b = photo(&app, &seen, "DJI_1_W.JPG", None).await;
        let other = photo(&app, &seen, "DJI_2_W.JPG", None).await;
        let (status, _) = del(&app, &admin, "/api/photos/shots/RC1%2FDJI_1").await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        for id in [a, b] {
            assert_eq!(
                crate::store::photos::get(&app.state.pool, id)
                    .await
                    .unwrap(),
                None
            );
            assert!(!exists(&app.state.photos.thumb_path(id)));
        }
        assert!(
            crate::store::photos::get(&app.state.pool, other)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(seen.lock().unwrap().objects.len(), 1);
        let (status, _) = del(&app, &admin, "/api/photos/shots/RC1%2FDJI_1").await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn start_and_assign_wait_for_a_delete_in_progress() {
        let app = test_app().await;
        let id = mission(&app, "A").await;
        crate::store::flights::insert(&app.state.pool, &flight(1, "RC1", T, Some(T + MIN)))
            .await
            .unwrap();
        let serial = app.state.missions.serial().await;
        let (hub, deleter) = (app.state.missions.clone(), app.state.deleter.clone());
        let delete = tokio::spawn(async move { deleter.delete_mission(id).await });
        let hub2 = hub.clone();
        let start = tokio::spawn(async move { hub2.start(id, T).await });
        let assign = tokio::spawn(async move {
            hub.assign_flight(1, crate::contract::MissionAssign { mission_id: id })
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        for (what, finished) in [
            ("Löschen", delete.is_finished()),
            ("Start", start.is_finished()),
            ("Zuordnen", assign.is_finished()),
        ] {
            assert!(!finished, "{what} wartet auf die Einsatzsperre");
        }
        drop(serial);
        let (deleted, started) = (delete.await.unwrap(), start.await.unwrap());
        let _ = assign.await.unwrap();
        // Wer zuerst kommt, gewinnt; nie beides: Ein laufender Einsatz wird nicht gelöscht.
        match (deleted, started) {
            (Ok(()), Err(crate::error::AppError::NotFound)) => {}
            (Err(crate::error::AppError::Unprocessable(_)), Ok(m)) => assert_eq!(m.id, id),
            other => panic!("Löschen und Start überlappen: {other:?}"),
        }
    }

    #[tokio::test]
    async fn update_renames_running_mission() {
        let app = test_app().await;
        let op = app.cookie(Role::Operator).await;
        let id = mission(&app, "A").await;
        app.state.missions.start(id, T).await.unwrap();
        let put = |uri: &str, name: &str| {
            with_cookie(
                put_json(uri, json!({"name": name, "location": "Nord"})),
                &op,
            )
        };
        let uri = format!("/api/missions/{id}");
        let (status, m) = send(&app.router, put(&uri, " Wald ")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            (m["name"].as_str(), m["location"].as_str()),
            (Some("Wald"), Some("Nord"))
        );
        assert_eq!(
            send(&app.router, put(&uri, " ")).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            send(&app.router, put("/api/missions/99", "B")).await.0,
            StatusCode::NOT_FOUND
        );
        app.state.missions.close(id, T + MIN).await.unwrap();
        assert_eq!(
            send(&app.router, put(&uri, "B")).await.0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
}

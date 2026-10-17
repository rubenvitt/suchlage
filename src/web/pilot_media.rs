//! Medienverwaltung für Pilot 2 (M3): die HTTPS-Endpunkte unter `/media/api/v1/…` und
//! `/storage/api/v1/…`. Zugang über den API-Token aus `/pilot/api/connect` wie bei den
//! Kartenelementen; ohne Fotospeicher-Einstellungen 503.

use axum::body::Bytes;
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::routing::post;
use axum::{Json, Router};
use serde_json::json;

use crate::app::AppState;
use crate::contract::{DeviceLogEntry, DeviceLogLevel, DeviceLogSource};
use crate::dji::map::ok;
use crate::dji::media::{
    self as dji, FastUploadInput, GroupUploadCallbackInput, Sts, TinyFingerprintsInput,
    UploadCallbackInput,
};
use crate::error::AppError;
use crate::photos::rules;
use crate::store::photos::NewPhoto;
use crate::store::remote_controllers::RemoteController;
use crate::time::now_ms;
use crate::web::pilot_map::{DjiResult, parse, pilot, workspace};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/media/api/v1/workspaces/{workspace_id}/files/tiny-fingerprints",
            post(tiny_fingerprints),
        )
        .route(
            "/media/api/v1/workspaces/{workspace_id}/fast-upload",
            post(fast_upload),
        )
        .route(
            "/media/api/v1/workspaces/{workspace_id}/upload-callback",
            post(upload_callback),
        )
        .route(
            "/media/api/v1/workspaces/{workspace_id}/group-upload-callback",
            post(group_upload_callback),
        )
        .route("/storage/api/v1/workspaces/{workspace_id}/sts", post(sts))
}

/// Token, Workspace und Fotospeicher prüfen; liefert die Fernsteuerung mit Seriennummer.
async fn gate(
    state: &AppState,
    workspace_id: &str,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<(RemoteController, String), crate::web::pilot_map::DjiError> {
    let rc = pilot(state, headers, query).await?;
    workspace(state, workspace_id)?;
    state.photos.storage()?;
    // Ein Token gibt es erst nach dem Binden; ohne Seriennummer fehlt der Ablageort.
    let sn = rc.sn.clone().ok_or(AppError::Unauthorized)?;
    Ok((rc, sn))
}

fn log(state: &AppState, rc_sn: &str, level: DeviceLogLevel, summary: String) {
    state.devlog.push(DeviceLogEntry {
        seq: 0,
        at_ms: now_ms(),
        source: DeviceLogSource::Pilot,
        level,
        device_sn: Some(rc_sn.to_string()),
        topic: None,
        method: None,
        summary,
        payload: None,
    });
}

async fn tiny_fingerprints(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    gate(&state, &workspace_id, &headers, query.as_deref()).await?;
    let input: TinyFingerprintsInput = parse(&body)?;
    let known = state
        .photos
        .tiny_fingerprints_known(&input.tiny_fingerprints)
        .await?;
    Ok(Json(ok(dji::known_tiny_fingerprints(&known))))
}

async fn fast_upload(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    gate(&state, &workspace_id, &headers, query.as_deref()).await?;
    let input: FastUploadInput = parse(&body)?;
    if state.photos.fingerprint_known(&input.fingerprint).await? {
        Ok(Json(ok(json!({}))))
    } else {
        Ok(Json(dji::not_uploaded_yet()))
    }
}

async fn sts(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> DjiResult {
    let (_, sn) = gate(&state, &workspace_id, &headers, query.as_deref()).await?;
    let cfg = state.photos.storage()?;
    let now = now_ms();
    let key = state.photos.upload_key(&cfg, &sn, now).await?;
    let prefix = rules::object_key_prefix(&sn);
    Ok(Json(ok(dji::sts(&Sts {
        endpoint: &cfg.rc_url,
        bucket: &cfg.bucket,
        region: &cfg.region,
        object_key_prefix: &prefix,
        access_key_id: &key.access_key_id,
        access_key_secret: &key.secret_access_key,
        expire_s: (key.expires_at_ms - now) / 1000,
    }))))
}

async fn upload_callback(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    let (_, sn) = gate(&state, &workspace_id, &headers, query.as_deref()).await?;
    let input: UploadCallbackInput = parse(&body)?;
    if input.result != 0 {
        // Pilot 2 meldet einen gescheiterten Upload; im Speicher liegt nichts.
        log(
            &state,
            &sn,
            DeviceLogLevel::Warn,
            format!("Foto-Upload gescheitert ({}): {}", input.result, input.name),
        );
        return Ok(Json(ok(json!({ "object_key": input.object_key }))));
    }
    if !rules::object_key_allowed(&input.object_key, &sn) {
        return Err(AppError::Unprocessable(format!(
            "Ablageort muss unter {}/ liegen",
            rules::object_key_prefix(&sn)
        ))
        .into());
    }
    let cfg = state.photos.storage()?;
    if !state.photos.object_exists(&cfg, &input.object_key).await? {
        return Err(AppError::Unprocessable("Foto liegt nicht im Fotospeicher".into()).into());
    }
    let name = rules::display_name(&input.name, &input.object_key);
    let aircraft_sn = input.ext.sn.filter(|s| crate::sn::is_valid_sn(s));
    let taken_at_ms = input
        .metadata
        .created_time
        .as_deref()
        .and_then(rules::taken_at_ms);
    let uploaded_at_ms = now_ms();
    let flight_id = state
        .photos
        .flight_at(
            &sn,
            aircraft_sn.as_deref(),
            taken_at_ms.unwrap_or(uploaded_at_ms),
        )
        .await?;
    let shoot = input.metadata.shoot_position.unwrap_or_default();
    let position = rules::position(shoot.lat, shoot.lng);
    let photo = NewPhoto {
        object_key: input.object_key.clone(),
        shot_key: rules::shot_key(&sn, &name),
        lens: rules::lens_of(&name),
        name,
        fingerprint: input.fingerprint.filter(|f| !f.is_empty()),
        tiny_fingerprint: input.ext.tiny_fingerprint.filter(|f| !f.is_empty()),
        gateway_sn: sn.clone(),
        aircraft_sn,
        flight_id,
        taken_at_ms,
        uploaded_at_ms,
        lat: position.map(|p| p.lat),
        lon: position.map(|p| p.lon),
        absolute_altitude_m: input.metadata.absolute_altitude,
        relative_altitude_m: input.metadata.relative_altitude,
        gimbal_yaw_deg: input.metadata.gimbal_yaw_degree,
        payload_model_key: input.ext.payload_model_key,
    };
    state.photos.accept(photo).await?;
    Ok(Json(ok(json!({ "object_key": input.object_key }))))
}

/// Nur für Wegpunkt-Missionen gedacht, die Suchlage nicht kennt: annehmen und protokollieren.
async fn group_upload_callback(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    let (_, sn) = gate(&state, &workspace_id, &headers, query.as_deref()).await?;
    let input: GroupUploadCallbackInput = parse(&body)?;
    log(
        &state,
        &sn,
        DeviceLogLevel::Info,
        format!(
            "Dateigruppe {}: {} von {} hochgeladen",
            input.file_group_id.as_deref().unwrap_or("ohne ID"),
            input.file_uploaded_count,
            input.file_count
        ),
    );
    Ok(Json(ok(json!({}))))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{Value, json};

    use crate::contract::{PhotoLens, PhotoStorageSettingsInput};
    use crate::storage::fake;
    use crate::testing::{TestApp, send, test_app};

    const WS: &str = "768d4a2e-1f34-477c-9fcc-872285e895f6";
    const TOKEN: &str = "pilot-token";

    /// DJI eingerichtet, RC1 gekoppelt mit `TOKEN`; mit `storage` zeigt der Fotospeicher auf den Fake.
    async fn paired(storage: bool) -> (TestApp, Option<fake::Shared>) {
        let app = test_app().await;
        let s = &app.state.settings;
        s.update_dji(crate::contract::DjiSettingsInput {
            app_id: "APPID".into(),
            app_key: Some("K".into()),
            license: Some("L".into()),
            workspace_id: WS.into(),
        })
        .await
        .unwrap();
        let pool = &app.state.pool;
        let (rc, _) = crate::store::remote_controllers::create(pool, "RC Plus 1", 0)
            .await
            .unwrap();
        crate::store::remote_controllers::bind(pool, rc.id, "RC1", "pw", "key", 1)
            .await
            .unwrap();
        crate::store::remote_controllers::set_api_token(pool, rc.id, TOKEN)
            .await
            .unwrap();
        if !storage {
            return (app, None);
        }
        let (url, seen) = fake::start().await;
        let cfg = fake::settings(&url);
        s.update_photo_storage(PhotoStorageSettingsInput {
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
        (app, Some(seen))
    }

    fn post(path: &str, body: Value) -> Request<Body> {
        Request::post(format!(
            "/{}/api/v1/workspaces/{WS}/{path}",
            if path == "sts" { "storage" } else { "media" }
        ))
        .header("content-type", "application/json")
        .header("x-auth-token", TOKEN)
        .body(Body::from(body.to_string()))
        .unwrap()
    }

    fn callback(name: &str, lat: Value) -> Value {
        json!({
            "result": 0,
            "name": name,
            "object_key": format!("fotos/RC1/{name}"),
            "fingerprint": format!("fp-{name}"),
            "ext": {"sn": "AC1", "tinny_fingerprint": format!("tiny-{name}"), "payload_model_key": "1-61-0"},
            "metadata": {
                "absolute_altitude": 142.3, "relative_altitude": 61.5, "gimbal_yaw_degree": 87.4,
                "created_time": "2026-10-17 14:30:12",
                "shoot_position": {"lat": lat, "lng": 10.5612}
            }
        })
    }

    fn put_object(seen: &fake::Shared, name: &str) {
        seen.lock().unwrap().objects.insert(
            format!("/{}/fotos/RC1/{name}", fake::BUCKET),
            b"jpg".to_vec(),
        );
    }

    #[tokio::test]
    async fn every_endpoint_needs_token_workspace_and_storage() {
        let (app, _) = paired(false).await;
        let (status, body) = send(&app.router, post("sts", json!({}))).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
        assert_eq!(body["code"], 503);
        assert!(body["message"].as_str().unwrap().contains("Fotospeicher"));

        let (app, _) = paired(true).await;
        let no_token = Request::post(format!("/storage/api/v1/workspaces/{WS}/sts"))
            .body(Body::empty())
            .unwrap();
        let (status, body) = send(&app.router, no_token).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], 401);
        let other =
            Request::post("/storage/api/v1/workspaces/00000000-0000-4000-8000-000000000000/sts")
                .header("x-auth-token", TOKEN)
                .body(Body::empty())
                .unwrap();
        assert_eq!(send(&app.router, other).await.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn sts_hands_out_one_write_only_key_per_controller() {
        let (app, seen) = paired(true).await;
        let seen = seen.unwrap();
        let (status, body) = send(&app.router, post("sts", json!({}))).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["code"], 0);
        let data = &body["data"];
        assert_eq!(data["provider"], "minio");
        assert_eq!(data["endpoint"], "http://100.64.0.10:3900");
        assert_eq!(data["bucket"], fake::BUCKET);
        assert_eq!(data["region"], "garage");
        assert_eq!(data["object_key_prefix"], "fotos/RC1");
        assert_eq!(data["credentials"]["access_key_id"], "GK1");
        assert_eq!(data["credentials"]["security_token"], "");
        let expire = data["credentials"]["expire"].as_i64().unwrap();
        assert!((3590..=3600).contains(&expire), "{expire}");
        let (_, again) = send(&app.router, post("sts", json!({}))).await;
        assert_eq!(again["data"]["credentials"]["access_key_id"], "GK1");
        assert_eq!(seen.lock().unwrap().keys.len(), 1);
    }

    #[tokio::test]
    async fn upload_callback_stores_the_photo_and_streams_its_shot() {
        let (app, seen) = paired(true).await;
        let seen = seen.unwrap();
        let mut shots = app.state.photos.subscribe();
        for name in [
            "DJI_20261017143012_0001_T.JPG",
            "DJI_20261017143012_0001_W.JPG",
        ] {
            put_object(&seen, name);
            let (status, body) = send(
                &app.router,
                post("upload-callback", callback(name, json!(52.9612))),
            )
            .await;
            assert_eq!(status, StatusCode::OK, "{body}");
            assert_eq!(body["data"]["object_key"], format!("fotos/RC1/{name}"));
        }
        let first = shots.recv().await.unwrap();
        assert_eq!(first.photos.len(), 1);
        let shot = shots.recv().await.unwrap();
        assert_eq!(shot.id, "RC1/DJI_20261017143012_0001");
        assert_eq!(
            shot.photos.iter().map(|p| p.lens).collect::<Vec<_>>(),
            [PhotoLens::Thermal, PhotoLens::Wide]
        );
        let pos = shot.position.unwrap();
        assert_eq!((pos.lat, pos.lon), (52.9612, 10.5612));
        assert_eq!(shot.aircraft_sn.as_deref(), Some("AC1"));
        assert_eq!(shot.relative_altitude_m, Some(61.5));
        // 14:30:12 MESZ
        assert_eq!(shot.taken_at_ms, 1_792_240_212_000);

        let (_, body) = send(
            &app.router,
            post(
                "files/tiny-fingerprints",
                json!({"tiny_fingerprints": ["x", "tiny-DJI_20261017143012_0001_T.JPG"]}),
            ),
        )
        .await;
        assert_eq!(
            body["data"]["tiny_fingerprints"],
            json!(["tiny-DJI_20261017143012_0001_T.JPG"])
        );
        let (status, body) = send(
            &app.router,
            post(
                "fast-upload",
                json!({"fingerprint": "fp-DJI_20261017143012_0001_W.JPG"}),
            ),
        )
        .await;
        assert_eq!((status, body["code"].as_i64()), (StatusCode::OK, Some(0)));
        let (status, body) = send(
            &app.router,
            post("fast-upload", json!({"fingerprint": "unbekannt"})),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "Pilot 2 lädt dann normal hoch");
        assert_eq!(body["code"], -1);
    }

    #[tokio::test]
    async fn a_photo_without_position_is_kept_without_a_place() {
        let (app, seen) = paired(true).await;
        let seen = seen.unwrap();
        put_object(&seen, "DJI_2_T.JPG");
        let (status, _) = send(
            &app.router,
            post("upload-callback", callback("DJI_2_T.JPG", json!(null))),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let shot = app.state.photos.shot("RC1/DJI_2").await.unwrap().unwrap();
        assert_eq!(shot.position, None);
    }

    #[tokio::test]
    async fn callbacks_for_foreign_or_missing_objects_are_refused() {
        let (app, seen) = paired(true).await;
        let seen = seen.unwrap();
        let mut foreign = callback("DJI_1_T.JPG", json!(52.9));
        foreign["object_key"] = json!("fotos/RC2/DJI_1_T.JPG");
        seen.lock()
            .unwrap()
            .objects
            .insert(format!("/{}/fotos/RC2/DJI_1_T.JPG", fake::BUCKET), vec![1]);
        let (status, body) = send(&app.router, post("upload-callback", foreign)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert!(body["message"].as_str().unwrap().contains("fotos/RC1/"));
        let (status, body) = send(
            &app.router,
            post("upload-callback", callback("fehlt_T.JPG", json!(52.9))),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert!(
            body["message"]
                .as_str()
                .unwrap()
                .contains("nicht im Fotospeicher")
        );
        assert!(app.state.photos.recent(0).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_failed_upload_is_logged_not_stored() {
        let (app, _) = paired(true).await;
        let mut failed = callback("DJI_3_T.JPG", json!(52.9));
        failed["result"] = json!(1);
        let (status, body) = send(&app.router, post("upload-callback", failed)).await;
        assert_eq!((status, body["code"].as_i64()), (StatusCode::OK, Some(0)));
        assert!(app.state.photos.recent(0).await.unwrap().is_empty());
        let (status, body) = send(
            &app.router,
            post(
                "group-upload-callback",
                json!({"file_group_id": "g", "file_count": 3, "file_uploaded_count": 3}),
            ),
        )
        .await;
        assert_eq!((status, body["code"].as_i64()), (StatusCode::OK, Some(0)));
    }
}

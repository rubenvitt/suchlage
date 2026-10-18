//! Einsätze für den Browser. Lesen darf jede Sitzung, auch das Großbild; ändern nur Operator
//! und Admin. Änderungen erreichen die Browser über `/api/live` (`missions_changed`).

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post, put};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::{Admin, Operator, Viewer};
use crate::contract::{
    Mission, MissionAssign, MissionContents, MissionCreate, MissionUpdate, SectionCreate,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/missions", get(list).post(create))
        .route("/missions/{id}", put(update).delete(delete))
        .route("/missions/{id}/contents", get(contents))
        .route("/missions/{id}/start", post(start))
        .route("/missions/{id}/close", post(close))
        .route("/missions/{id}/sections", post(add_section))
        .route(
            "/missions/{id}/sections/{section_id}/map-photo",
            get(map_photo),
        )
        .route("/flights/{id}/mission", put(assign_flight))
}

/// `?mission=` der Listen: fehlt → `None` (alles), `none` → `Some(None)` (ohne Einsatz),
/// Zahl → `Some(Some(n))`, sonst 400.
pub(crate) fn parse_scope(raw: Option<&str>) -> Result<Option<Option<i64>>, AppError> {
    match raw {
        None => Ok(None),
        Some("none") => Ok(Some(None)),
        Some(n) => n
            .parse()
            .map(|n| Some(Some(n)))
            .map_err(|_| AppError::Validation("Einsatz ungültig".into())),
    }
}

async fn list(_: Viewer, State(state): State<AppState>) -> Result<Json<Vec<Mission>>, AppError> {
    Ok(Json(state.missions.list().await?))
}

async fn create(
    Operator(user): Operator,
    State(state): State<AppState>,
    JsonBody(input): JsonBody<MissionCreate>,
) -> Result<Json<Mission>, AppError> {
    Ok(Json(
        state
            .missions
            .create(input, &user.display_name, now_ms())
            .await?,
    ))
}

async fn update(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<MissionUpdate>,
) -> Result<Json<Mission>, AppError> {
    Ok(Json(state.missions.update(id, input).await?))
}

async fn contents(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<MissionContents>, AppError> {
    Ok(Json(state.deleter.contents(id).await?))
}

/// Löscht den Einsatz mit allem, was dazugehört (Flüge, Videos, Fotos, Kartenelemente).
async fn delete(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    let d = state.deleter.clone();
    crate::deletion::detached(async move { d.delete_mission(id).await }).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn start(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Mission>, AppError> {
    Ok(Json(state.missions.start(id, now_ms()).await?))
}

async fn close(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Mission>, AppError> {
    Ok(Json(state.missions.close(id, now_ms()).await?))
}

async fn add_section(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<SectionCreate>,
) -> Result<Json<Mission>, AppError> {
    Ok(Json(state.missions.add_section(id, input, now_ms()).await?))
}

/// Foto der Papierkarte eines Abschnitts (aus der Flugmappe). Die Flugmappe kann es ersetzen;
/// `map_photo_at_ms` am Abschnitt taugt als Versionszeichen für die URL.
async fn map_photo(
    _: Viewer,
    State(state): State<AppState>,
    Path((id, section_id)): Path<(i64, i64)>,
) -> Result<Response, AppError> {
    let mission = state.missions.get(id).await?;
    // Der Dateipfad entsteht aus der ID der gespeicherten Abschnittszeile, nicht aus dem URL-Pfad.
    let section = mission
        .sections
        .iter()
        .find(|s| s.id == section_id && s.has_map_photo)
        .ok_or(AppError::NotFound)?;
    let bytes = match tokio::fs::read(state.missions.map_photo_path(section.id)).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Err(AppError::NotFound),
        Err(e) => {
            return Err(AppError::Internal(format!(
                "Kartenfoto {}: {e}",
                section.id
            )));
        }
    };
    Response::builder()
        .header(header::CONTENT_TYPE, "image/jpeg")
        .header(header::CACHE_CONTROL, "private, no-cache")
        .body(Body::from(bytes))
        .map_err(|e| AppError::Internal(e.to_string()))
}

async fn assign_flight(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<MissionAssign>,
) -> Result<StatusCode, AppError> {
    state.missions.assign_flight(id, input).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::StatusCode;
    use serde_json::json;

    use crate::contract::{FlightSummary, Role};
    use crate::testing::{TestApp, get, post_json, put_json, send, test_app, with_cookie};

    async fn post(
        app: &TestApp,
        uri: &str,
        body: serde_json::Value,
    ) -> (StatusCode, serde_json::Value) {
        let cookie = app.cookie(Role::Operator).await;
        send(&app.router, with_cookie(post_json(uri, body), &cookie)).await
    }

    async fn prepared(app: &TestApp, name: &str) -> i64 {
        let (status, m) = post(
            app,
            "/api/missions",
            json!({"name": name, "location": null}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        m["id"].as_i64().unwrap()
    }

    fn flight(id: i64, ended: Option<i64>) -> FlightSummary {
        FlightSummary {
            id,
            aircraft_sn: format!("AC{id}"),
            gateway_sn: "RC1".into(),
            started_at_ms: id * 1_000,
            ended_at_ms: ended,
            end_reason: ended.map(|_| crate::contract::FlightEndReason::Landed),
            max_elevation_m: None,
            distance_m: 0.0,
        }
    }

    #[tokio::test]
    async fn operator_prepares_starts_and_closes() {
        let app = test_app().await;
        let (status, m) = post(
            &app,
            "/api/missions",
            json!({"name": " Wald ", "location": "Nord"}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            (m["state"].as_str(), m["name"].as_str()),
            (Some("prepared"), Some("Wald"))
        );
        let id = m["id"].as_i64().unwrap();
        let (_, m) = post(&app, &format!("/api/missions/{id}/start"), json!({})).await;
        assert_eq!(m["state"], "running");
        let (status, m) = post(&app, &format!("/api/missions/{id}/close"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(m["state"], "closed");
        assert!(m["closed_at_ms"].is_i64());
    }

    #[tokio::test]
    async fn starting_a_second_mission_is_409() {
        let app = test_app().await;
        let (a, b) = (prepared(&app, "A").await, prepared(&app, "B").await);
        post(&app, &format!("/api/missions/{a}/start"), json!({})).await;
        let (status, _) = post(&app, &format!("/api/missions/{b}/start"), json!({})).await;
        assert_eq!(status, StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn closing_with_an_open_flight_is_422() {
        let app = test_app().await;
        let id = prepared(&app, "A").await;
        post(&app, &format!("/api/missions/{id}/start"), json!({})).await;
        crate::store::flights::insert(&app.state.pool, &flight(1, None))
            .await
            .unwrap();
        let (status, _) = post(&app, &format!("/api/missions/{id}/close"), json!({})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        // Ein Neustart des Abschlusses nach der Landung geht.
        crate::store::flights::update(&app.state.pool, &flight(1, Some(5_000)))
            .await
            .unwrap();
        let (status, _) = post(&app, &format!("/api/missions/{id}/close"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn closed_mission_takes_no_section_422() {
        let app = test_app().await;
        let id = prepared(&app, "A").await;
        let uri = format!("/api/missions/{id}/sections");
        let (status, m) = post(&app, &uri, json!({"label": "Nord", "location": null})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(m["sections"].as_array().unwrap().len(), 2);
        post(&app, &format!("/api/missions/{id}/close"), json!({})).await;
        let (status, _) = post(&app, &uri, json!({"label": "Süd", "location": null})).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = post(
            &app,
            "/api/missions/999/sections",
            json!({"label": "x", "location": null}),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn kiosk_reads_but_cannot_change() {
        let app = test_app().await;
        let kiosk = app.cookie(Role::Kiosk).await;
        let (status, list) = send(&app.router, with_cookie(get("/api/missions"), &kiosk)).await;
        assert_eq!((status, list), (StatusCode::OK, json!([])));
        let req = post_json("/api/missions", json!({"name": "A", "location": null}));
        let (status, _) = send(&app.router, with_cookie(req, &kiosk)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = send(&app.router, get("/api/missions")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn empty_name_is_400() {
        let app = test_app().await;
        let (status, _) = post(
            &app,
            "/api/missions",
            json!({"name": "  ", "location": null}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let id = prepared(&app, "A").await;
        let (status, _) = post(
            &app,
            &format!("/api/missions/{id}/sections"),
            json!({"label": "", "location": null}),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn assigning_a_flight_that_already_has_a_mission_is_422() {
        let app = test_app().await;
        let id = prepared(&app, "A").await;
        crate::store::flights::insert(&app.state.pool, &flight(1, Some(2_000)))
            .await
            .unwrap();
        let cookie = app.cookie(Role::Operator).await;
        let put = |uri: &str, mission: i64| {
            with_cookie(put_json(uri, json!({"mission_id": mission})), &cookie)
        };
        let (status, _) = send(&app.router, put("/api/flights/1/mission", id)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = send(&app.router, put("/api/flights/1/mission", id)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = send(&app.router, put("/api/flights/99/mission", id)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&app.router, put("/api/flights/1/mission", 999)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

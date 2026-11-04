//! Schnittstelle für das Einsatztagebuch der IUK-Suite (`/api/einsatztagebuch/v1`). Angemeldet
//! wird wie beim Lagemonitor mit einem Großbild-Token als `Authorization: Bearer …`: Der Server der
//! Suite fragt selbst und reicht die Liste an die Rechner des Einsatztagebuchs weiter. Nur lesen,
//! kein CORS.

use axum::extract::{FromRef, FromRequestParts, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::app::AppState;
use crate::contract::{
    EINSATZTAGEBUCH_API_VERSION, EINSATZTAGEBUCH_DEFAULT_DAYS, EINSATZTAGEBUCH_MAX_DAYS,
    EtbMission, EtbMissions, Mission, MissionState,
};
use crate::error::AppError;
use crate::store;
use crate::time::now_ms;

const DAY_MS: i64 = 24 * 60 * 60 * 1000;

pub fn routes() -> Router<AppState> {
    Router::new().route("/einsaetze", get(einsaetze))
}

/// Ein gültiger Großbild-Token. Widerrufen wirkt sofort, denn jede Anfrage prüft neu.
pub struct EinsatztagebuchClient;

impl<S: Send + Sync> FromRequestParts<S> for EinsatztagebuchClient
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
            .map(|_| EinsatztagebuchClient)
            .ok_or(AppError::Unauthorized)
    }
}

#[derive(Deserialize)]
pub struct EinsaetzeQuery {
    /// Wie viele Tage zurück abgeschlossene Einsätze zählen (1 bis 31).
    tage: Option<u32>,
}

/// Laufende und seit `since_ms` abgeschlossene Einsätze; laufende zuerst, dann zuletzt beendete
/// zuerst. Vorbereitete fehlen: Sie liefen nie, im Einsatztagebuch gibt es nichts zu verknüpfen.
pub fn selectable(missions: Vec<Mission>, since_ms: i64) -> Vec<EtbMission> {
    let mut out: Vec<EtbMission> = missions
        .into_iter()
        .filter(|m| match m.state {
            MissionState::Running => true,
            MissionState::Closed => m.closed_at_ms.is_some_and(|t| t >= since_ms),
            MissionState::Prepared => false,
        })
        .map(|m| EtbMission {
            id: m.id,
            name: m.name,
            location: m.location,
            state: m.state,
            started_at_ms: m.started_at_ms,
            closed_at_ms: m.closed_at_ms,
        })
        .collect();
    out.sort_by_key(|m| {
        (
            m.state != MissionState::Running,
            std::cmp::Reverse(m.closed_at_ms),
        )
    });
    out
}

async fn einsaetze(
    _: EinsatztagebuchClient,
    State(state): State<AppState>,
    Query(query): Query<EinsaetzeQuery>,
) -> Result<Json<EtbMissions>, AppError> {
    let days = query.tage.unwrap_or(EINSATZTAGEBUCH_DEFAULT_DAYS);
    if !(1..=EINSATZTAGEBUCH_MAX_DAYS).contains(&days) {
        return Err(AppError::Validation("tage ungültig".into()));
    }
    let now = now_ms();
    let missions = selectable(state.missions.list().await?, now - i64::from(days) * DAY_MS);
    Ok(Json(EtbMissions {
        api_version: EINSATZTAGEBUCH_API_VERSION,
        server_time_ms: now,
        missions,
    }))
}

#[cfg(test)]
mod tests {
    use axum::http::{Request, StatusCode};
    use serde_json::Value;

    use super::*;
    use crate::contract::MissionCreate;
    use crate::testing::{get, send, test_app};

    fn bearer(token: &str, path: &str) -> Request<axum::body::Body> {
        let mut req = get(path);
        req.headers_mut()
            .insert(AUTHORIZATION, format!("Bearer {token}").parse().unwrap());
        req
    }

    const PATH: &str = "/api/einsatztagebuch/v1/einsaetze";

    #[tokio::test]
    async fn without_or_with_an_unknown_token_is_401() {
        let app = test_app().await;
        assert_eq!(
            send(&app.router, get(PATH)).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            send(&app.router, bearer("falsch", PATH)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn a_revoked_token_stops_at_once() {
        let app = test_app().await;
        let (view, secret) = store::kiosk::create(&app.state.pool, "Suite", 0)
            .await
            .unwrap();
        assert_eq!(
            send(&app.router, bearer(&secret, PATH)).await.0,
            StatusCode::OK
        );
        store::kiosk::delete(&app.state.pool, view.id)
            .await
            .unwrap();
        assert_eq!(
            send(&app.router, bearer(&secret, PATH)).await.0,
            StatusCode::UNAUTHORIZED
        );
    }

    #[tokio::test]
    async fn lists_running_and_recently_closed_but_not_prepared() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "Suite", 0)
            .await
            .unwrap();
        let create = |name: &str| MissionCreate {
            name: name.into(),
            location: Some("Uelzen".into()),
        };
        let now = now_ms();
        let alt = app
            .state
            .missions
            .create(create("Alt"), "T", 1)
            .await
            .unwrap();
        app.state.missions.start(alt.id, 2).await.unwrap();
        app.state.missions.close(alt.id, 3).await.unwrap();
        let gestern = app
            .state
            .missions
            .create(create("Gestern"), "T", 1)
            .await
            .unwrap();
        app.state
            .missions
            .start(gestern.id, now - DAY_MS - 1000)
            .await
            .unwrap();
        app.state
            .missions
            .close(gestern.id, now - DAY_MS - 1)
            .await
            .unwrap();
        app.state
            .missions
            .create(create("Vorbereitet"), "T", 1)
            .await
            .unwrap();
        let laeuft = app
            .state
            .missions
            .create(create("Läuft"), "T", 1)
            .await
            .unwrap();
        app.state
            .missions
            .start(laeuft.id, now - 1000)
            .await
            .unwrap();

        let (status, json) = send(&app.router, bearer(&secret, PATH)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(json["api_version"], 1);
        let namen: Vec<&str> = json["missions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["name"].as_str().unwrap())
            .collect();
        assert_eq!(namen, ["Läuft", "Gestern"]);
        assert_eq!(json["missions"][0]["state"], "running");
        assert_eq!(json["missions"][0]["closed_at_ms"], Value::Null);
        assert_eq!(json["missions"][1]["location"], "Uelzen");
        assert_eq!(json["missions"][1]["closed_at_ms"], now - DAY_MS - 1);

        let (_, json) = send(&app.router, bearer(&secret, &format!("{PATH}?tage=1"))).await;
        assert_eq!(
            json["missions"].as_array().unwrap().len(),
            1,
            "gestern liegt vor einem Tag"
        );
    }

    #[tokio::test]
    async fn tage_outside_1_to_31_is_400() {
        let app = test_app().await;
        let (_, secret) = store::kiosk::create(&app.state.pool, "Suite", 0)
            .await
            .unwrap();
        for tage in ["0", "32", "x"] {
            let (status, _) =
                send(&app.router, bearer(&secret, &format!("{PATH}?tage={tage}"))).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "tage={tage}");
        }
    }
}

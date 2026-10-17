//! Flugliste und Flugdetail. Nur für Konten: das Großbild zeigt die Lage, nicht das Archiv.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Operator;
use crate::contract::{FlightDetail, FlightSummary};
use crate::error::AppError;
use crate::store;

const LIST_LIMIT: i64 = 200;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/flights", get(list))
        .route("/flights/{id}", get(get_one))
}

async fn list(
    _: Operator,
    State(state): State<AppState>,
) -> Result<Json<Vec<FlightSummary>>, AppError> {
    Ok(Json(store::flights::list(&state.pool, LIST_LIMIT).await?))
}

async fn get_one(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<FlightDetail>, AppError> {
    Ok(Json(detail(&state, id).await?))
}

/// Flug samt Spur. B15 ergänzt das Video.
pub async fn detail(state: &AppState, id: i64) -> Result<FlightDetail, AppError> {
    let flight = store::flights::get(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;
    let segments = store::flights::segments(&state.pool, id).await?;
    Ok(FlightDetail {
        flight,
        segments,
        video: None,
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
    async fn flights_are_for_accounts_only() {
        let app = test_app().await;
        let kiosk = app.cookie(Role::Kiosk).await;
        for uri in ["/api/flights", "/api/flights/1"] {
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

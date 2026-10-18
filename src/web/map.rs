//! Kartenelemente für den Browser (M2). Lesen darf jede Sitzung, auch das Großbild; ändern nur
//! Operator und Admin. Änderungen erreichen Browser über `/api/live` und Pilots über `/pilot/ws`.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::{Operator, Viewer};
use crate::contract::{
    MapElement, MapElementInput, MapElementOrigin, MapElementUpdate, MapPoint, MissionAssign,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::map::{ElementPatch, NewElement, rules};
use crate::time::now_ms;
use crate::web::missions::parse_scope;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/map/elements", get(list).post(create))
        .route("/map/elements/{id}", put(update).delete(remove))
        .route("/map/elements/{id}/mission", put(assign))
}

#[derive(serde::Deserialize)]
struct ScopeQuery {
    mission: Option<String>,
}

async fn list(
    _: Viewer,
    State(state): State<AppState>,
    Query(q): Query<ScopeQuery>,
) -> Result<Json<Vec<MapElement>>, AppError> {
    Ok(Json(match parse_scope(q.mission.as_deref())? {
        Some(mission) => state.map.list_in(mission).await?,
        None => state.map.list().await?,
    }))
}

async fn assign(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<MissionAssign>,
) -> Result<Json<MapElement>, AppError> {
    Ok(Json(state.map.assign(&id, input.mission_id).await?))
}

async fn create(
    Operator(user): Operator,
    State(state): State<AppState>,
    JsonBody(input): JsonBody<MapElementInput>,
) -> Result<Json<MapElement>, AppError> {
    let color = rules::palette_color(&input.color).map_err(AppError::Validation)?;
    let points = input
        .points
        .iter()
        .map(|p| MapPoint {
            lat: p.lat,
            lon: p.lon,
            alt_m: None,
        })
        .collect();
    let new = NewElement {
        id: None,
        name: input.name,
        kind: input.kind,
        points,
        color,
        // Ohne Höhe: Pilot 2 legt das Element auf das Gelände.
        clamp_to_ground: true,
        author: user.display_name,
        origin: MapElementOrigin::Web,
        mission_id: input.mission_id,
    };
    Ok(Json(state.map.create(new, now_ms()).await?))
}

async fn update(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<String>,
    JsonBody(input): JsonBody<MapElementUpdate>,
) -> Result<Json<MapElement>, AppError> {
    let current = state.map.get(&id).await?.ok_or(AppError::NotFound)?;
    let color = match rules::palette_color(&input.color) {
        Ok(color) => color,
        // Eine eigene Farbe aus Pilot 2 bleibt, solange im Browser niemand eine andere wählt.
        Err(_) if rules::normalize_color(&input.color).as_ref() == Some(&current.color) => {
            current.color
        }
        Err(e) => return Err(AppError::Validation(e)),
    };
    let patch = ElementPatch {
        name: Some(input.name),
        color: Some(color),
        ..ElementPatch::default()
    };
    Ok(Json(state.map.update(&id, patch, now_ms()).await?))
}

async fn remove(
    _: Operator,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    state.map.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::json;

    use crate::contract::Role;
    use crate::testing::{TestApp, get, post_json, put_json, send, test_app, with_cookie};

    fn area() -> serde_json::Value {
        json!({
            "name": "Suchgebiet Nord",
            "kind": "polygon",
            "points": [
                {"lat": 52.433, "lon": 10.008},
                {"lat": 52.433, "lon": 10.016},
                {"lat": 52.4305, "lon": 10.016}
            ],
            "color": "#ffbb00"
        })
    }

    fn delete(uri: &str) -> Request<Body> {
        Request::delete(uri).body(Body::empty()).unwrap()
    }

    async fn operator(app: &TestApp) -> String {
        app.cookie(Role::Operator).await
    }

    #[tokio::test]
    async fn operator_draws_renames_and_deletes_an_area() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let (status, created) = send(
            &app.router,
            with_cookie(post_json("/api/map/elements", area()), &cookie),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{created}");
        assert_eq!(created["color"], "#FFBB00");
        assert_eq!(created["author"], "Operator");
        assert_eq!(created["origin"], "web");
        assert_eq!(created["clamp_to_ground"], true);
        let id = created["id"].as_str().unwrap().to_string();
        let uri = format!("/api/map/elements/{id}");

        let (status, renamed) = send(
            &app.router,
            with_cookie(
                put_json(&uri, json!({"name": "Abschnitt 1", "color": "#19BE6B"})),
                &cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{renamed}");
        assert_eq!(
            (
                renamed["name"].as_str(),
                renamed["points"].as_array().map(Vec::len)
            ),
            (Some("Abschnitt 1"), Some(3))
        );

        let (status, list) =
            send(&app.router, with_cookie(get("/api/map/elements"), &cookie)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(list.as_array().unwrap().len(), 1);

        let (status, _) = send(&app.router, with_cookie(delete(&uri), &cookie)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let (status, _) = send(&app.router, with_cookie(delete(&uri), &cookie)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn invalid_drawings_are_field_errors() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let mut wrong_color = area();
        wrong_color["color"] = json!("#0091FF");
        let mut too_few = area();
        too_few["points"] = json!([{"lat": 52.4, "lon": 10.0}, {"lat": 52.5, "lon": 10.0}]);
        let mut no_name = area();
        no_name["name"] = json!("  ");
        for body in [wrong_color, too_few, no_name] {
            let (status, res) = send(
                &app.router,
                with_cookie(post_json("/api/map/elements", body), &cookie),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{res}");
        }
        let (status, _) = send(
            &app.router,
            with_cookie(
                put_json(
                    "/api/map/elements/fehlt",
                    json!({"name": "x", "color": "#19BE6B"}),
                ),
                &cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    /// Pilot 2 schickt womöglich Farben außerhalb der Palette (Beispiel der DJI-Doku: `#0091FF`).
    #[tokio::test]
    async fn renaming_keeps_a_foreign_pilot_color_but_does_not_accept_new_ones() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let pin = app
            .state
            .map
            .create(
                crate::map::NewElement {
                    id: Some("pilot-pin".into()),
                    name: "Pilot 1".into(),
                    kind: crate::contract::MapElementKind::Point,
                    points: vec![crate::contract::MapPoint {
                        lat: 52.4,
                        lon: 10.0,
                        alt_m: None,
                    }],
                    color: "#0091FF".into(),
                    clamp_to_ground: false,
                    author: "RC Plus 1".into(),
                    origin: crate::contract::MapElementOrigin::Pilot,
                    mission_id: None,
                },
                1,
            )
            .await
            .unwrap();
        let uri = format!("/api/map/elements/{}", pin.id);
        let rename = |color: &str| {
            with_cookie(
                put_json(&uri, json!({"name": "Fundstelle", "color": color})),
                &cookie,
            )
        };
        let (status, body) = send(&app.router, rename("#0091ff")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            (body["name"].as_str(), body["color"].as_str()),
            (Some("Fundstelle"), Some("#0091FF"))
        );
        let (status, _) = send(&app.router, rename("#123456")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    async fn mission(app: &TestApp, cookie: &str, name: &str) -> i64 {
        let (status, m) = send(
            &app.router,
            with_cookie(
                post_json("/api/missions", json!({"name": name, "location": null})),
                cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{m}");
        m["id"].as_i64().unwrap()
    }

    async fn draw(
        app: &TestApp,
        cookie: &str,
        mission_id: Option<i64>,
    ) -> (StatusCode, serde_json::Value) {
        let mut body = area();
        body["mission_id"] = json!(mission_id);
        send(
            &app.router,
            with_cookie(post_json("/api/map/elements", body), cookie),
        )
        .await
    }

    #[tokio::test]
    async fn browser_draws_into_the_selected_mission() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let a = mission(&app, &cookie, "A").await;
        let (status, created) = draw(&app, &cookie, Some(a)).await;
        assert_eq!(status, StatusCode::OK, "{created}");
        assert_eq!(created["mission_id"], a);
        draw(&app, &cookie, None).await;
        let count = |uri: String| {
            let router = app.router.clone();
            let cookie = cookie.clone();
            async move {
                let (_, list) = send(&router, with_cookie(get(&uri), &cookie)).await;
                list.as_array().unwrap().len()
            }
        };
        assert_eq!(count(format!("/api/map/elements?mission={a}")).await, 1);
        assert_eq!(count("/api/map/elements?mission=none".into()).await, 1);
        assert_eq!(count("/api/map/elements".into()).await, 2);
        let (status, _) = draw(&app, &cookie, Some(999)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn elements_of_a_closed_mission_are_read_only() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let a = mission(&app, &cookie, "A").await;
        let (_, created) = draw(&app, &cookie, Some(a)).await;
        let uri = format!("/api/map/elements/{}", created["id"].as_str().unwrap());
        let (status, _) = send(
            &app.router,
            with_cookie(
                post_json(&format!("/api/missions/{a}/close"), json!({})),
                &cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let (status, _) = send(
            &app.router,
            with_cookie(
                put_json(&uri, json!({"name": "x", "color": "#19BE6B"})),
                &cookie,
            ),
        )
        .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = send(&app.router, with_cookie(delete(&uri), &cookie)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = draw(&app, &cookie, Some(a)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[tokio::test]
    async fn unassigned_element_can_be_assigned_once() {
        let app = test_app().await;
        let cookie = operator(&app).await;
        let a = mission(&app, &cookie, "A").await;
        let (_, created) = draw(&app, &cookie, None).await;
        let uri = format!(
            "/api/map/elements/{}/mission",
            created["id"].as_str().unwrap()
        );
        let assign =
            |uri: &str, id: i64| with_cookie(put_json(uri, json!({"mission_id": id})), &cookie);
        let (status, body) = send(&app.router, assign(&uri, a)).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["mission_id"], a);
        let (status, _) = send(&app.router, assign(&uri, a)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, _) = send(&app.router, assign("/api/map/elements/fehlt/mission", a)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn kiosk_reads_but_does_not_draw() {
        let app = test_app().await;
        let kiosk = app.cookie(Role::Kiosk).await;
        let (status, list) = send(&app.router, with_cookie(get("/api/map/elements"), &kiosk)).await;
        assert_eq!((status, list), (StatusCode::OK, json!([])));
        let (status, _) = send(
            &app.router,
            with_cookie(post_json("/api/map/elements", area()), &kiosk),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, _) = send(&app.router, get("/api/map/elements")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
    }
}

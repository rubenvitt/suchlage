//! Kartenelemente für Pilot 2 (M2): die HTTPS-Endpunkte der Cloud API unter `/map/api/v1/…` und
//! der WebSocket für Pushes. Zugang über den API-Token aus `/pilot/api/connect`; Antworten und
//! Fehler im DJI-Format `{code, message, data}`, weil Pilot 2 nur das liest.

use axum::body::Bytes;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, RawQuery, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use tokio::sync::broadcast::error::RecvError;

use crate::app::AppState;
use crate::contract::MapElementOrigin;
use crate::dji::map::{self as dji, ElementCreateInput, ElementUpdateInput, SHARED_GROUP_ID};
use crate::error::AppError;
use crate::map::{ElementPatch, MapChange, MapHub, NewElement, rules};
use crate::store::remote_controllers::{self as rcs, RemoteController};
use crate::time::now_ms;

/// Name des Tokens im Header (`40.https.md`) und in der Query des WebSockets (`30.jsbridge.md`).
const TOKEN: &str = "x-auth-token";
/// So oft prüft ein offener WebSocket, ob sein Token noch gilt (Fernsteuerung gelöscht oder neu gekoppelt).
const TOKEN_RECHECK: std::time::Duration = if cfg!(test) {
    std::time::Duration::from_millis(200)
} else {
    std::time::Duration::from_secs(30)
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/map/api/v1/workspaces/{workspace_id}/element-groups",
            get(groups),
        )
        .route(
            "/map/api/v1/workspaces/{workspace_id}/element-groups/{group_id}/elements",
            post(create),
        )
        .route(
            "/map/api/v1/workspaces/{workspace_id}/elements/{id}",
            put(update).delete(remove),
        )
        .route("/pilot/ws", get(websocket))
}

/// `AppError` im DJI-Format. Pilot 2 zeigt je Status einen Toast; 401 meldet die Plattform ab.
pub struct DjiError(pub AppError);

impl From<AppError> for DjiError {
    fn from(e: AppError) -> Self {
        Self(e)
    }
}

impl IntoResponse for DjiError {
    fn into_response(self) -> Response {
        let status = self.0.status();
        if let AppError::Internal(detail) = &self.0 {
            tracing::error!(%detail, "interner Fehler");
        }
        let body = dji::failure(status.as_u16(), &self.0.to_string());
        (status, Json(body)).into_response()
    }
}

pub(crate) type DjiResult = Result<Json<Value>, DjiError>;

pub(crate) fn query_param(query: Option<&str>, key: &str) -> Option<String> {
    url::form_urlencoded::parse(query?.as_bytes())
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.into_owned())
}

fn token_of(headers: &HeaderMap, query: Option<&str>) -> Option<String> {
    headers
        .get(TOKEN)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .or_else(|| query_param(query, TOKEN))
        .filter(|t| !t.is_empty())
}

/// Fernsteuerung zum Token aus Header oder Query.
pub(crate) async fn pilot(
    state: &AppState,
    headers: &HeaderMap,
    query: Option<&str>,
) -> Result<RemoteController, DjiError> {
    let token = token_of(headers, query).ok_or(AppError::Unauthorized)?;
    Ok(rcs::find_by_api_token(&state.pool, &token)
        .await
        .map_err(AppError::from)?
        .ok_or(AppError::UnauthorizedWith(
            "Anmeldung abgelaufen – in Pilot 2 die Cloud-Seite von Suchlage neu öffnen".into(),
        ))?)
}

/// Eine andere Workspace-ID als in den Einstellungen ist ein anderer Arbeitsbereich: 404.
pub(crate) fn workspace(state: &AppState, workspace_id: &str) -> Result<(), DjiError> {
    let dji = state.settings.current().dji.ok_or_else(|| {
        AppError::Unavailable("DJI-App-Daten fehlen (Einstellungen → DJI)".into())
    })?;
    if dji.workspace_id != workspace_id {
        return Err(AppError::NotFound.into());
    }
    Ok(())
}

pub(crate) fn parse<T: DeserializeOwned>(body: &Bytes) -> Result<T, DjiError> {
    serde_json::from_slice(body)
        .map_err(|e| AppError::Validation(format!("Ungültige Anfrage: {e}")).into())
}

async fn groups(
    State(state): State<AppState>,
    Path(workspace_id): Path<String>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> DjiResult {
    pilot(&state, &headers, query.as_deref()).await?;
    workspace(&state, &workspace_id)?;
    // `is_distributed` hat für die eine Gruppe keine Bedeutung; eine fremde `group_id` gibt es nicht.
    if query_param(query.as_deref(), "group_id").is_some_and(|g| g != SHARED_GROUP_ID) {
        return Ok(Json(dji::ok(json!([]))));
    }
    let elements = state.map.list().await?;
    Ok(Json(dji::ok(dji::groups(&elements))))
}

async fn create(
    State(state): State<AppState>,
    Path((workspace_id, group_id)): Path<(String, String)>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    let rc = pilot(&state, &headers, query.as_deref()).await?;
    workspace(&state, &workspace_id)?;
    if group_id != SHARED_GROUP_ID {
        return Err(AppError::NotFound.into());
    }
    let input: ElementCreateInput = parse(&body)?;
    let shape = input
        .resource
        .content
        .shape()
        .map_err(AppError::Validation)?;
    let name = input
        .name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| rules::default_name(shape.kind).to_string());
    let author = input
        .resource
        .user_name
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(rc.label);
    let new = NewElement {
        id: Some(input.id),
        name,
        kind: shape.kind,
        points: shape.points,
        color: shape
            .color
            .as_deref()
            .and_then(rules::normalize_color)
            .unwrap_or_else(|| rules::DEFAULT_COLOR.to_string()),
        clamp_to_ground: shape.clamp_to_ground.unwrap_or(false),
        author,
        origin: MapElementOrigin::Pilot,
        // Task 4 stellt auf den Einsatz um, den Pilot 2 sieht.
        mission_id: None,
    };
    let element = state.map.create(new, now_ms()).await?;
    Ok(Json(dji::ok(json!({ "id": element.id }))))
}

async fn update(
    State(state): State<AppState>,
    Path((workspace_id, id)): Path<(String, String)>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    body: Bytes,
) -> DjiResult {
    pilot(&state, &headers, query.as_deref()).await?;
    workspace(&state, &workspace_id)?;
    let input: ElementUpdateInput = parse(&body)?;
    let mut patch = ElementPatch {
        name: input.name.filter(|n| !n.trim().is_empty()),
        ..ElementPatch::default()
    };
    if let Some(content) = input.content {
        let shape = content.shape().map_err(AppError::Validation)?;
        // Eine unlesbare Farbe ändert nichts; beim Anlegen gilt dafür die Standardfarbe.
        patch.color = shape.color.as_deref().and_then(rules::normalize_color);
        patch.clamp_to_ground = shape.clamp_to_ground;
        patch.shape = Some((shape.kind, shape.points));
    }
    let element = state.map.update(&id, patch, now_ms()).await?;
    Ok(Json(dji::ok(json!({ "id": element.id }))))
}

async fn remove(
    State(state): State<AppState>,
    Path((workspace_id, id)): Path<(String, String)>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> DjiResult {
    pilot(&state, &headers, query.as_deref()).await?;
    workspace(&state, &workspace_id)?;
    state.map.delete(&id).await?;
    Ok(Json(dji::ok(json!({ "id": id }))))
}

/// Pilot 2 verbindet sich mit `?x-auth-token=…`. Der Token wird vor dem Upgrade geprüft.
async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
    upgrade: WebSocketUpgrade,
) -> Result<Response, DjiError> {
    let rc = pilot(&state, &headers, query.as_deref()).await?;
    let token = token_of(&headers, query.as_deref()).unwrap_or_default();
    let (hub, pool) = (state.map.clone(), state.pool.clone());
    Ok(upgrade.on_upgrade(move |socket| push_to_pilot(socket, hub, pool, token, rc)))
}

/// Gilt der Token noch für dieselbe Fernsteuerung? Ein Fehler der Datenbank zählt als „ja“: Er
/// soll keinen Piloten trennen, die nächste Prüfung entscheidet.
async fn still_valid(pool: &sqlx::SqlitePool, token: &str, rc_id: i64) -> bool {
    match rcs::find_by_api_token(pool, token).await {
        Ok(found) => found.is_some_and(|rc| rc.id == rc_id),
        Err(e) => {
            tracing::warn!(error = %e, "Token des Kartenabgleichs nicht prüfbar");
            true
        }
    }
}

fn push_for(change: &MapChange, at_ms: i64) -> String {
    match change {
        MapChange::Created(e) => dji::push_created(e, at_ms),
        MapChange::Updated(e) => dji::push_updated(e, at_ms),
        MapChange::Deleted { id } => dji::push_deleted(id, at_ms),
    }
}

/// Jede Änderung geht an jeden Pilot, auch an den, der sie gemacht hat (wie in der DJI-Demo).
/// Wer Änderungen verpasst, lädt die Gruppe neu. Endet mit dem Server, der Verbindung oder dem
/// Token: Eine gelöschte oder neu gekoppelte Fernsteuerung bekommt nach spätestens `TOKEN_RECHECK`
/// nichts mehr.
async fn push_to_pilot(
    mut socket: WebSocket,
    hub: MapHub,
    pool: sqlx::SqlitePool,
    token: String,
    rc: RemoteController,
) {
    let rc_label = rc.label.clone();
    let mut changes = hub.subscribe();
    let mut stop = hub.stop_signal();
    let mut recheck =
        tokio::time::interval_at(tokio::time::Instant::now() + TOKEN_RECHECK, TOKEN_RECHECK);
    tracing::info!(rc = %rc_label, "Pilot 2 am Kartenabgleich");
    // Was zwischen dem Laden der Gruppen und diesem Abonnement geändert wurde, käme sonst nie an.
    if socket
        .send(Message::Text(dji::push_group_refresh(now_ms()).into()))
        .await
        .is_err()
    {
        return;
    }
    loop {
        tokio::select! {
            change = changes.recv() => {
                let text = match change {
                    Ok(change) => push_for(&change, now_ms()),
                    Err(RecvError::Lagged(missed)) => {
                        tracing::debug!(missed, rc = %rc_label, "Kartenabgleich überholt – Gruppe neu laden");
                        dji::push_group_refresh(now_ms())
                    }
                    Err(RecvError::Closed) => break,
                };
                if socket.send(Message::Text(text.into())).await.is_err() {
                    break;
                }
            }
            incoming = socket.recv() => match incoming {
                // Pilot 2 schickt nichts, was der Server braucht; Pings beantwortet axum selbst.
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => {}
            },
            _ = recheck.tick() => {
                if !still_valid(&pool, &token, rc.id).await {
                    tracing::info!(rc = %rc_label, "Token ungültig – Kartenabgleich getrennt");
                    let _ = socket.send(Message::Close(None)).await;
                    break;
                }
            }
            // Der `Ref` aus `wait_for` ist nicht `Send` und darf nicht über das `await` unten leben.
            () = async {
                let _ = stop.wait_for(|stopped| *stopped).await;
            } => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
        }
    }
    tracing::info!(rc = %rc_label, "Pilot 2 nicht mehr am Kartenabgleich");
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use futures::StreamExt;
    use serde_json::{Value, json};
    use tokio_tungstenite::tungstenite;

    use crate::contract::{MapElementKind, MapElementOrigin, MapPoint};
    use crate::dji::map::SHARED_GROUP_ID;
    use crate::map::{MapChange, NewElement};
    use crate::testing::{TestApp, send, test_app};

    const WS: &str = "768d4a2e-1f34-477c-9fcc-872285e895f6";
    const TOKEN: &str = "pilot-token";

    /// DJI eingerichtet, RC1 gekoppelt mit `TOKEN`.
    async fn paired() -> TestApp {
        let app = test_app().await;
        app.state
            .settings
            .update_dji(crate::contract::DjiSettingsInput {
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
        app
    }

    fn request(method: &str, uri: &str, body: Option<Value>) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .header("X-Auth-Token", TOKEN)
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
            .unwrap()
    }

    fn groups_uri() -> String {
        format!("/map/api/v1/workspaces/{WS}/element-groups")
    }

    fn create_uri() -> String {
        format!("/map/api/v1/workspaces/{WS}/element-groups/{SHARED_GROUP_ID}/elements")
    }

    fn element_uri(id: &str) -> String {
        format!("/map/api/v1/workspaces/{WS}/elements/{id}")
    }

    fn pin_body(id: &str) -> Value {
        json!({
            "id": id,
            "name": "Pilot 1",
            "resource": {
                "type": 0,
                "user_name": "",
                "content": {
                    "type": "Feature",
                    "properties": {"color": "#e23c39", "clampToGround": false},
                    "geometry": {"type": "Point", "coordinates": [10.0124, 52.4318, 84.2]}
                }
            }
        })
    }

    #[tokio::test]
    async fn without_a_valid_token_pilot_gets_401_in_dji_format() {
        let app = paired().await;
        let no_token = Request::get(groups_uri()).body(Body::empty()).unwrap();
        let wrong = Request::get(groups_uri())
            .header("x-auth-token", "falsch")
            .body(Body::empty())
            .unwrap();
        for req in [no_token, wrong] {
            let (status, body) = send(&app.router, req).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(body["code"], 401);
            assert!(body["message"].is_string());
        }
        // Über die Query wie beim WebSocket geht es auch.
        let (status, _) = send(
            &app.router,
            Request::get(format!("{}?x-auth-token={TOKEN}", groups_uri()))
                .body(Body::empty())
                .unwrap(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_foreign_workspace_or_group_is_not_found() {
        let app = paired().await;
        let other = "00000000-0000-4000-8000-000000000000";
        let (status, body) = send(
            &app.router,
            request(
                "GET",
                &format!("/map/api/v1/workspaces/{other}/element-groups"),
                None,
            ),
        )
        .await;
        assert_eq!(
            (status, &body["code"]),
            (StatusCode::NOT_FOUND, &json!(404))
        );
        let (status, _) = send(
            &app.router,
            request(
                "POST",
                &format!("/map/api/v1/workspaces/{WS}/element-groups/{other}/elements"),
                Some(pin_body("a")),
            ),
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, body) = send(
            &app.router,
            request("GET", &format!("{}?group_id={other}", groups_uri()), None),
        )
        .await;
        assert_eq!((status, body["data"].clone()), (StatusCode::OK, json!([])));
    }

    #[tokio::test]
    async fn pilot_creates_updates_and_deletes_and_the_hub_publishes_it() {
        let app = paired().await;
        let mut changes = app.state.map.subscribe();
        let id = "94c51c50-f111-45e8-ac8c-4f96c93ced44";

        let (status, body) = send(
            &app.router,
            request("POST", &create_uri(), Some(pin_body(id))),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(
            body,
            json!({"code": 0, "message": "success", "data": {"id": id}})
        );
        let MapChange::Created(created) = changes.recv().await.unwrap() else {
            panic!("Anlegen erwartet");
        };
        assert_eq!(created.origin, MapElementOrigin::Pilot);
        assert_eq!(
            created.author, "RC Plus 1",
            "leerer user_name → Fernsteuerung"
        );
        assert_eq!(created.color, "#E23C39");
        assert_eq!(created.points[0].alt_m, Some(84.2));

        let (status, body) = send(&app.router, request("GET", &groups_uri(), None)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["data"][0]["type"], 2);
        assert_eq!(body["data"][0]["elements"][0]["id"], id);

        let (status, _) = send(
            &app.router,
            request("PUT", &element_uri(id), Some(json!({"name": "Fundstelle"}))),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        let MapChange::Updated(renamed) = changes.recv().await.unwrap() else {
            panic!("Ändern erwartet");
        };
        assert_eq!(
            (renamed.name.as_str(), renamed.points.clone()),
            ("Fundstelle", created.points.clone()),
            "nur der Name ändert sich"
        );

        let moved = json!({"content": {"type": "Feature", "properties": {"color": "unlesbar"},
            "geometry": {"type": "LineString", "coordinates": [[10.0, 52.0], [10.1, 52.1]]}}});
        let (status, _) = send(&app.router, request("PUT", &element_uri(id), Some(moved))).await;
        assert_eq!(status, StatusCode::OK);
        let MapChange::Updated(line) = changes.recv().await.unwrap() else {
            panic!("Ändern erwartet");
        };
        assert_eq!((line.kind, line.points.len()), (MapElementKind::Line, 2));
        assert_eq!(line.color, "#E23C39", "unlesbare Farbe ändert nichts");

        let (status, body) = send(&app.router, request("DELETE", &element_uri(id), None)).await;
        assert_eq!(
            (status, body["data"]["id"].clone()),
            (StatusCode::OK, json!(id))
        );
        assert_eq!(
            changes.recv().await.unwrap(),
            MapChange::Deleted { id: id.into() }
        );
        let (status, body) = send(&app.router, request("DELETE", &element_uri(id), None)).await;
        assert_eq!(
            (status, &body["code"]),
            (StatusCode::NOT_FOUND, &json!(404))
        );
    }

    #[tokio::test]
    async fn broken_bodies_are_400_in_dji_format() {
        let app = paired().await;
        let mut circle = pin_body("b");
        circle["resource"]["content"]["geometry"]["type"] = json!("Circle");
        let mut two_points = pin_body("c");
        two_points["resource"]["content"]["geometry"] =
            json!({"type": "Polygon", "coordinates": [[[10.0, 52.0], [10.1, 52.1]]]});
        for body in [json!({"id": "x"}), circle, two_points] {
            let (status, res) = send(&app.router, request("POST", &create_uri(), Some(body))).await;
            assert_eq!(
                (status, &res["code"]),
                (StatusCode::BAD_REQUEST, &json!(400)),
                "{res}"
            );
        }
        assert!(app.state.map.list().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn without_dji_settings_the_map_is_unavailable() {
        let app = test_app().await;
        let pool = &app.state.pool;
        let (rc, _) = crate::store::remote_controllers::create(pool, "RC", 0)
            .await
            .unwrap();
        crate::store::remote_controllers::bind(pool, rc.id, "RC1", "pw", "key", 1)
            .await
            .unwrap();
        crate::store::remote_controllers::set_api_token(pool, rc.id, TOKEN)
            .await
            .unwrap();
        let (status, _) = send(&app.router, request("GET", &groups_uri(), None)).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    /// Echter Server, echter WebSocket: Was der Browser zeichnet, kommt bei Pilot 2 als Push an.
    #[tokio::test]
    async fn browser_changes_are_pushed_to_pilot_over_the_websocket() {
        let app = paired().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app.router.clone()).into_future());

        let refused =
            tokio_tungstenite::connect_async(format!("ws://{addr}/pilot/ws?x-auth-token=falsch"))
                .await;
        match refused {
            Err(tungstenite::Error::Http(res)) => assert_eq!(res.status(), 401),
            other => panic!("401 erwartet, war {other:?}"),
        }

        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/pilot/ws?x-auth-token={TOKEN}"))
                .await
                .unwrap();
        let next_text = async |socket: &mut tokio_tungstenite::WebSocketStream<_>| loop {
            if let tungstenite::Message::Text(text) = socket.next().await.unwrap().unwrap() {
                break serde_json::from_str::<Value>(&text).unwrap();
            }
        };
        // Zuerst „Gruppe neu laden“; danach hat der Server sicher abonniert.
        assert_eq!(
            next_text(&mut socket).await["biz_code"],
            "map_group_refresh"
        );

        let area = NewElement {
            id: None,
            name: "Suchgebiet Nord".into(),
            kind: MapElementKind::Polygon,
            points: [(52.433, 10.008), (52.433, 10.016), (52.4305, 10.016)]
                .map(|(lat, lon)| MapPoint {
                    lat,
                    lon,
                    alt_m: None,
                })
                .to_vec(),
            color: "#FFBB00".into(),
            clamp_to_ground: true,
            author: "Gruppenführer".into(),
            origin: MapElementOrigin::Web,
            mission_id: None,
        };
        let created = app.state.map.create(area, 1).await.unwrap();
        let push = tokio::time::timeout(std::time::Duration::from_secs(5), next_text(&mut socket))
            .await
            .expect("Push innerhalb von 5 s");
        assert_eq!(push["biz_code"], "map_element_create");
        assert_eq!(push["data"]["id"], created.id.as_str());
        assert_eq!(
            push["data"]["resource"]["content"]["geometry"]["type"],
            "Polygon"
        );

        app.state.map.delete(&created.id).await.unwrap();
        let push = next_text(&mut socket).await;
        assert_eq!(push["biz_code"], "map_element_delete");

        // Herunterfahren schließt die Verbindung.
        app.stop_situation();
        let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(Ok(msg)) = socket.next().await {
                if msg.is_close() {
                    break;
                }
            }
        })
        .await;
        assert!(closed.is_ok(), "Verbindung endet mit dem Server");
    }

    /// Eine gelöschte Fernsteuerung verliert auch einen schon offenen Kartenabgleich.
    #[tokio::test]
    async fn deleting_the_remote_controller_closes_its_websocket() {
        let app = paired().await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(axum::serve(listener, app.router.clone()).into_future());
        let (mut socket, _) =
            tokio_tungstenite::connect_async(format!("ws://{addr}/pilot/ws?x-auth-token={TOKEN}"))
                .await
                .unwrap();
        let rc = crate::store::remote_controllers::find_by_api_token(&app.state.pool, TOKEN)
            .await
            .unwrap()
            .unwrap();
        crate::store::remote_controllers::delete(&app.state.pool, rc.id)
            .await
            .unwrap();
        let closed = tokio::time::timeout(std::time::Duration::from_secs(5), async {
            while let Some(Ok(msg)) = socket.next().await {
                if msg.is_close() {
                    break;
                }
            }
        })
        .await;
        assert!(closed.is_ok(), "Verbindung endet nach der nächsten Prüfung");
    }
}

//! Schnittstelle für die Flugmappe (`/api/flugmappe/v1`, SUL-66). Anmeldung per Zugriffstoken
//! (`auth::bearer`), keine Sitzungscookies. CORS nur hier und nur für den eingestellten Ursprung;
//! die Einstellung gilt ohne Neustart.

use axum::body::Bytes;
use axum::extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Query, Request, State};
use axum::http::header::{
    ACCESS_CONTROL_ALLOW_HEADERS, ACCESS_CONTROL_ALLOW_METHODS, ACCESS_CONTROL_ALLOW_ORIGIN,
    ACCESS_CONTROL_MAX_AGE, ACCESS_CONTROL_REQUEST_METHOD, ORIGIN, VARY,
};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;

use crate::app::AppState;
use crate::auth::bearer::FlugmappeUser;
use crate::contract::{
    FLUGMAPPE_API_VERSION, FLUGMAPPE_MISSION_HEADER, FmFeatureCollection, FmFlight, FmMapImport,
    FmMapImportResult, FmMe, FmMission, FmMissionCreate, FmMissionLink, FmSectionCreate,
    FmSectionUpdate,
};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::missions::rules;
use crate::settings::SettingsHandle;
use crate::time::now_ms;

/// Größtes Kartenfoto (ein Handyfoto der Papierkarte).
pub const MAP_PHOTO_MAX_BYTES: usize = 8 * 1024 * 1024;
/// Zeichnungen der Flugmappe können groß sein (200 Elemente mit bis zu 1000 Punkten).
pub const MAP_IMPORT_MAX_BYTES: usize = 2 * 1024 * 1024;

const ALLOW_METHODS: &str = "GET, POST, PUT, OPTIONS";
const ALLOW_HEADERS: &str = "Authorization, Content-Type, X-Flugmappe-Einsatz";
const PREFLIGHT_MAX_AGE_S: &str = "600";

pub fn routes(settings: SettingsHandle) -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/missions", get(missions).post(create))
        .route("/missions/{id}", get(mission))
        .route("/missions/{id}/link", post(link))
        .route("/missions/{id}/sections", post(add_section))
        .route("/missions/{id}/sections/{section_id}", put(update_section))
        .route("/missions/{id}/flights", get(flights))
        .route("/missions/{id}/map", get(map))
        .route(
            "/missions/{id}/map/import",
            post(import_map).layer(DefaultBodyLimit::max(MAP_IMPORT_MAX_BYTES)),
        )
        .route(
            "/missions/{id}/sections/{section_id}/photo",
            put(map_photo).layer(DefaultBodyLimit::max(MAP_PHOTO_MAX_BYTES)),
        )
        .route("/missions/{id}/close", post(close))
        .layer(middleware::from_fn_with_state(settings, cors))
}

/// Erlaubter Ursprung, wenn die Anfrage genau von ihm kommt.
fn allowed_origin(settings: &SettingsHandle, headers: &HeaderMap) -> Option<HeaderValue> {
    let allowed = settings.current().flugmappe?.allowed_origin;
    headers
        .get(ORIGIN)
        .filter(|origin| origin.as_bytes() == allowed.as_bytes())
        .cloned()
}

/// CORS für die Flugmappe, ohne Credentials (das Token steht im Header). Ein Preflight braucht
/// kein Token; von einem fremden Ursprung oder bei ausgeschalteter Schnittstelle bekommt er 403
/// ohne Erlaubnis.
async fn cors(State(settings): State<SettingsHandle>, req: Request, next: Next) -> Response {
    let origin = allowed_origin(&settings, req.headers());
    let preflight = req.method() == Method::OPTIONS
        && req.headers().contains_key(ACCESS_CONTROL_REQUEST_METHOD);
    let mut res = if preflight {
        match &origin {
            Some(_) => {
                let mut res = StatusCode::NO_CONTENT.into_response();
                let h = res.headers_mut();
                h.insert(
                    ACCESS_CONTROL_ALLOW_METHODS,
                    HeaderValue::from_static(ALLOW_METHODS),
                );
                h.insert(
                    ACCESS_CONTROL_ALLOW_HEADERS,
                    HeaderValue::from_static(ALLOW_HEADERS),
                );
                h.insert(
                    ACCESS_CONTROL_MAX_AGE,
                    HeaderValue::from_static(PREFLIGHT_MAX_AGE_S),
                );
                res
            }
            None => AppError::Forbidden.into_response(),
        }
    } else {
        next.run(req).await
    };
    let h = res.headers_mut();
    h.append(VARY, HeaderValue::from_static("Origin"));
    if let Some(origin) = origin {
        h.insert(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    }
    res
}

/// Einsatz-ID der Flugmappe aus `X-Flugmappe-Einsatz`; Pflicht bei `/missions/{id}/…`.
pub struct Binding(pub String);

impl<S: Send + Sync> FromRequestParts<S> for Binding {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, AppError> {
        let raw = parts
            .headers
            .get(FLUGMAPPE_MISSION_HEADER)
            .ok_or_else(|| {
                AppError::Validation(format!("Header {FLUGMAPPE_MISSION_HEADER} fehlt"))
            })?
            .to_str()
            .map_err(|_| {
                AppError::Validation(format!("Header {FLUGMAPPE_MISSION_HEADER} ungültig"))
            })?;
        rules::flugmappe_mission_id(raw)
            .map(Binding)
            .map_err(AppError::Validation)
    }
}

/// Body des Kartenfotos; über der Grenze der Route 413.
struct PhotoBody(Bytes);

impl<S: Send + Sync> FromRequest<S> for PhotoBody {
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, AppError> {
        match Bytes::from_request(req, state).await {
            Ok(bytes) => Ok(PhotoBody(bytes)),
            Err(r) if r.status() == StatusCode::PAYLOAD_TOO_LARGE => {
                Err(AppError::TooLarge(format!(
                    "Kartenfoto: höchstens {} MiB",
                    MAP_PHOTO_MAX_BYTES / 1024 / 1024
                )))
            }
            Err(r) => Err(AppError::Validation(format!(
                "Ungültige Anfrage: {}",
                r.body_text()
            ))),
        }
    }
}

/// Probe: Die Flugmappe zeigt ihre Suchlage-Teile nur, wenn das klappt.
async fn me(FlugmappeUser(user): FlugmappeUser) -> Json<FmMe> {
    Json(FmMe {
        name: user.display_name,
        role: user.role,
        api_version: FLUGMAPPE_API_VERSION,
    })
}

/// Vorbereitete und laufende Einsätze zur Auswahl.
async fn missions(
    _: FlugmappeUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<FmMission>>, AppError> {
    Ok(Json(state.missions.flugmappe_missions().await?))
}

async fn create(
    FlugmappeUser(user): FlugmappeUser,
    State(state): State<AppState>,
    JsonBody(input): JsonBody<FmMissionCreate>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(
        state
            .missions
            .create_from_flugmappe(input, &user.display_name, now_ms())
            .await?,
    ))
}

/// Der verknüpfte Einsatz, auch abgeschlossen (die Flugmappe zeigt das an).
async fn mission(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(state.missions.flugmappe_mission(id, &fm).await?))
}

async fn link(
    FlugmappeUser(user): FlugmappeUser,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<FmMissionLink>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(
        state
            .missions
            .link(id, input, &user.display_name, now_ms())
            .await?,
    ))
}

async fn add_section(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<FmSectionCreate>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(
        state
            .missions
            .add_flugmappe_section(id, &fm, input, now_ms())
            .await?,
    ))
}

async fn update_section(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path((id, section)): Path<(i64, String)>,
    JsonBody(input): JsonBody<FmSectionUpdate>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(
        state
            .missions
            .update_flugmappe_section(id, &fm, &section, input)
            .await?,
    ))
}

async fn flights(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<Vec<FmFlight>>, AppError> {
    Ok(Json(state.missions.flugmappe_flights(id, &fm).await?))
}

#[derive(Deserialize)]
struct MapQuery {
    /// Abschnitts-ID der Flugmappe: nur die Flugspuren dieses Abschnitts.
    section: Option<String>,
}

async fn map(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Query(q): Query<MapQuery>,
) -> Result<Json<FmFeatureCollection>, AppError> {
    Ok(Json(
        state
            .missions
            .flugmappe_map(id, &fm, q.section.as_deref())
            .await?,
    ))
}

async fn import_map(
    FlugmappeUser(user): FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
    JsonBody(input): JsonBody<FmMapImport>,
) -> Result<Json<FmMapImportResult>, AppError> {
    Ok(Json(
        state
            .missions
            .import_map(id, &fm, input, &user.display_name, now_ms())
            .await?,
    ))
}

async fn map_photo(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path((id, section)): Path<(i64, String)>,
    PhotoBody(jpeg): PhotoBody,
) -> Result<StatusCode, AppError> {
    state
        .missions
        .set_map_photo(id, &fm, &section, &jpeg, now_ms())
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn close(
    _: FlugmappeUser,
    Binding(fm): Binding,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<FmMission>, AppError> {
    Ok(Json(
        state
            .missions
            .close_from_flugmappe(id, &fm, now_ms())
            .await?,
    ))
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode, header};
    use http_body_util::BodyExt;
    use serde_json::{Value, json};
    use tower::ServiceExt;

    use crate::auth::oidc::test_idp::{self, TestIdp};
    use crate::contract::{
        FlightEndReason, FlightSummary, FlugmappeSettings, OidcSettingsInput, Role,
    };
    use crate::testing::{TestApp, get, post_json, send, test_app, with_cookie};

    const CLIENT: &str = "flugmappe";
    const ORIGIN: &str = "https://uav.example.org";
    const FM_A: &str = "0f6b2a1e-5d43-4c8e-9b7a-2e1f3c4d5a6b";
    const FM_B: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

    struct Setup {
        app: TestApp,
        idp: TestIdp,
        token: String,
    }

    /// App mit Pocket ID (Test-IdP), eingeschalteter Schnittstelle und einem Konto für `SUBJECT`.
    async fn setup() -> Setup {
        let app = test_app().await;
        let idp = test_idp::start("suchlage").await;
        let settings = &app.state.settings;
        settings
            .update_oidc(OidcSettingsInput {
                issuer: idp.issuer.clone(),
                client_id: "suchlage".into(),
                client_secret: Some("geheim".into()),
                admin_group: "suchlage-admin".into(),
                user_group: "suchlage".into(),
            })
            .await
            .unwrap();
        settings
            .update_flugmappe(FlugmappeSettings {
                client_id: CLIENT.into(),
                allowed_origin: ORIGIN.into(),
            })
            .await
            .unwrap();
        crate::store::users::upsert_oidc(
            &app.state.pool,
            &idp.issuer,
            test_idp::SUBJECT,
            "Testpilot",
            crate::contract::Role::Admin,
            0,
        )
        .await
        .unwrap();
        let token = idp.access_token_for(test_idp::SUBJECT, CLIENT, 300);
        Setup { app, idp, token }
    }

    fn request(
        method: &str,
        uri: &str,
        token: Option<&str>,
        fm: Option<&str>,
        body: Option<Value>,
    ) -> Request<Body> {
        let mut req = Request::builder()
            .method(method)
            .uri(format!("/api/flugmappe/v1{uri}"));
        if let Some(t) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        if let Some(fm) = fm {
            req = req.header("X-Flugmappe-Einsatz", fm);
        }
        match body {
            Some(b) => req
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(b.to_string()))
                .unwrap(),
            None => req.body(Body::empty()).unwrap(),
        }
    }

    impl Setup {
        async fn call(
            &self,
            method: &str,
            uri: &str,
            fm: Option<&str>,
            body: Option<Value>,
        ) -> (StatusCode, Value) {
            send(
                &self.app.router,
                request(method, uri, Some(&self.token), fm, body),
            )
            .await
        }

        async fn create(&self, fm: &str) -> (StatusCode, Value) {
            let body = json!({
                "name": "Vermisste Person", "location": "Forst", "section_label": "Abschnitt 1",
                "section_location": null, "flugmappe_mission_id": fm, "flugmappe_section_id": "s1"
            });
            self.call("POST", "/missions", None, Some(body)).await
        }

        async fn created(&self, fm: &str) -> i64 {
            let (status, m) = self.create(fm).await;
            assert_eq!(status, StatusCode::OK, "{m}");
            m["id"].as_i64().unwrap()
        }
    }

    fn flight(id: i64, ended: Option<i64>) -> FlightSummary {
        FlightSummary {
            id,
            aircraft_sn: "AC1".into(),
            gateway_sn: "RC1".into(),
            started_at_ms: 1_000 * id,
            ended_at_ms: ended,
            end_reason: ended.map(|_| FlightEndReason::Landed),
            max_elevation_m: Some(80.0),
            distance_m: 1_200.0,
        }
    }

    #[tokio::test]
    async fn without_or_with_a_broken_token_is_401() {
        let s = setup().await;
        let (status, body) = send(&s.app.router, request("GET", "/me", None, None, None)).await;
        assert_eq!(
            (status, body["error"].as_str()),
            (StatusCode::UNAUTHORIZED, Some("Nicht angemeldet"))
        );
        let basic = Request::get("/api/flugmappe/v1/me")
            .header(header::AUTHORIZATION, "Basic dXNlcjpwdw==")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&s.app.router, basic).await.0, StatusCode::UNAUTHORIZED);
        let wrong_client = s.idp.access_token_for(test_idp::SUBJECT, "suchlage", 300);
        let expired = s.idp.access_token_for(test_idp::SUBJECT, CLIENT, -120);
        let id_token_typ = s.idp.access_token(test_idp::AccessToken {
            typ: "JWT".into(),
            ..test_idp::AccessToken::new(test_idp::SUBJECT, CLIENT)
        });
        for token in ["kaputt", "a.b.c", &wrong_client, &expired, &id_token_typ] {
            let (status, body) = send(
                &s.app.router,
                request("GET", "/me", Some(token), None, None),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{token}: {body}");
        }
    }

    #[tokio::test]
    async fn an_unreachable_idp_is_503_only_for_otherwise_valid_tokens() {
        let s = setup().await;
        s.idp.set_down(true);
        let expired = s.idp.access_token_for(test_idp::SUBJECT, CLIENT, -120);
        for token in ["kaputt", &expired] {
            let (status, body) = send(
                &s.app.router,
                request("GET", "/me", Some(token), None, None),
            )
            .await;
            assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
        }
        assert_eq!(s.idp.discovery_count(), 0, "Müll fragt den Anbieter nicht");
        let (status, _) = s.call("GET", "/me", None, None).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    }

    #[tokio::test]
    async fn a_token_without_an_account_is_403() {
        let s = setup().await;
        let stranger = s.idp.access_token_for("niemand", CLIENT, 300);
        let (status, _) = send(
            &s.app.router,
            request("GET", "/me", Some(&stranger), None, None),
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        let (status, body) = s.call("GET", "/me", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            body,
            json!({"name": "Testpilot", "role": "admin", "api_version": 1})
        );
    }

    /// Bearer-Aufrufe haben keine Sitzung: Die Sperre wirkt beim nächsten Aufruf.
    #[tokio::test]
    async fn a_blocked_account_is_403_at_the_next_call() {
        let s = setup().await;
        assert_eq!(s.call("GET", "/me", None, None).await.0, StatusCode::OK);
        let pool = &s.app.state.pool;
        let user = crate::store::users::find_by_oidc(pool, &s.idp.issuer, test_idp::SUBJECT)
            .await
            .unwrap()
            .unwrap();
        crate::store::users::block(pool, user.id, 1).await.unwrap();
        assert_eq!(
            s.call("GET", "/me", None, None).await.0,
            StatusCode::FORBIDDEN
        );
        crate::store::users::unblock(pool, user.id).await.unwrap();
        assert_eq!(s.call("GET", "/me", None, None).await.0, StatusCode::OK);
    }

    #[tokio::test]
    async fn switched_off_is_503_for_every_call() {
        let s = setup().await;
        s.app.state.settings.remove_flugmappe().await.unwrap();
        for (method, uri) in [
            ("GET", "/me"),
            ("GET", "/missions"),
            ("POST", "/missions/1/close"),
        ] {
            let (status, _) = s.call(method, uri, Some(FM_A), None).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{uri}");
            let (status, _) = send(&s.app.router, request(method, uri, None, None, None)).await;
            assert_eq!(
                status,
                StatusCode::SERVICE_UNAVAILABLE,
                "auch ohne Token: {uri}"
            );
        }
    }

    #[tokio::test]
    async fn a_token_creates_a_mission_and_reads_its_flights_and_map() {
        let s = setup().await;
        let (status, m) = s.create(FM_A).await;
        assert_eq!(status, StatusCode::OK, "{m}");
        assert_eq!(m["state"], "running");
        assert_eq!(m["link"]["flugmappe_mission_id"], FM_A);
        assert_eq!(m["link"]["linked_by"], "Testpilot");
        assert_eq!(m["sections"][0]["flugmappe_section_id"], "s1");
        let id = m["id"].as_i64().unwrap();

        crate::store::flights::insert(&s.app.state.pool, &flight(1, Some(5_000)))
            .await
            .unwrap();
        let (status, flights) = s
            .call("GET", &format!("/missions/{id}/flights"), Some(FM_A), None)
            .await;
        assert_eq!(status, StatusCode::OK, "{flights}");
        assert_eq!(flights.as_array().unwrap().len(), 1);
        assert_eq!(flights[0]["flugmappe_section_id"], "s1");
        assert_eq!(flights[0]["end_reason"], "landed");
        assert_eq!(flights[0]["photo_count"], 0);

        let (status, body) = s
            .call("GET", &format!("/missions/{id}/flights"), None, None)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let (status, _) = s
            .call(
                "GET",
                &format!("/missions/{id}/flights"),
                Some("keine-uuid"),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);

        let (status, list) = s.call("GET", "/missions", None, None).await;
        assert_eq!((status, list[0]["id"].as_i64()), (StatusCode::OK, Some(id)));
        let (status, one) = s
            .call("GET", &format!("/missions/{id}"), Some(FM_A), None)
            .await;
        assert_eq!((status, &one), (StatusCode::OK, &m));
        let (status, map) = s
            .call(
                "GET",
                &format!("/missions/{id}/map?section=s1"),
                Some(FM_A),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(map["type"], "FeatureCollection");
        let (status, _) = s
            .call(
                "GET",
                &format!("/missions/{id}/map?section=s9"),
                Some(FM_A),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = s
            .call("GET", "/missions/999/flights", Some(FM_A), None)
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn the_location_set_after_relocating_reaches_the_section() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let section =
            json!({"flugmappe_section_id": "s2", "label": "Standort 2", "location": null});
        let (status, _) = s
            .call(
                "POST",
                &format!("/missions/{id}/sections"),
                Some(FM_A),
                Some(section),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let uri = format!("/missions/{id}/sections/s2");
        let put = |body: Value| s.call("PUT", &uri, Some(FM_A), Some(body));
        let (status, m) = put(json!({"label": "Standort 2", "location": "Waldrand"})).await;
        assert_eq!(status, StatusCode::OK, "{m}");
        assert_eq!(m["sections"][1]["location"], "Waldrand");
        let (_, one) = s
            .call("GET", &format!("/missions/{id}"), Some(FM_A), None)
            .await;
        assert_eq!(one, m);
        let (status, _) = put(json!({"label": "", "location": null})).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let (status, _) = s
            .call(
                "PUT",
                &format!("/missions/{id}/sections/s9"),
                Some(FM_A),
                Some(json!({"label": "X", "location": null})),
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn a_second_running_mission_and_a_foreign_binding_are_409() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let (status, body) = s.create(FM_B).await;
        assert_eq!(status, StatusCode::CONFLICT, "{body}");
        assert_eq!(body["error"], "Es läuft bereits ein anderer Einsatz");
        let (status, body) = s
            .call("GET", &format!("/missions/{id}/flights"), Some(FM_B), None)
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(
            body["error"],
            "Der Einsatz ist mit einer anderen Flugmappe verknüpft"
        );
        let section = json!({"flugmappe_section_id": "t2", "label": "Verlegung", "location": null});
        let import = json!({"features": [], "standort": null});
        for (method, uri, body) in [
            ("POST", format!("/missions/{id}/sections"), Some(section)),
            (
                "PUT",
                format!("/missions/{id}/sections/s1"),
                Some(json!({"label": "Abschnitt 1", "location": "Forst"})),
            ),
            ("GET", format!("/missions/{id}"), None),
            ("GET", format!("/missions/{id}/map"), None),
            ("POST", format!("/missions/{id}/map/import"), Some(import)),
            ("POST", format!("/missions/{id}/close"), None),
        ] {
            let (status, body) = s.call(method, &uri, Some(FM_B), body).await;
            assert_eq!(status, StatusCode::CONFLICT, "{method} {uri}: {body}");
            assert_eq!(
                body["error"],
                "Der Einsatz ist mit einer anderen Flugmappe verknüpft"
            );
        }
        let (_, m) = s
            .call("GET", &format!("/missions/{id}"), Some(FM_A), None)
            .await;
        assert_eq!(m["state"], "running", "die fremde schließt nichts");
    }

    #[tokio::test]
    async fn a_browser_mission_without_flugmappe_is_not_linked() {
        let s = setup().await;
        let m = s
            .app
            .state
            .missions
            .create(
                crate::contract::MissionCreate {
                    name: "Aus dem Browser".into(),
                    location: None,
                },
                "Browser",
                1,
            )
            .await
            .unwrap();
        for uri in [
            format!("/missions/{}", m.id),
            format!("/missions/{}/flights", m.id),
        ] {
            let (status, body) = s.call("GET", &uri, Some(FM_A), None).await;
            assert_eq!(status, StatusCode::CONFLICT, "{uri}");
            assert_eq!(
                body["error"],
                "Der Einsatz ist nicht mit dieser Flugmappe verknüpft"
            );
        }
    }

    #[tokio::test]
    async fn creating_again_after_a_lost_answer_returns_the_same_mission() {
        let s = setup().await;
        let (status, first) = s.create(FM_A).await;
        assert_eq!(status, StatusCode::OK, "{first}");
        let (status, again) = s.create(FM_A).await;
        assert_eq!((status, &again), (StatusCode::OK, &first));
        let (_, list) = s.call("GET", "/missions", None, None).await;
        assert_eq!(list.as_array().unwrap().len(), 1);

        let id = first["id"].as_i64().unwrap();
        let (status, _) = s
            .call("POST", &format!("/missions/{id}/close"), Some(FM_A), None)
            .await;
        assert_eq!(status, StatusCode::OK);
        let (status, body) = s.create(FM_A).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
        assert_eq!(body["error"], "Der Einsatz ist abgeschlossen");
    }

    #[tokio::test]
    async fn taking_over_replaces_the_old_flugmappe() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let link = |take_over: bool| {
            json!({"flugmappe_mission_id": FM_B, "flugmappe_section_id": "t1",
                   "section_label": "Abschnitt 1", "section_location": null, "take_over": take_over})
        };
        let (status, _) = s
            .call(
                "POST",
                &format!("/missions/{id}/link"),
                None,
                Some(link(false)),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let (status, m) = s
            .call(
                "POST",
                &format!("/missions/{id}/link"),
                None,
                Some(link(true)),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{m}");
        assert_eq!(m["link"]["flugmappe_mission_id"], FM_B);
        assert_eq!(m["sections"][0]["flugmappe_section_id"], "t1");
        let (status, _) = s
            .call("GET", &format!("/missions/{id}/flights"), Some(FM_A), None)
            .await;
        assert_eq!(
            status,
            StatusCode::CONFLICT,
            "die abgelöste bekommt nichts mehr"
        );
        let (status, _) = s
            .call("GET", &format!("/missions/{id}/flights"), Some(FM_B), None)
            .await;
        assert_eq!(status, StatusCode::OK);
    }

    #[tokio::test]
    async fn a_closed_mission_is_422_and_closing_again_is_fine() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        crate::store::flights::insert(&s.app.state.pool, &flight(1, None))
            .await
            .unwrap();
        let close = format!("/missions/{id}/close");
        let (status, _) = s.call("POST", &close, Some(FM_A), None).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "erst landen");
        crate::store::flights::update(&s.app.state.pool, &flight(1, Some(9_000)))
            .await
            .unwrap();
        let (status, m) = s.call("POST", &close, Some(FM_A), None).await;
        assert_eq!(
            (status, m["state"].as_str()),
            (StatusCode::OK, Some("closed"))
        );
        let (status, _) = s.call("POST", &close, Some(FM_A), None).await;
        assert_eq!(status, StatusCode::OK, "Abschluss nachholen");

        let section = json!({"flugmappe_section_id": "s2", "label": "Verlegung", "location": null});
        let (status, _) = s
            .call(
                "POST",
                &format!("/missions/{id}/sections"),
                Some(FM_A),
                Some(section),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let link = json!({"flugmappe_mission_id": FM_A, "flugmappe_section_id": "s1",
                          "section_label": "Abschnitt 1", "section_location": null, "take_over": false});
        let (status, _) = s
            .call("POST", &format!("/missions/{id}/link"), None, Some(link))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let (status, list) = s.call("GET", "/missions", None, None).await;
        assert_eq!((status, list), (StatusCode::OK, json!([])));
        let (status, m) = s
            .call("GET", &format!("/missions/{id}"), Some(FM_A), None)
            .await;
        assert_eq!(
            (status, m["state"].as_str()),
            (StatusCode::OK, Some("closed"))
        );
    }

    #[tokio::test]
    async fn drawings_are_imported_once() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let body = json!({
            "features": [{"name": null, "kind": "polygon", "points": [
                {"lat": 52.433, "lon": 10.008}, {"lat": 52.433, "lon": 10.016}, {"lat": 52.4305, "lon": 10.016}]}],
            "standort": {"lat": 52.43, "lon": 10.01}
        });
        let uri = format!("/missions/{id}/map/import");
        let (status, done) = s.call("POST", &uri, Some(FM_A), Some(body.clone())).await;
        assert_eq!((status, done), (StatusCode::OK, json!({"imported": 2})));
        let (_, done) = s.call("POST", &uri, Some(FM_A), Some(body)).await;
        assert_eq!(done, json!({"imported": 0}));
        let (_, map) = s
            .call("GET", &format!("/missions/{id}/map"), Some(FM_A), None)
            .await;
        assert_eq!(map["features"].as_array().unwrap().len(), 2);
        let bad = json!({"features": [{"name": null, "kind": "line", "points": [{"lat": 52.0, "lon": 10.0}]}], "standort": null});
        let (status, _) = s.call("POST", &uri, Some(FM_A), Some(bad)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn large_drawings_pass_the_router_limit_up_to_their_own() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let uri = format!("/missions/{id}/map/import");
        let line = |n: usize| {
            let points: Vec<Value> = (0..n)
                .map(|i| json!({"lat": 52.0 + i as f64 * 1e-5, "lon": 10.123_456_789}))
                .collect();
            json!({"name": "Spur", "kind": "line", "points": points})
        };
        let big = json!({"features": vec![line(20); 200], "standort": null});
        assert!(
            big.to_string().len() > 64 * 1024,
            "über der Grenze des Routers"
        );
        let (status, done) = s.call("POST", &uri, Some(FM_A), Some(big)).await;
        assert_eq!((status, done), (StatusCode::OK, json!({"imported": 200})));
        let huge = json!({"features": vec![line(1_000); 60], "standort": null});
        assert!(huge.to_string().len() > super::MAP_IMPORT_MAX_BYTES);
        let (status, _) = s.call("POST", &uri, Some(FM_A), Some(huge)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    async fn raw(s: &Setup, req: Request<Body>) -> axum::response::Response {
        s.app.router.clone().oneshot(req).await.unwrap()
    }

    fn preflight(origin: &str) -> Request<Body> {
        Request::builder()
            .method("OPTIONS")
            .uri("/api/flugmappe/v1/missions/1/sections/s1/photo")
            .header(header::ORIGIN, origin)
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "PUT")
            .header(
                header::ACCESS_CONTROL_REQUEST_HEADERS,
                "authorization, x-flugmappe-einsatz",
            )
            .body(Body::empty())
            .unwrap()
    }

    #[tokio::test]
    async fn cors_preflight_is_allowed_only_for_the_set_origin() {
        let s = setup().await;
        let res = raw(&s, preflight(ORIGIN)).await;
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
        let h = res.headers();
        assert_eq!(h[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);
        assert!(
            h[header::ACCESS_CONTROL_ALLOW_METHODS]
                .to_str()
                .unwrap()
                .contains("PUT")
        );
        assert!(
            h[header::ACCESS_CONTROL_ALLOW_HEADERS]
                .to_str()
                .unwrap()
                .contains("X-Flugmappe-Einsatz")
        );
        assert_eq!(h[header::ACCESS_CONTROL_MAX_AGE], "600");
        assert_eq!(h[header::VARY], "Origin");
        assert!(h.get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS).is_none());

        let res = raw(&s, preflight("https://evil.example.org")).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        assert!(
            res.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );

        // Auch Fehlerantworten tragen die Erlaubnis, sonst sieht die Flugmappe den Status nicht.
        let req = Request::get("/api/flugmappe/v1/me")
            .header(header::ORIGIN, ORIGIN)
            .body(Body::empty())
            .unwrap();
        let res = raw(&s, req).await;
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(res.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], ORIGIN);

        // Nur dieser Bereich: Die Browser-API kennt kein CORS.
        let mut other = preflight(ORIGIN);
        *other.uri_mut() = "/api/missions".parse().unwrap();
        assert!(
            raw(&s, other)
                .await
                .headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );

        // Die Einstellung gilt ohne Neustart.
        s.app.state.settings.remove_flugmappe().await.unwrap();
        let res = raw(&s, preflight(ORIGIN)).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN);
        assert!(
            res.headers()
                .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
                .is_none()
        );
    }

    fn photo(id: i64, section: &str, bytes: Vec<u8>, token: &str, fm: &str) -> Request<Body> {
        Request::put(format!(
            "/api/flugmappe/v1/missions/{id}/sections/{section}/photo"
        ))
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .header("X-Flugmappe-Einsatz", fm)
        .header(header::CONTENT_TYPE, "image/jpeg")
        .body(Body::from(bytes))
        .unwrap()
    }

    #[tokio::test]
    async fn map_photo_is_checked_stored_and_shown_to_the_browser() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, b'J', b'F', b'I', b'F'];
        let (status, body) = send(
            &s.app.router,
            photo(id, "s1", b"\x89PNG\r\n\x1a\n".to_vec(), &s.token, FM_A),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        let mut huge = jpeg.clone();
        huge.resize(super::MAP_PHOTO_MAX_BYTES + 1, 0);
        let (status, body) = send(&s.app.router, photo(id, "s1", huge, &s.token, FM_A)).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE, "{body}");
        assert_eq!(body["error"], "Kartenfoto: höchstens 8 MiB");
        let (status, _) = send(&s.app.router, photo(id, "s9", jpeg.clone(), &s.token, FM_A)).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        let (status, _) = send(&s.app.router, photo(id, "s1", jpeg.clone(), &s.token, FM_B)).await;
        assert_eq!(status, StatusCode::CONFLICT);
        let (status, _) = send(&s.app.router, photo(id, "s1", jpeg.clone(), &s.token, FM_A)).await;
        assert_eq!(status, StatusCode::NO_CONTENT);

        let mission = s.app.state.missions.get(id).await.unwrap();
        let section = &mission.sections[0];
        assert!(section.has_map_photo);
        let kiosk = s.app.cookie(Role::Kiosk).await;
        let uri = format!("/api/missions/{id}/sections/{}/map-photo", section.id);
        let res = raw(&s, with_cookie(get(&uri), &kiosk)).await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "image/jpeg");
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.to_vec(), jpeg);
        assert_eq!(
            send(&s.app.router, get(&uri)).await.0,
            StatusCode::UNAUTHORIZED
        );
        let other = format!("/api/missions/{}/sections/{}/map-photo", id + 1, section.id);
        assert_eq!(
            send(&s.app.router, with_cookie(get(&other), &kiosk))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn the_browser_adds_no_sections_to_a_linked_mission() {
        let s = setup().await;
        let id = s.created(FM_A).await;
        let op = s.app.cookie(Role::Operator).await;
        let req = post_json(
            &format!("/api/missions/{id}/sections"),
            json!({"label": "Nord", "location": null}),
        );
        let (status, body) = send(&s.app.router, with_cookie(req, &op)).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body["error"],
            "Abschnitte kommen bei diesem Einsatz aus der Flugmappe"
        );
        let (_, list) = send(&s.app.router, with_cookie(get("/api/missions"), &op)).await;
        assert_eq!(list[0]["origin"], "flugmappe");
        assert_eq!(list[0]["flugmappe"]["linked_by"], "Testpilot");
    }
}

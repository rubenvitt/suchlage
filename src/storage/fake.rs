//! Fake von Garage für Tests: Admin-API unter `/v2/…` und S3 (Path-Style) unter derselben Adresse.
//! Merkt sich Schlüssel, Freigaben und Objekte; prüft Bearer-Token und das Signaturformat, nicht
//! die Signatur selbst (die prüfen die AWS-Testvektoren in `s3`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::body::Bytes;
use axum::extract::{Query, Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::settings::PhotoStorageSettings;

pub const TOKEN: &str = "admin-test";
pub const BUCKET: &str = "suchlage-fotos";
pub const INBOX: &str = "suchlage-fotos-eingang";

#[derive(Default)]
pub struct Seen {
    /// (id, name, expiration)
    pub keys: Vec<(String, String, Option<String>)>,
    pub grants: Vec<Value>,
    pub deleted: Vec<String>,
    /// Pfad (`/<bucket>/<key>`) → Inhalt
    pub objects: HashMap<String, Vec<u8>>,
    /// Simuliert einen nicht erreichbaren Fotospeicher: S3 antwortet mit 503.
    pub down: bool,
    /// Simuliert einen hängenden Fotospeicher: S3 antwortet nie.
    pub hang: bool,
}

pub type Shared = Arc<Mutex<Seen>>;

fn authorized(h: &HeaderMap) -> bool {
    h.get("authorization")
        .is_some_and(|v| v == &format!("Bearer {TOKEN}")[..])
}

type Params = Query<HashMap<String, String>>;

async fn bucket_info(h: HeaderMap, Query(q): Params) -> Response {
    if !authorized(&h) {
        return StatusCode::FORBIDDEN.into_response();
    }
    match q.get("globalAlias").map(String::as_str) {
        Some(BUCKET) => Json(json!({"id": "b1"})).into_response(),
        Some(INBOX) => Json(json!({"id": "b2"})).into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn create_key(State(seen): State<Shared>, h: HeaderMap, Json(body): Json<Value>) -> Response {
    if !authorized(&h) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let mut s = seen.lock().unwrap();
    let id = format!("GK{}", s.keys.len() + 1);
    s.keys.push((
        id.clone(),
        body["name"].as_str().unwrap_or_default().into(),
        body["expiration"].as_str().map(Into::into),
    ));
    Json(json!({
        "accessKeyId": id, "secretAccessKey": format!("{id}-geheim"), "name": body["name"],
        "expired": false, "permissions": {}, "buckets": [],
    }))
    .into_response()
}

async fn allow(State(seen): State<Shared>, Json(body): Json<Value>) -> Json<Value> {
    seen.lock().unwrap().grants.push(body);
    Json(json!({"id": "b1"}))
}

async fn list_keys(State(seen): State<Shared>) -> Json<Value> {
    let s = seen.lock().unwrap();
    let mut list: Vec<Value> = s
        .keys
        .iter()
        .filter(|(id, _, _)| !s.deleted.contains(id))
        .map(|(id, name, exp)| json!({"id": id, "name": name, "expired": false, "expiration": exp}))
        .collect();
    list.push(json!({"id": "GKfremd", "name": "anderer", "expired": true}));
    Json(Value::Array(list))
}

async fn delete_key(State(seen): State<Shared>, Query(q): Params) -> StatusCode {
    seen.lock().unwrap().deleted.push(q["id"].clone());
    StatusCode::OK
}

async fn s3(State(seen): State<Shared>, req: Request) -> Response {
    let signed = req
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("AWS4-HMAC-SHA256 Credential=GK"));
    if !signed {
        return StatusCode::FORBIDDEN.into_response();
    }
    if seen.lock().unwrap().hang {
        std::future::pending::<()>().await;
    }
    let (method, path) = (req.method().clone(), req.uri().path().to_string());
    let copy_source = req
        .headers()
        .get("x-amz-copy-source")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let body: Bytes = axum::body::to_bytes(req.into_body(), usize::MAX)
        .await
        .unwrap_or_default();
    let mut s = seen.lock().unwrap();
    if s.down {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    }
    match method {
        Method::PUT => {
            let data = match copy_source {
                Some(source) => match s.objects.get(&source) {
                    Some(o) => o.clone(),
                    None => return StatusCode::NOT_FOUND.into_response(),
                },
                None => body.to_vec(),
            };
            s.objects.insert(path, data);
            StatusCode::OK.into_response()
        }
        Method::HEAD => match s.objects.get(&path) {
            Some(o) => ([("content-length", o.len().to_string())], "").into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        Method::GET => match s.objects.get(&path) {
            Some(o) => o.clone().into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        Method::DELETE => match s.objects.remove(&path) {
            Some(_) => StatusCode::NO_CONTENT.into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        _ => StatusCode::METHOD_NOT_ALLOWED.into_response(),
    }
}

/// Startet den Fake und liefert seine Adresse.
pub async fn start() -> (String, Shared) {
    let seen: Shared = Arc::default();
    let app = Router::new()
        .route("/v2/GetBucketInfo", get(bucket_info))
        .route("/v2/CreateKey", post(create_key))
        .route("/v2/AllowBucketKey", post(allow))
        .route("/v2/ListKeys", get(list_keys))
        .route("/v2/DeleteKey", post(delete_key))
        .fallback(s3)
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), seen)
}

/// Einstellungen, die auf den Fake zeigen.
pub fn settings(url: &str) -> PhotoStorageSettings {
    PhotoStorageSettings {
        s3_url: url.into(),
        rc_url: "http://100.64.0.10:3900".into(),
        admin_url: url.into(),
        bucket: BUCKET.into(),
        region: "garage".into(),
        access_key_id: "GKserver".into(),
        secret_access_key: "server-geheim".into(),
        admin_token: TOKEN.into(),
    }
}

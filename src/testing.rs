//! Testhelfer für Handler-Tests (nur `cfg(test)`): App mit In-Memory-DB und Lage-Aktor, Anmeldung.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tokio::sync::{mpsc, watch};
use tower::ServiceExt;

use crate::app::{AppState, router};
use crate::config::BootConfig;
use crate::contract::Role;
use crate::situation::state::Config;
use crate::store::{sessions, users};

pub struct TestApp {
    pub router: Router,
    pub state: AppState,
    /// Livebild-Aufträge des Aktors (Gateway-SN). Der Empfänger bleibt offen, solange die App lebt.
    #[expect(
        dead_code,
        reason = "hält wie `_stop` den Kanal offen; noch liest kein Handler-Test die Aufträge"
    )]
    pub live_requests: mpsc::UnboundedReceiver<String>,
    _stop: watch::Sender<bool>,
}

/// App wie in `app::run`, aber mit In-Memory-DB, öffentlicher HTTPS-Adresse und ohne Auto-Livebild.
pub async fn test_app() -> TestApp {
    let pool = crate::db::test_pool().await;
    let (stop, stop_rx) = watch::channel(false);
    let (live_tx, live_requests) = mpsc::unbounded_channel();
    let config = Config {
        auto_live_on_takeoff: false,
    };
    let (situation, _task) = crate::runtime::service::start(
        pool.clone(),
        config,
        crate::runtime::system_clock(),
        live_tx,
        stop_rx,
    )
    .await
    .unwrap();
    let boot = BootConfig::from_lookup(|k| {
        (k == "SUCHLAGE_PUBLIC_URL").then(|| "https://suchlage.test".to_string())
    })
    .unwrap();
    let state = AppState {
        boot: Arc::new(boot),
        pool,
        situation,
    };
    TestApp {
        router: router(state.clone()),
        state,
        live_requests,
        _stop: stop,
    }
}

impl TestApp {
    /// Legt ein Konto mit der Rolle an (beim Großbild einen Token) und liefert `suchlage_sid=<token>`.
    pub async fn cookie(&self, role: Role) -> String {
        let pool = &self.state.pool;
        if role == Role::Kiosk {
            let (kiosk, _) = crate::store::kiosk::create(pool, "Großbild", 0)
                .await
                .unwrap();
            let token = sessions::create_for_kiosk(pool, kiosk.id, crate::time::now_ms())
                .await
                .unwrap();
            return format!("{}={token}", crate::auth::session::COOKIE);
        }
        if role == Role::Admin && users::count(pool).await.unwrap() > 0 {
            panic!("Admin zuerst anlegen: das erste Konto wird Admin");
        }
        if role == Role::Operator && users::count(pool).await.unwrap() == 0 {
            users::upsert_oidc(pool, "test", "erster", "Admin", 0)
                .await
                .unwrap();
        }
        let sub = format!("{role:?}-{}", crate::token::generate());
        let user = users::upsert_oidc(pool, "test", &sub, &format!("{role:?}"), 0)
            .await
            .unwrap();
        assert_eq!(user.role, role);
        let token = sessions::create_for_user(pool, user.id, crate::time::now_ms())
            .await
            .unwrap();
        format!("{}={token}", crate::auth::session::COOKIE)
    }
}

pub fn with_cookie(mut req: Request<Body>, cookie: &str) -> Request<Body> {
    req.headers_mut().insert("cookie", cookie.parse().unwrap());
    req
}

pub async fn send(router: &Router, req: Request<Body>) -> (StatusCode, serde_json::Value) {
    let res = router.clone().oneshot(req).await.expect("Router antwortet");
    let status = res.status();
    let bytes = res
        .into_body()
        .collect()
        .await
        .expect("Body lesbar")
        .to_bytes();
    let json = if bytes.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| serde_json::Value::String(String::from_utf8_lossy(&bytes).into()))
    };
    (status, json)
}

pub fn get(uri: &str) -> Request<Body> {
    Request::get(uri).body(Body::empty()).unwrap()
}

pub fn post_json(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::post(uri)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}

//! Zustand und Router. Spätere Tasks ergänzen Felder und Routen, die Struktur bleibt.

use std::future::Future;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::routing::get;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, watch};

use crate::config::BootConfig;
use crate::runtime::service::{self, SituationHandle};
use crate::situation::state::Config;

#[derive(Clone)]
pub struct AppState {
    pub boot: Arc<BootConfig>,
    pub pool: SqlitePool,
    pub situation: SituationHandle,
}

pub fn router(state: AppState) -> Router {
    use tower_http::catch_panic::CatchPanicLayer;
    use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
    use tower_http::trace::TraceLayer;

    // Routen unter /api stehen ohne Präfix im verschachtelten Router; sein Fallback liefert JSON-404,
    // damit eine falsche API-Adresse nie die SPA bekommt. Spätere Tasks ergänzen hier `.merge(...)`.
    let api = Router::new()
        .route("/health", get(health))
        .merge(crate::web::auth::routes())
        .merge(crate::web::admin::routes())
        .merge(crate::web::live::routes())
        .merge(crate::web::flights::routes())
        .fallback(|| async { crate::error::AppError::NotFound });

    Router::new()
        .nest("/api", api)
        .fallback(crate::static_files::serve)
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

pub async fn run(
    boot: BootConfig,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(&boot.data_dir)?;
    let pool = crate::db::connect(&boot.db_path())
        .await
        .map_err(std::io::Error::other)?;
    crate::db::migrate(&pool)
        .await
        .map_err(std::io::Error::other)?;
    let (stop_tx, stop_rx) = watch::channel(false);
    // B12 hängt hier den Livebild-Auftragnehmer an; bis dahin werden Anfragen nur geloggt.
    let (live_tx, mut live_rx) = mpsc::unbounded_channel::<String>();
    tokio::spawn(async move {
        while let Some(gw) = live_rx.recv().await {
            tracing::info!(gateway = %gw, "Livebild angefordert (noch ohne Befehlsweg)");
        }
    });
    let config = Config {
        auto_live_on_takeoff: true,
    };
    let (situation, situation_task) = service::start(
        pool.clone(),
        config,
        crate::runtime::system_clock(),
        live_tx,
        stop_rx,
    )
    .await
    .map_err(std::io::Error::other)?;
    let listener = tokio::net::TcpListener::bind(boot.bind).await?;
    tracing::info!(bind = %boot.bind, public_url = %boot.public_url, "Suchlage läuft");
    let state = AppState {
        boot: Arc::new(boot),
        pool: pool.clone(),
        situation,
    };
    let result = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await;
    let _ = stop_tx.send(true);
    let _ = situation_task.await;
    pool.close().await;
    result
}

#[cfg(test)]
mod tests {
    use crate::testing::{get as get_req, send, test_app};
    use axum::http::StatusCode;

    #[tokio::test]
    async fn health_reports_ok_and_version() {
        let (status, body) = send(&test_app().await.router, get_req("/api/health")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn unknown_api_route_is_json_404() {
        let (status, body) = send(&test_app().await.router, get_req("/api/gibt-es-nicht")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Nicht gefunden");
    }
}

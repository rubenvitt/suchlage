//! Zustand und Router. Spätere Tasks ergänzen Felder und Routen, die Struktur bleibt.

use std::future::Future;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::routing::get;

use crate::config::BootConfig;

#[derive(Clone)]
pub struct AppState {
    pub boot: Arc<BootConfig>,
}

pub fn router(state: AppState) -> Router {
    use tower_http::catch_panic::CatchPanicLayer;
    use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
    use tower_http::trace::TraceLayer;

    // Routen unter /api stehen ohne Präfix im verschachtelten Router; sein Fallback liefert JSON-404,
    // damit eine falsche API-Adresse nie die SPA bekommt. Spätere Tasks ergänzen hier `.merge(...)`.
    let api = Router::new()
        .route("/health", get(health))
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
    let listener = tokio::net::TcpListener::bind(boot.bind).await?;
    tracing::info!(bind = %boot.bind, public_url = %boot.public_url, "Suchlage läuft");
    let state = AppState {
        boot: Arc::new(boot),
    };
    axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{get as get_req, send};
    use axum::http::StatusCode;

    fn state() -> AppState {
        AppState {
            boot: Arc::new(BootConfig::from_lookup(|_| None).unwrap()),
        }
    }

    #[tokio::test]
    async fn health_reports_ok_and_version() {
        let (status, body) = send(&router(state()), get_req("/api/health")).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["status"], "ok");
        assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn unknown_api_route_is_json_404() {
        let (status, body) = send(&router(state()), get_req("/api/gibt-es-nicht")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Nicht gefunden");
    }
}

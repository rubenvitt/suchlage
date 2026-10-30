//! Zustand und Router. Spätere Tasks ergänzen Felder und Routen, die Struktur bleibt.

use std::future::Future;
use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::FromRef;
use axum::routing::get;
use sqlx::SqlitePool;
use tokio::sync::{mpsc, watch};

use crate::auth::oidc::OidcState;
use crate::config::BootConfig;
use crate::dji::control::LiveControl;
use crate::error::AppError;
use crate::runtime::service::{self, SituationHandle};
use crate::runtime::streams::{StreamOrchestrator, run_auto};
use crate::settings::SettingsHandle;
use crate::situation::observation::{Observation, Observed};
use crate::situation::state::Config;

#[derive(Clone)]
pub struct AppState {
    pub boot: Arc<BootConfig>,
    pub pool: SqlitePool,
    pub situation: SituationHandle,
    pub settings: SettingsHandle,
    pub oidc: Arc<OidcState>,
    pub mqtt: crate::mqtt::MqttLink,
    pub streams: StreamOrchestrator,
    /// Kartenelemente (M2) für Browser und Pilots.
    pub map: crate::map::MapHub,
    /// Fotos aus Pilot 2 (M3).
    pub photos: crate::photos::PhotoHub,
    pub missions: crate::missions::MissionHub,
    /// Löschen von Einsätzen, Flügen und Medien (nur Admin).
    pub deleter: crate::deletion::Deleter,
    /// Für die Proxies zu MediaMTX (`media::http_client`).
    pub http: reqwest::Client,
    /// Funkprotokoll der Fernsteuerungen für die Systemseite.
    pub devlog: crate::devlog::DeviceLog,
}

// Teilzustände für die Extractors in `auth`, damit `auth` den `AppState` nicht kennen muss.
impl FromRef<AppState> for SqlitePool {
    fn from_ref(state: &AppState) -> Self {
        state.pool.clone()
    }
}

impl FromRef<AppState> for Arc<BootConfig> {
    fn from_ref(state: &AppState) -> Self {
        state.boot.clone()
    }
}

impl FromRef<AppState> for SettingsHandle {
    fn from_ref(state: &AppState) -> Self {
        state.settings.clone()
    }
}

impl FromRef<AppState> for Arc<OidcState> {
    fn from_ref(state: &AppState) -> Self {
        state.oidc.clone()
    }
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
        .merge(crate::web::map::routes())
        .merge(crate::web::missions::routes())
        .merge(crate::web::photos::routes())
        .merge(crate::web::streams::routes())
        .merge(crate::web::media::routes())
        .merge(crate::web::devlog::routes())
        // Schnittstelle für die Flugmappe: Bearer statt Sitzung, eigenes CORS (SUL-66).
        .nest(
            "/flugmappe/v2",
            crate::web::flugmappe::routes(state.settings.clone()),
        )
        // Schnittstelle für den Lagemonitor: Großbild-Token als Bearer, nur lesen.
        .nest("/lagemonitor/v1", crate::web::lagemonitor::routes())
        .fallback(|| async { crate::error::AppError::NotFound });

    Router::new()
        .nest("/api", api)
        // Pilot 2 öffnet `/pilot` direkt, nicht unter `/api`.
        .merge(crate::web::pilot::routes())
        // Pilot 2 hängt `/map/api/v1/…` an die Wurzel; der WebSocket liegt unter `/pilot/ws`.
        .merge(crate::web::pilot_map::routes())
        // Medienverwaltung (M3): `/media/api/v1/…` und `/storage/api/v1/…` an der Wurzel.
        .merge(crate::web::pilot_media::routes())
        .merge(crate::web::media::internal_routes())
        .fallback(crate::static_files::serve)
        .layer(axum::extract::DefaultBodyLimit::max(64 * 1024))
        .layer(CatchPanicLayer::new())
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}

async fn health() -> Json<crate::contract::Health> {
    Json(crate::contract::Health {
        status: "ok".into(),
        version: crate::version::VERSION.into(),
        commit: crate::version::COMMIT.map(Into::into),
    })
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
    let settings = SettingsHandle::load(pool.clone())
        .await
        .map_err(std::io::Error::other)?;
    #[cfg(feature = "dev")]
    crate::auth::dev::seed(&settings)
        .await
        .map_err(startup_error)?;
    #[cfg(feature = "dev")]
    crate::auth::dev::seed_simulator(&pool)
        .await
        .map_err(startup_error)?;
    if let Some(link) =
        crate::auth::setup::ensure_token(&pool, &settings, &boot, crate::time::now_ms())
            .await
            .map_err(startup_error)?
    {
        tracing::warn!("Ersteinrichtung: {link}  (24 h gültig, einmal verwendbar)");
    }
    // Vor dem Lesen abonnieren: Keine Änderung fällt zwischen Startwert und Nachführen heraus.
    let settings_rx = settings.subscribe();
    let (stop_tx, stop_rx) = watch::channel(false);
    // Livebild-Aufträge des Kerns beim Abheben; `run_auto` arbeitet sie ab, sobald der Befehlsweg steht.
    let (live_tx, live_rx) = mpsc::unbounded_channel::<String>();
    let config = Config {
        auto_live_on_takeoff: settings_rx.borrow().general.auto_live_on_takeoff,
    };
    let (situation, situation_task) = service::start(
        pool.clone(),
        config,
        crate::runtime::system_clock(),
        live_tx,
        stop_rx.clone(),
    )
    .await
    .map_err(std::io::Error::other)?;
    follow_settings(settings_rx, situation.clone());
    let labels = crate::store::remote_controllers::labels(&pool)
        .await
        .map_err(std::io::Error::other)?;
    situation
        .observe(Observed {
            at_ms: crate::time::now_ms(),
            observation: Observation::Labels(labels),
        })
        .await;
    let map = crate::map::MapHub::new(pool.clone(), stop_rx.clone());
    tokio::spawn(crate::map::cleanup_loop(map.clone(), stop_rx.clone()));
    let (mqtt, link_requests) = crate::mqtt::MqttLink::channel();
    let devlog = crate::devlog::DeviceLog::new(stop_rx.clone());
    // Upload-Schlüssel der Fotos (M3) laufen nach einer Stunde ab; der Lauf löscht sie danach.
    tokio::spawn(crate::storage::garage::cleanup_loop(
        crate::media::http_client(),
        settings.clone(),
        stop_rx.clone(),
    ));
    let session_task = tokio::spawn(crate::dji::session::run(
        settings.subscribe(),
        crate::dji::session::SessionDeps {
            observer: situation.observer(),
            pool: pool.clone(),
            records_dir: boot.records_dir(),
            devlog: devlog.clone(),
        },
        link_requests,
        stop_rx,
    ));
    let streams = StreamOrchestrator::new(
        situation.clone(),
        LiveControl::new(mqtt.clone(), devlog.clone()),
        pool.clone(),
        settings.clone(),
    );
    tokio::spawn(run_auto(live_rx, streams.clone()));
    let listener = tokio::net::TcpListener::bind(boot.bind).await?;
    tracing::info!(
        version = crate::version::VERSION,
        commit = crate::version::COMMIT.unwrap_or("–"),
        bind = %boot.bind,
        public_url = %boot.public_url,
        "Suchlage läuft"
    );
    let photos = crate::photos::PhotoHub::new(
        pool.clone(),
        settings.clone(),
        crate::media::http_client(),
        boot.thumbs_dir(),
    );
    let missions =
        crate::missions::MissionHub::start_up(pool.clone(), map.clone(), boot.map_photos_dir())
            .await
            .map_err(startup_error)?;
    let deleter = crate::deletion::Deleter {
        pool: pool.clone(),
        photos: photos.clone(),
        map: map.clone(),
        missions: missions.clone(),
        video_dir: boot.video_dir(),
    };
    let state = AppState {
        boot: Arc::new(boot),
        pool: pool.clone(),
        situation,
        settings,
        oidc: Arc::new(OidcState::new()),
        mqtt,
        streams,
        map,
        photos,
        missions,
        deleter,
        http: crate::media::http_client(),
        devlog,
    };
    // Der Aktor stoppt schon mit dem Signal: Erst sein Ende schließt offene Live-Ströme, und erst
    // danach kehrt `serve` zurück. Bis dahin melden Anfragen an die Lage 503. Die MQTT-Sitzung
    // trennt mit demselben Signal vom Broker.
    let shutdown = async move {
        shutdown.await;
        let _ = stop_tx.send(true);
    };
    let result = axum::serve(listener, router(state))
        .with_graceful_shutdown(shutdown)
        .await;
    let _ = situation_task.await;
    let _ = session_task.await;
    pool.close().await;
    result
}

/// `Display` verschweigt bei `AppError::Internal` das Detail; beim Start gehört es ins Log.
fn startup_error(e: AppError) -> std::io::Error {
    std::io::Error::other(format!("{e:?}"))
}

/// Die Lage übernimmt „Livebild beim Start“ ohne Neustart. Endet, wenn die Einstellungen wegfallen.
fn follow_settings(
    mut settings: watch::Receiver<crate::settings::Settings>,
    situation: SituationHandle,
) {
    tokio::spawn(async move {
        while settings.changed().await.is_ok() {
            let auto_live_on_takeoff = settings.borrow_and_update().general.auto_live_on_takeoff;
            situation
                .set_config(Config {
                    auto_live_on_takeoff,
                })
                .await;
        }
    });
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
        assert_eq!(body["version"], crate::version::VERSION);
    }

    #[tokio::test]
    async fn unknown_api_route_is_json_404() {
        let (status, body) = send(&test_app().await.router, get_req("/api/gibt-es-nicht")).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["error"], "Nicht gefunden");
    }
}

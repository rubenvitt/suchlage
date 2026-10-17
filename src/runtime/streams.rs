//! Livebild-Ablauf: Quelle wählen, Ziel bauen, Befehl senden, Zustand im Kern führen.
//! Von der API und vom Auto-Start beim Abheben benutzt.

use sqlx::SqlitePool;
use tokio::sync::mpsc;

use crate::contract::{Lens, MediaSettings, StreamPhase, StreamState};
use crate::dji::control::{ControlError, LiveControl};
use crate::error::AppError;
use crate::runtime::service::SituationHandle;
use crate::settings::SettingsHandle;
use crate::situation::observation::{Observation, Observed, StreamCommand};
use crate::situation::stream::idle;
use crate::store;
use crate::time::now_ms;

#[derive(Clone)]
pub struct StreamOrchestrator {
    situation: SituationHandle,
    control: LiveControl,
    pool: SqlitePool,
    settings: SettingsHandle,
}

/// RTMP-Ziel der Fernsteuerung. Enthält den Stream-Schlüssel – nie loggen.
pub fn publish_url(media: &MediaSettings, gateway_sn: &str, stream_key: &str) -> String {
    format!(
        "{}/live/{gateway_sn}?user=rc-{gateway_sn}&pass={stream_key}",
        media.rtmp_url.trim_end_matches('/')
    )
}

/// Startet für jede Gateway-SN aus dem Kern (Abheben mit „Livebild beim Start“) das Standardbild.
/// Endet, wenn der Lage-Aktor seinen Sender fallen lässt.
pub async fn run_auto(mut rx: mpsc::UnboundedReceiver<String>, orchestrator: StreamOrchestrator) {
    while let Some(gateway_sn) = rx.recv().await {
        match orchestrator.start(&gateway_sn, None).await {
            Ok(_) => tracing::info!(gateway = %gateway_sn, "Livebild beim Abheben gestartet"),
            Err(e) => {
                tracing::warn!(gateway = %gateway_sn, error = %e, "Livebild beim Abheben nicht gestartet")
            }
        }
    }
}

impl StreamOrchestrator {
    pub fn new(
        situation: SituationHandle,
        control: LiveControl,
        pool: SqlitePool,
        settings: SettingsHandle,
    ) -> Self {
        Self {
            situation,
            control,
            pool,
            settings,
        }
    }

    async fn observe(&self, observation: Observation) {
        self.situation
            .observe(Observed {
                at_ms: now_ms(),
                observation,
            })
            .await;
    }

    async fn current(&self, gateway_sn: &str) -> Result<StreamState, AppError> {
        Ok(self
            .situation
            .stream(gateway_sn)
            .await?
            .unwrap_or_else(|| idle(gateway_sn, now_ms())))
    }

    /// Meldet Beginn und Ergebnis eines Befehls an den Kern; jeder Ausgang endet mit
    /// `CommandFinished`, damit kein Stream in `starting` hängen bleibt.
    async fn run(
        &self,
        gateway_sn: &str,
        command: StreamCommand,
        sent: impl Future<Output = Result<(), ControlError>>,
    ) -> Result<StreamState, AppError> {
        self.observe(Observation::CommandIssued {
            gateway_sn: gateway_sn.into(),
            command: command.clone(),
        })
        .await;
        let result = sent.await;
        self.observe(Observation::CommandFinished {
            gateway_sn: gateway_sn.into(),
            command,
            result: result.clone().map_err(|e| e.to_string()),
        })
        .await;
        result.map_err(|e| match e {
            ControlError::Refused(message) => AppError::Unprocessable(message),
            ControlError::Unreachable(message) => AppError::Unavailable(message),
        })?;
        self.current(gateway_sn).await
    }

    /// Die Kamera des laufenden Livebilds; Stopp und Linsenwechsel brauchen sie.
    async fn live_video(&self, gateway_sn: &str) -> Result<String, AppError> {
        let current = self.current(gateway_sn).await?;
        match (current.phase, current.video_id) {
            (StreamPhase::Live, Some(video_id)) => Ok(video_id),
            _ => Err(AppError::Unprocessable("Es läuft kein Livebild".into())),
        }
    }

    pub async fn start(
        &self,
        gateway_sn: &str,
        video_id: Option<String>,
    ) -> Result<StreamState, AppError> {
        let gateway = self
            .situation
            .gateway(gateway_sn)
            .await?
            .filter(|g| g.aircraft_sn.is_some())
            .ok_or_else(|| AppError::Conflict("Fernsteuerung nicht verbunden".into()))?;
        // Ein zweites `live_start_push` lehnt Pilot 2 ab (513003); der Zustand würde `failed`,
        // obwohl das Bild läuft.
        if matches!(
            self.current(gateway_sn).await?.phase,
            StreamPhase::Starting | StreamPhase::Live | StreamPhase::Stopping
        ) {
            return Err(AppError::Conflict("Livebild läuft bereits".into()));
        }
        let source = match video_id {
            Some(id) => gateway
                .video_sources
                .iter()
                .find(|s| s.video_id == id)
                .ok_or_else(|| {
                    AppError::Unprocessable("Diese Videoquelle meldet das Fluggerät nicht".into())
                })?,
            None => gateway
                .video_sources
                .iter()
                .find(|s| s.camera_label != "FPV")
                .or(gateway.video_sources.first())
                .ok_or_else(|| {
                    AppError::Unprocessable(
                        "Keine Videoquelle gemeldet – ist das Fluggerät eingeschaltet?".into(),
                    )
                })?,
        };
        let media = self.settings.current().media.ok_or_else(|| {
            AppError::Unavailable("MediaMTX ist nicht eingerichtet (Einstellungen → Video)".into())
        })?;
        let stream_key = store::remote_controllers::find_by_sn(&self.pool, gateway_sn)
            .await?
            .and_then(|rc| rc.stream_key)
            .ok_or_else(|| AppError::Conflict("Fernsteuerung ist nicht gekoppelt".into()))?;
        let url = publish_url(&media, gateway_sn, &stream_key);
        let video_id = source.video_id.clone();
        self.run(
            gateway_sn,
            StreamCommand::Start {
                video_id: video_id.clone(),
            },
            self.control.start(gateway_sn, &video_id, &url),
        )
        .await
    }

    pub async fn stop(&self, gateway_sn: &str) -> Result<StreamState, AppError> {
        let video_id = self.live_video(gateway_sn).await?;
        self.run(
            gateway_sn,
            StreamCommand::Stop,
            self.control.stop(gateway_sn, &video_id),
        )
        .await
    }

    /// Ein abgelehnter Wechsel lässt das Bild mit der bisherigen Linse weiterlaufen.
    pub async fn lens(&self, gateway_sn: &str, lens: Lens) -> Result<StreamState, AppError> {
        let video_id = self.live_video(gateway_sn).await?;
        self.run(
            gateway_sn,
            StreamCommand::ChangeLens { lens },
            self.control.change_lens(gateway_sn, &video_id, lens),
        )
        .await
    }
}

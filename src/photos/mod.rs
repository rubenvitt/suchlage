//! Fotos aus Pilot 2 (M3): annehmen, einem Flug zuordnen und an die Browser verteilen. Das
//! DJI-Format kennt nur `dji::media`, den Objektspeicher nur `storage`.

pub mod rules;

use sqlx::SqlitePool;
use tokio::sync::broadcast;

use crate::contract::PhotoShot;
use crate::error::AppError;
use crate::settings::{PhotoStorageSettings, SettingsHandle};
use crate::storage::garage::{UploadKey, UploadKeys};
use crate::storage::{s3, server_access};
use crate::store::photos::{self as store, NewPhoto};

/// Puffer je Empfänger. Wer mehr verpasst, sieht die Fotos beim nächsten Laden der Liste.
const SHOTS_BUFFERED: usize = 64;
/// So weit vor dem Start und nach der Landung zählt ein Foto noch zum Flug.
pub const FLIGHT_SLACK_MS: i64 = 2 * 60_000;
/// Höchstzahl der Fotos je Abfrage der Lagekarte.
const RECENT_LIMIT: i64 = 2_000;

#[derive(Clone)]
pub struct PhotoHub {
    pool: SqlitePool,
    settings: SettingsHandle,
    http: reqwest::Client,
    shots: broadcast::Sender<PhotoShot>,
    keys: UploadKeys,
}

impl PhotoHub {
    pub fn new(pool: SqlitePool, settings: SettingsHandle, http: reqwest::Client) -> Self {
        let (shots, _) = broadcast::channel(SHOTS_BUFFERED);
        Self {
            pool,
            settings,
            http,
            shots,
            keys: UploadKeys::default(),
        }
    }

    /// Neue oder ergänzte Auslösungen.
    pub fn subscribe(&self) -> broadcast::Receiver<PhotoShot> {
        self.shots.subscribe()
    }

    /// Einstellungen des Fotospeichers; ohne sie nimmt der Server keine Fotos an.
    pub fn storage(&self) -> Result<PhotoStorageSettings, AppError> {
        self.settings.current().photo_storage.ok_or_else(|| {
            AppError::Unavailable(
                "Fotospeicher ist nicht eingerichtet (Einstellungen → Fotospeicher)".into(),
            )
        })
    }

    /// Upload-Schlüssel für die Fernsteuerung (je RC einer, bis kurz vor Ablauf).
    pub async fn upload_key(
        &self,
        cfg: &PhotoStorageSettings,
        rc_sn: &str,
        now_ms: i64,
    ) -> Result<UploadKey, AppError> {
        self.keys
            .for_controller(&self.http, cfg, rc_sn, now_ms)
            .await
    }

    /// Liegt das Objekt im Speicher?
    pub async fn object_exists(
        &self,
        cfg: &PhotoStorageSettings,
        object_key: &str,
    ) -> Result<bool, AppError> {
        let (bucket, creds) = server_access(cfg);
        Ok(s3::head(&self.http, &bucket, &creds, object_key)
            .await?
            .is_some())
    }

    /// Flug, zu dem eine Aufnahme gehört.
    pub async fn flight_at(
        &self,
        gateway_sn: &str,
        aircraft_sn: Option<&str>,
        t_ms: i64,
    ) -> Result<Option<i64>, AppError> {
        Ok(store::flight_at(&self.pool, gateway_sn, aircraft_sn, t_ms, FLIGHT_SLACK_MS).await?)
    }

    /// Speichert das Foto und verteilt seine Auslösung. Liefert die ID des Fotos.
    pub async fn accept(&self, photo: NewPhoto) -> Result<(i64, PhotoShot), AppError> {
        let id = store::upsert(&self.pool, &photo).await?;
        let shot = self
            .shot(&photo.shot_key)
            .await?
            .ok_or_else(|| AppError::Internal("Foto nach dem Speichern nicht lesbar".into()))?;
        // Ohne Browser gibt es keine Empfänger; das ist kein Fehler.
        let _ = self.shots.send(shot.clone());
        Ok((id, shot))
    }

    pub async fn shot(&self, shot_key: &str) -> Result<Option<PhotoShot>, AppError> {
        let photos = store::of_shot(&self.pool, shot_key).await?;
        Ok(rules::shots(&photos).into_iter().next())
    }

    /// Auslösungen ab `since_ms` für die Lagekarte, älteste zuerst.
    pub async fn recent(&self, since_ms: i64) -> Result<Vec<PhotoShot>, AppError> {
        Ok(rules::shots(
            &store::since(&self.pool, since_ms, RECENT_LIMIT).await?,
        ))
    }

    pub async fn of_flight(&self, flight_id: i64) -> Result<Vec<PhotoShot>, AppError> {
        Ok(rules::shots(
            &store::of_flight(&self.pool, flight_id).await?,
        ))
    }

    pub async fn tiny_fingerprints_known(
        &self,
        candidates: &[String],
    ) -> Result<Vec<String>, AppError> {
        Ok(store::known_tiny_fingerprints(&self.pool, candidates).await?)
    }

    pub async fn fingerprint_known(&self, fingerprint: &str) -> Result<bool, AppError> {
        Ok(store::fingerprint_known(&self.pool, fingerprint).await?)
    }
}

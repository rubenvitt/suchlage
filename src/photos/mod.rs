//! Fotos aus Pilot 2 (M3): annehmen, einem Flug zuordnen und an die Browser verteilen. Das
//! DJI-Format kennt nur `dji::media`, den Objektspeicher nur `storage`.

pub mod rules;
pub mod thumb;

use std::path::PathBuf;
use std::sync::Arc;

use sqlx::SqlitePool;
use tokio::sync::{Semaphore, broadcast};

use crate::contract::PhotoShot;
use crate::error::AppError;
use crate::settings::{PhotoStorageSettings, SettingsHandle};
use crate::storage::garage::{UploadKey, UploadKeys};
use crate::storage::{inbox_access, s3, server_access};
use crate::store::photos::{self as store, NewPhoto};

/// Puffer je Empfänger. Wer mehr verpasst, sieht die Fotos beim nächsten Laden der Liste.
const SHOTS_BUFFERED: usize = 64;
/// So weit vor dem Start und nach der Landung zählt ein Foto noch zum Flug.
pub const FLIGHT_SLACK_MS: i64 = 2 * 60_000;
/// Höchstzahl der Fotos je Abfrage der Lagekarte.
const RECENT_LIMIT: i64 = 2_000;
/// Größer ist kein Foto der H20N; größere Dateien bekommen kein Vorschaubild.
const MAX_ORIGINAL_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct PhotoHub {
    pool: SqlitePool,
    settings: SettingsHandle,
    http: reqwest::Client,
    shots: broadcast::Sender<PhotoShot>,
    keys: UploadKeys,
    thumbs_dir: PathBuf,
    /// Ein Vorschaubild nach dem anderen: Ein 20-MP-Foto belegt dekodiert rund 60 MB.
    thumb_jobs: Arc<Semaphore>,
}

impl PhotoHub {
    pub fn new(
        pool: SqlitePool,
        settings: SettingsHandle,
        http: reqwest::Client,
        thumbs_dir: PathBuf,
    ) -> Self {
        let (shots, _) = broadcast::channel(SHOTS_BUFFERED);
        Self {
            pool,
            settings,
            http,
            shots,
            keys: UploadKeys::default(),
            thumbs_dir,
            thumb_jobs: Arc::new(Semaphore::new(1)),
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

    /// Übernimmt ein hochgeladenes Foto aus dem Eingang in den Fotospeicher (ADR 0004). `false`,
    /// wenn es weder dort noch schon im Fotospeicher liegt; ein wiederholter Callback findet es dort.
    pub async fn take_upload(
        &self,
        cfg: &PhotoStorageSettings,
        object_key: &str,
    ) -> Result<bool, AppError> {
        // shortcut: Fotos ohne Callback bleiben im Eingang liegen; bei Bedarf Lifecycle-Regel setzen.
        let (bucket, creds) = server_access(cfg);
        let (inbox, _) = inbox_access(cfg);
        if s3::head(&self.http, &inbox, &creds, object_key)
            .await?
            .is_some()
        {
            s3::copy(&self.http, &bucket, &creds, &inbox.name, object_key).await?;
            // Erst nach der Kopie: Scheitert das Löschen, holt der nächste Callback es nach.
            s3::delete(&self.http, &inbox, &creds, object_key).await?;
            return Ok(true);
        }
        Ok(s3::head(&self.http, &bucket, &creds, object_key)
            .await?
            .is_some())
    }

    /// Löscht das Objekt; fehlt es schon, ist das auch erledigt.
    pub async fn delete_object(&self, object_key: &str) -> Result<(), AppError> {
        let cfg = self.storage()?;
        let (bucket, creds) = server_access(&cfg);
        s3::delete(&self.http, &bucket, &creds, object_key).await
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

    /// Speichert das Foto, verteilt seine Auslösung und stößt das Vorschaubild an; ist es fertig,
    /// geht die Auslösung erneut hinaus. Liefert die ID des Fotos.
    pub async fn accept(&self, photo: NewPhoto) -> Result<(i64, PhotoShot), AppError> {
        let id = store::upsert(&self.pool, &photo).await?;
        let shot = self.publish(&photo.shot_key).await?;
        let hub = self.clone();
        let (object_key, shot_key) = (photo.object_key, photo.shot_key);
        tokio::spawn(async move {
            match hub.make_thumbnail(id, &object_key).await {
                Ok(()) => {
                    if let Err(e) = hub.publish(&shot_key).await {
                        tracing::warn!(error = %e, "Auslösung nach dem Vorschaubild nicht lesbar");
                    }
                }
                Err(e) => tracing::warn!(id, error = %e, "kein Vorschaubild"),
            }
        });
        Ok((id, shot))
    }

    async fn publish(&self, shot_key: &str) -> Result<PhotoShot, AppError> {
        let shot = self
            .shot(shot_key)
            .await?
            .ok_or_else(|| AppError::Internal("Foto nach dem Speichern nicht lesbar".into()))?;
        // Ohne Browser gibt es keine Empfänger; das ist kein Fehler.
        let _ = self.shots.send(shot.clone());
        Ok(shot)
    }

    pub fn thumb_path(&self, id: i64) -> PathBuf {
        self.thumbs_dir.join(format!("{id}.jpg"))
    }

    /// Lädt das Original, verkleinert es außerhalb der Async-Threads und legt das Vorschaubild ab.
    async fn make_thumbnail(&self, id: i64, object_key: &str) -> Result<(), AppError> {
        let _turn = self
            .thumb_jobs
            .acquire()
            .await
            .map_err(|_| AppError::Internal("Vorschaubild-Reihe geschlossen".into()))?;
        let cfg = self.storage()?;
        let (bucket, creds) = server_access(&cfg);
        let res = s3::get(&self.http, &bucket, &creds, object_key).await?;
        if res
            .content_length()
            .is_some_and(|n| n > MAX_ORIGINAL_BYTES as u64)
        {
            return Err(AppError::Unprocessable(
                "Foto zu groß für ein Vorschaubild".into(),
            ));
        }
        let original = res
            .bytes()
            .await
            .map_err(|e| AppError::Unavailable(format!("Foto nicht vollständig geladen: {e}")))?;
        if original.len() > MAX_ORIGINAL_BYTES {
            return Err(AppError::Unprocessable(
                "Foto zu groß für ein Vorschaubild".into(),
            ));
        }
        let small = tokio::task::spawn_blocking(move || thumb::make(&original))
            .await
            .map_err(|e| AppError::Internal(format!("Vorschaubild abgebrochen: {e}")))?
            .map_err(AppError::Unprocessable)?;
        tokio::fs::create_dir_all(&self.thumbs_dir)
            .await
            .map_err(|e| AppError::Internal(format!("Ordner der Vorschaubilder: {e}")))?;
        // Erst vollständig schreiben, dann umbenennen: Ein Browser liest nie ein halbes Bild.
        let path = self.thumb_path(id);
        let partial = path.with_extension("part");
        tokio::fs::write(&partial, small)
            .await
            .map_err(|e| AppError::Internal(format!("Vorschaubild nicht geschrieben: {e}")))?;
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(|e| AppError::Internal(format!("Vorschaubild nicht abgelegt: {e}")))?;
        store::set_thumb_ready(&self.pool, id).await?;
        Ok(())
    }

    /// Objektschlüssel und Name eines Fotos für den Download.
    pub async fn original_of(&self, id: i64) -> Result<(String, String), AppError> {
        let key = store::object_key(&self.pool, id)
            .await?
            .ok_or(AppError::NotFound)?;
        let photo = store::get(&self.pool, id)
            .await?
            .ok_or(AppError::NotFound)?;
        Ok((key, photo.name))
    }

    /// Antwort des Speichers mit dem Original als Strom.
    pub async fn original(&self, object_key: &str) -> Result<reqwest::Response, AppError> {
        let cfg = self.storage()?;
        let (bucket, creds) = server_access(&cfg);
        s3::get(&self.http, &bucket, &creds, object_key).await
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

    /// Alle Auslösungen eines Einsatzes, ohne Zeitgrenze.
    pub async fn in_mission(&self, mission: i64) -> Result<Vec<PhotoShot>, AppError> {
        Ok(rules::shots(&store::in_mission(&self.pool, mission).await?))
    }

    /// Auslösungen ohne Einsatz ab `since_ms`, höchstens `RECENT_LIMIT` (die neuesten).
    pub async fn recent_unassigned(&self, since_ms: i64) -> Result<Vec<PhotoShot>, AppError> {
        let rows = store::since_unassigned(&self.pool, since_ms).await?;
        Ok(rules::shots(&rows[..rows.len().min(RECENT_LIMIT as usize)]))
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

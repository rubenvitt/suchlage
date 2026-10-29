//! Admin-API von Garage (v2): kurzlebige Upload-Schlüssel je Fernsteuerung statt STS. Garage kennt
//! kein AssumeRole, aber Schlüssel mit Ablaufzeit; ein abgelaufener Schlüssel gilt dort nicht mehr.
//! Der Aufräumlauf löscht sie danach auch aus der Liste.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::{Mutex, watch};

use crate::error::AppError;
use crate::settings::{PhotoStorageSettings, SettingsHandle};

/// So lange gilt ein neuer Upload-Schlüssel.
pub const KEY_TTL_MS: i64 = 3_600_000;
/// Ein Schlüssel mit weniger Restzeit wird nicht mehr ausgegeben: Ein großer Upload soll nicht
/// mittendrin ablaufen.
pub const KEY_MIN_REMAINING_MS: i64 = 30 * 60_000;
/// Namenspräfix der Upload-Schlüssel; nur diese räumt der Server auf.
pub const KEY_PREFIX: &str = "suchlage-rc-";

#[derive(Debug, Clone, PartialEq)]
pub struct UploadKey {
    pub access_key_id: String,
    pub secret_access_key: String,
    pub expires_at_ms: i64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BucketInfo {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct KeyInfo {
    access_key_id: String,
    secret_access_key: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ListedKey {
    id: String,
    name: String,
    expired: bool,
    /// RFC 3339
    expiration: Option<String>,
}

fn unreachable(e: reqwest::Error) -> AppError {
    tracing::warn!(error = %e, "Admin-API des Fotospeichers nicht erreichbar");
    AppError::Unavailable("Fotospeicher nicht erreichbar".into())
}

async fn call<T: for<'de> Deserialize<'de>>(
    cfg: &PhotoStorageSettings,
    req: reqwest::RequestBuilder,
) -> Result<T, AppError> {
    let res = req
        .bearer_auth(&cfg.admin_token)
        .send()
        .await
        .map_err(unreachable)?;
    let status = res.status();
    if !status.is_success() {
        let detail = res.text().await.unwrap_or_default();
        tracing::warn!(%status, %detail, "Admin-API des Fotospeichers lehnt ab");
        return Err(AppError::Unavailable(format!(
            "Fotospeicher lehnt ab (HTTP {})",
            status.as_u16()
        )));
    }
    res.json()
        .await
        .map_err(|e| AppError::Internal(format!("Antwort der Garage-Admin-API: {e}")))
}

/// Adresse eines Admin-Endpunkts mit kodierten Query-Parametern.
fn endpoint(cfg: &PhotoStorageSettings, path: &str, params: &[(&str, &str)]) -> String {
    let base = format!("{}/v2/{path}", cfg.admin_url);
    if params.is_empty() {
        return base;
    }
    let query: String = url::form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params)
        .finish();
    format!("{base}?{query}")
}

/// Legt einen Schlüssel an, der bis `expires_at_ms` nur in den Eingangs-Bucket schreiben darf.
pub async fn create_upload_key(
    http: &reqwest::Client,
    cfg: &PhotoStorageSettings,
    name: &str,
    expires_at_ms: i64,
) -> Result<UploadKey, AppError> {
    let bucket: BucketInfo = call(
        cfg,
        http.get(endpoint(
            cfg,
            "GetBucketInfo",
            &[("globalAlias", super::inbox_bucket(cfg).as_str())],
        )),
    )
    .await?;
    let expiration = DateTime::<Utc>::from_timestamp_millis(expires_at_ms)
        .ok_or_else(|| AppError::Internal("Ablaufzeit außerhalb des Bereichs".into()))?;
    let key: KeyInfo = call(
        cfg,
        http.post(endpoint(cfg, "CreateKey", &[]))
            .json(&json!({ "name": name, "expiration": expiration.to_rfc3339() })),
    )
    .await?;
    let secret = key.secret_access_key.ok_or_else(|| {
        AppError::Internal("Garage hat zum neuen Schlüssel kein Geheimnis geliefert".into())
    })?;
    let _: serde_json::Value = call(
        cfg,
        http.post(endpoint(cfg, "AllowBucketKey", &[]))
            .json(&json!({
                "bucketId": bucket.id,
                "accessKeyId": key.access_key_id,
                "permissions": { "read": false, "write": true, "owner": false },
            })),
    )
    .await?;
    Ok(UploadKey {
        access_key_id: key.access_key_id,
        secret_access_key: secret,
        expires_at_ms,
    })
}

/// Löscht abgelaufene Upload-Schlüssel (nur mit Präfix `suchlage-rc-`). Liefert die Anzahl.
pub async fn delete_expired_keys(
    http: &reqwest::Client,
    cfg: &PhotoStorageSettings,
    now_ms: i64,
) -> Result<usize, AppError> {
    let keys: Vec<ListedKey> = call(cfg, http.get(endpoint(cfg, "ListKeys", &[]))).await?;
    let mut deleted = 0;
    for key in keys.iter().filter(|k| {
        k.name.starts_with(KEY_PREFIX)
            && (k.expired
                || k.expiration.as_deref().is_some_and(|e| {
                    DateTime::parse_from_rfc3339(e).is_ok_and(|e| e.timestamp_millis() <= now_ms)
                }))
    }) {
        let res = http
            .post(endpoint(cfg, "DeleteKey", &[("id", key.id.as_str())]))
            .bearer_auth(&cfg.admin_token)
            .send()
            .await
            .map_err(unreachable)?;
        if res.status().is_success() {
            deleted += 1;
        } else {
            tracing::warn!(status = %res.status(), key = %key.id, "Upload-Schlüssel nicht gelöscht");
        }
    }
    Ok(deleted)
}

/// Abstand der Aufräumläufe; der erste läuft beim Start (Schlüssel aus der Zeit vor einem Neustart).
const CLEANUP_EVERY: Duration = Duration::from_secs(15 * 60);

/// Räumt abgelaufene Upload-Schlüssel auf, bis der Server stoppt. Ohne Fotospeicher wartet sie.
pub async fn cleanup_loop(
    http: reqwest::Client,
    settings: SettingsHandle,
    mut stop: watch::Receiver<bool>,
) {
    loop {
        if let Some(cfg) = settings.current().photo_storage {
            match delete_expired_keys(&http, &cfg, crate::time::now_ms()).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(n, "abgelaufene Upload-Schlüssel gelöscht"),
                Err(e) => tracing::warn!(error = %e, "Upload-Schlüssel nicht aufgeräumt"),
            }
        }
        tokio::select! {
            () = tokio::time::sleep(CLEANUP_EVERY) => {}
            _ = stop.changed() => return,
        }
    }
}

/// Upload-Schlüssel je Fernsteuerung. Pilot 2 fragt `sts` vor jedem Upload; so entsteht nicht für
/// jedes Foto ein neuer Schlüssel.
#[derive(Clone, Default)]
pub struct UploadKeys {
    cache: Arc<Mutex<HashMap<String, UploadKey>>>,
}

impl UploadKeys {
    /// Gültiger Schlüssel für die Fernsteuerung, bei Bedarf neu angelegt. Der Cache gilt je
    /// Speicher: Neue Einstellungen erzeugen neue Schlüssel.
    pub async fn for_controller(
        &self,
        http: &reqwest::Client,
        cfg: &PhotoStorageSettings,
        rc_sn: &str,
        now_ms: i64,
    ) -> Result<UploadKey, AppError> {
        let slot = format!("{}|{}|{rc_sn}", cfg.admin_url, cfg.bucket);
        // Die Sperre gilt über das Anlegen hinweg: Zwei gleichzeitige Anfragen derselben
        // Fernsteuerung sollen nicht zwei Schlüssel erzeugen.
        let mut cache = self.cache.lock().await;
        if let Some(key) = cache.get(&slot)
            && key.expires_at_ms - now_ms >= KEY_MIN_REMAINING_MS
        {
            return Ok(key.clone());
        }
        let name = format!("{KEY_PREFIX}{rc_sn}-{now_ms}");
        let key = create_upload_key(http, cfg, &name, now_ms + KEY_TTL_MS).await?;
        cache.insert(slot, key.clone());
        Ok(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::fake;

    fn cfg(admin_url: &str, token: &str) -> PhotoStorageSettings {
        PhotoStorageSettings {
            admin_token: token.into(),
            ..fake::settings(admin_url)
        }
    }

    #[tokio::test]
    async fn a_controller_gets_one_write_only_key_until_it_runs_low() {
        let (url, seen) = fake::start().await;
        let cfg = cfg(&url, fake::TOKEN);
        let keys = UploadKeys::default();
        let http = reqwest::Client::new();
        let t0 = 1_792_150_000_000;
        let a = keys.for_controller(&http, &cfg, "RC1", t0).await.unwrap();
        assert_eq!(a.access_key_id, "GK1");
        assert_eq!(a.secret_access_key, "GK1-geheim");
        assert_eq!(a.expires_at_ms, t0 + KEY_TTL_MS);
        let again = keys
            .for_controller(&http, &cfg, "RC1", t0 + 29 * 60_000)
            .await
            .unwrap();
        assert_eq!(again, a, "noch über 30 min Restzeit");
        let other = keys.for_controller(&http, &cfg, "RC2", t0).await.unwrap();
        assert_eq!(other.access_key_id, "GK2", "je Fernsteuerung ein Schlüssel");
        let renewed = keys
            .for_controller(&http, &cfg, "RC1", t0 + 31 * 60_000)
            .await
            .unwrap();
        assert_eq!(renewed.access_key_id, "GK3");

        let s = seen.lock().unwrap();
        assert_eq!(s.keys[0].1, format!("suchlage-rc-RC1-{t0}"));
        assert_eq!(
            s.keys[0].2.as_deref(),
            Some("2026-10-16T12:26:40+00:00"),
            "Ablauf nach einer Stunde"
        );
        assert_eq!(
            s.grants[0],
            json!({"bucketId": "b2", "accessKeyId": "GK1", "permissions": {"read": false, "write": true, "owner": false}})
        );
    }

    #[tokio::test]
    async fn cleanup_deletes_only_our_expired_keys() {
        let (url, seen) = fake::start().await;
        let cfg = cfg(&url, fake::TOKEN);
        let http = reqwest::Client::new();
        let t0 = 1_792_150_000_000;
        create_upload_key(&http, &cfg, "suchlage-rc-RC1-1", t0)
            .await
            .unwrap();
        create_upload_key(&http, &cfg, "suchlage-rc-RC1-2", t0 + KEY_TTL_MS)
            .await
            .unwrap();
        assert_eq!(delete_expired_keys(&http, &cfg, t0).await.unwrap(), 1);
        assert_eq!(seen.lock().unwrap().deleted, ["GK1"], "fremde bleiben");
        assert_eq!(delete_expired_keys(&http, &cfg, t0).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn a_wrong_token_or_bucket_is_unavailable() {
        let (url, _) = fake::start().await;
        let http = reqwest::Client::new();
        let err = create_upload_key(&http, &cfg(&url, "falsch"), "x", 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("HTTP 403"), "{err}");
        let mut other = cfg(&url, fake::TOKEN);
        other.bucket = "gibt-es-nicht".into();
        let err = create_upload_key(&http, &other, "x", 0).await.unwrap_err();
        assert!(err.to_string().contains("HTTP 404"), "{err}");
        let err = create_upload_key(&http, &cfg("http://127.0.0.1:9", "t"), "x", 0)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unavailable(_)), "{err:?}");
    }
}

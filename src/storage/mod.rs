//! Objektspeicher der Fotos (Garage, M3): S3 mit eigener Signatur und die Admin-API für
//! kurzlebige Upload-Schlüssel.

pub mod garage;
pub mod s3;

#[cfg(test)]
pub(crate) mod fake;

use crate::settings::PhotoStorageSettings;

/// Eingangs-Bucket neben dem Fotospeicher (ADR 0004). Garage kennt nur Rechte je Bucket, und
/// `write` umfasst dort auch Löschen und Überschreiben: Fernsteuerungen schreiben deshalb nur hierher,
/// der Server übernimmt jedes Foto beim Upload-Callback in den eigentlichen Bucket.
pub fn inbox_bucket(cfg: &PhotoStorageSettings) -> String {
    format!("{}-eingang", cfg.bucket)
}

/// Bucket und Schlüssel des Servers aus den Einstellungen.
pub fn server_access(cfg: &PhotoStorageSettings) -> (s3::Bucket, s3::Credentials) {
    (
        s3::Bucket {
            endpoint: cfg.s3_url.clone(),
            name: cfg.bucket.clone(),
            region: cfg.region.clone(),
        },
        s3::Credentials {
            access_key_id: cfg.access_key_id.clone(),
            secret_access_key: cfg.secret_access_key.clone(),
        },
    )
}

/// Eingangs-Bucket mit dem Schlüssel des Servers (lesen, schreiben, löschen).
pub fn inbox_access(cfg: &PhotoStorageSettings) -> (s3::Bucket, s3::Credentials) {
    let (mut bucket, creds) = server_access(cfg);
    bucket.name = inbox_bucket(cfg);
    (bucket, creds)
}

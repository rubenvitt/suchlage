use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::live::GeoPoint;

/// Linse eines Fotos, aus dem DJI-Dateinamen (`_T`, `_W`, `_Z`, `_V`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum PhotoLens {
    Thermal,
    Wide,
    Zoom,
    Visible,
    Unknown,
}

/// Eine Datei einer Auslösung.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Photo {
    #[ts(type = "number")]
    pub id: i64,
    /// Dateiname aus Pilot 2, z. B. `DJI_20261017143012_0001_T.JPG`.
    pub name: String,
    pub lens: PhotoLens,
    /// `null`, solange das Vorschaubild noch entsteht oder nicht erzeugt werden konnte.
    pub thumb_url: Option<String>,
    pub original_url: String,
}

/// Eine Auslösung: alle Dateien, die zur selben Zeit mit derselben Kamera entstanden sind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PhotoShot {
    /// Fernsteuerung und Dateiname ohne Linsen-Endung, z. B. `RC1/DJI_20261017143012_0001`.
    pub id: String,
    #[ts(type = "number | null")]
    pub flight_id: Option<i64>,
    pub gateway_sn: String,
    pub aircraft_sn: Option<String>,
    /// Aufnahmezeit, sonst die Zeit des Uploads.
    #[ts(type = "number")]
    pub taken_at_ms: i64,
    /// `null`: Pilot 2 hat keine gültige Position geschickt; das Foto fehlt dann auf der Karte.
    pub position: Option<GeoPoint>,
    /// Höhe über dem Ellipsoid laut Pilot 2 (DJI `absolute_altitude`).
    pub absolute_altitude_m: Option<f64>,
    /// Höhe über dem Startpunkt laut Pilot 2 (DJI `relative_altitude`).
    pub relative_altitude_m: Option<f64>,
    pub gimbal_yaw_deg: Option<f64>,
    /// `null`: ohne Einsatz.
    #[ts(type = "number | null")]
    pub mission_id: Option<i64>,
    /// Wärmebild zuerst, dann Weit, Zoom, sichtbar, unbekannt.
    pub photos: Vec<Photo>,
}

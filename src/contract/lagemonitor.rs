//! Schnittstelle für den Lagemonitor im FüKw (`/api/lagemonitor/v1`). Nur lesen, angemeldet mit
//! einem Großbild-Token als `Authorization: Bearer …`. Felder `snake_case`, Zeiten in ms seit Epoch.
//!
//! Der Lagemonitor zeigt nur den laufenden Einsatz: Ohne ihn ist `mission` `null` und alles andere
//! leer, auch wenn Drohnen fliegen.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::flugmappe::FmFeatureCollection;
use super::live::GeoPoint;

/// Version der Schnittstelle, steht im Pfad (`/v1`) und in [`LmLage`].
pub const LAGEMONITOR_API_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LmMission {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    #[ts(type = "number | null")]
    pub started_at_ms: Option<i64>,
    /// Bezeichnung des aktiven (jüngsten) Abschnitts.
    pub section: Option<String>,
}

/// Eine Drohne mit Position, deren Fernsteuerung gerade Daten liefert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LmAircraft {
    pub gateway_sn: String,
    /// Bezeichnung der Fernsteuerung in Suchlage, sonst `null`.
    pub label: Option<String>,
    pub position: GeoPoint,
    pub heading_deg: Option<f64>,
    /// Höhe über dem Startpunkt.
    pub elevation_takeoff_m: Option<f64>,
    pub battery_percent: Option<u8>,
    pub airborne: bool,
    #[ts(type = "number | null")]
    pub flight_id: Option<i64>,
}

/// Ein laufendes Livebild. Abspielen über `/video/{gateway_sn}/whep` bzw. `/hls/index.m3u8`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LmVideo {
    pub gateway_sn: String,
    pub label: Option<String>,
}

/// `GET /lage`: alles, was der Lagemonitor vom laufenden Einsatz zeigt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LmLage {
    pub api_version: u32,
    #[ts(type = "number")]
    pub server_time_ms: i64,
    pub mission: Option<LmMission>,
    /// Kartenelemente und Flugspuren des Einsatzes, wie für die Flugmappe.
    pub map: FmFeatureCollection,
    pub aircraft: Vec<LmAircraft>,
    pub videos: Vec<LmVideo>,
}

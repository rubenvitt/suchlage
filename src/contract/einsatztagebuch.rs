//! Schnittstelle für das Einsatztagebuch der IUK-Suite (`/api/einsatztagebuch/v1`), angemeldet mit
//! einem Großbild-Token als `Authorization: Bearer …`. Felder `snake_case`, Zeiten in ms seit Epoch.
//!
//! Das Einsatztagebuch wird immer nach dem Einsatz ausgefüllt. Deshalb nennt die Liste neben dem
//! laufenden Einsatz auch die kürzlich abgeschlossenen; vorbereitete fehlen, sie liefen nie.
//!
//! `POST /einsaetze/{id}/verknuepfung` (ohne Körper, 204) meldet einen versiegelten Eintrag, der
//! den Einsatz nennt. Der Einsatz fehlt danach in der Liste und lässt sich nicht mehr löschen.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::missions::MissionState;

/// Version der Schnittstelle, steht im Pfad (`/v1`) und in [`EtbMissions`].
pub const EINSATZTAGEBUCH_API_VERSION: u32 = 1;
/// Ohne `?tage=`: abgeschlossene Einsätze der letzten 7 Tage.
pub const EINSATZTAGEBUCH_DEFAULT_DAYS: u32 = 7;
/// Höchstens so weit zurück.
pub const EINSATZTAGEBUCH_MAX_DAYS: u32 = 31;

/// Ein Einsatz, wie das Einsatztagebuch ihn zur Auswahl zeigt und als Schnappschuss versiegelt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EtbMission {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    /// `running` oder `closed`.
    pub state: MissionState,
    #[ts(type = "number | null")]
    pub started_at_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub closed_at_ms: Option<i64>,
}

/// `GET /einsaetze`: laufender Einsatz zuerst, dann die abgeschlossenen, zuletzt beendete zuerst.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct EtbMissions {
    pub api_version: u32,
    #[ts(type = "number")]
    pub server_time_ms: i64,
    pub missions: Vec<EtbMission>,
}

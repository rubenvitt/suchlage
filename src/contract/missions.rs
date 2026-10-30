use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MissionState {
    Prepared,
    Running,
    Closed,
}

/// Wo ein Einsatz angelegt wurde: im Browser oder in der Flugmappe.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MissionOrigin {
    Web,
    Flugmappe,
}

/// Verknüpfung eines Einsatzes mit einer Flugmappe (SUL-113: mehrere je Einsatz). Die
/// Einsatz-ID der Flugmappe braucht der Browser nicht.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlugmappeLink {
    /// ID der Verknüpfung in Suchlage (für „Lösen“).
    #[ts(type = "number")]
    pub id: i64,
    /// Fluggerät der Flugmappe, z. B. „Matrice 200“.
    pub aircraft_label: String,
    /// Anzeigename des Kontos, das verknüpft hat.
    pub linked_by: String,
    #[ts(type = "number")]
    pub linked_at_ms: i64,
    /// Die Flugmappe hat ihren Teil abgeschlossen.
    #[ts(type = "number | null")]
    pub done_at_ms: Option<i64>,
    /// In Suchlage gelöst; die Flugmappe darf nichts mehr am Einsatz.
    #[ts(type = "number | null")]
    pub released_at_ms: Option<i64>,
}

impl FlugmappeLink {
    /// Nicht gelöst: Die Flugmappe arbeitet (noch) am Einsatz.
    pub fn is_active(&self) -> bool {
        self.released_at_ms.is_none()
    }
}

/// Ein Abschnitt eines Einsatzes (z. B. ein Suchgebiet).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionSection {
    #[ts(type = "number")]
    pub id: i64,
    pub label: String,
    pub location: Option<String>,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    /// Ein Foto der Papierkarte liegt (aus der Flugmappe).
    pub has_map_photo: bool,
    /// Zeitpunkt des letzten Kartenfotos; taugt als Versionszeichen für die Bild-URL.
    #[ts(type = "number | null")]
    pub map_photo_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Mission {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    pub state: MissionState,
    pub created_by: String,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    #[ts(type = "number | null")]
    pub started_at_ms: Option<i64>,
    #[ts(type = "number | null")]
    pub closed_at_ms: Option<i64>,
    /// Älteste zuerst; der letzte Abschnitt ist der aktive.
    pub sections: Vec<MissionSection>,
    pub origin: MissionOrigin,
    /// Verknüpfte Flugmappen, älteste zuerst, auch gelöste. Ohne nicht gelöste legt Suchlage
    /// auch Abschnitte an.
    pub flugmappe_links: Vec<FlugmappeLink>,
}

impl Mission {
    /// Mindestens eine Flugmappe arbeitet am Einsatz (nicht gelöst).
    pub fn has_active_flugmappe(&self) -> bool {
        self.flugmappe_links.iter().any(FlugmappeLink::is_active)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionCreate {
    pub name: String,
    pub location: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SectionCreate {
    pub label: String,
    pub location: Option<String>,
}

/// Ordnet einen Flug oder ein Kartenelement „ohne Einsatz“ nachträglich zu.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionAssign {
    #[ts(type = "number")]
    pub mission_id: i64,
}

/// Name und Ort eines Einsatzes ändern.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionUpdate {
    pub name: String,
    pub location: Option<String>,
}

/// Was an einem Einsatz hängt (Anzahl Flüge, Fotodateien, Kartenelemente, Videoabschnitte).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionContents {
    pub flights: u32,
    pub photos: u32,
    pub map_elements: u32,
    pub video_segments: u32,
}

/// Woher ein Eintrag der Flugliste eines Einsatzes kommt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MissionFlightSource {
    /// Aufgezeichnet über Pilot 2 (Spur, Video, Fotos).
    Telemetry,
    /// Von Hand im Flugbuch einer Flugmappe eingetragen, ohne Telemetrie.
    Flugmappe,
}

/// Eine Zeile der Flugliste eines Einsatzes (`GET /api/missions/{id}/flight-log`): Telemetrie-
/// Flüge und Handeinträge der Flugmappen, nach Start sortiert.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MissionFlight {
    /// Eindeutig in der Liste: `t<Flug>` oder `f<Eintrag>`.
    pub key: String,
    /// Telemetrie-Flug mit Detailseite; `null` bei Handeinträgen.
    #[ts(type = "number | null")]
    pub flight_id: Option<i64>,
    pub source: MissionFlightSource,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    /// `null`: in der Luft.
    #[ts(type = "number | null")]
    pub ended_at_ms: Option<i64>,
    /// Bezeichnung aus der Flugmappe, sonst die der Fernsteuerung, sonst die Seriennummer.
    pub aircraft: String,
    /// Bezeichnung des Abschnitts.
    pub section: Option<String>,
    /// Fluggerät der Flugmappe, in der ein Telemetrie-Flug bestätigt ist; sonst `null`.
    pub confirmed_in: Option<String>,
    /// Fernpilot aus der Flugmappe.
    pub pilot: Option<String>,
}

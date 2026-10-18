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

/// Verknüpfung eines Einsatzes mit einer Flugmappe. Die Einsatz-ID der Flugmappe braucht der
/// Browser nicht.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlugmappeLink {
    /// Anzeigename des Kontos, das verknüpft (oder übernommen) hat.
    pub linked_by: String,
    #[ts(type = "number")]
    pub linked_at_ms: i64,
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
    /// `null`: mit keiner Flugmappe verknüpft. Dann legt Suchlage auch Abschnitte an.
    pub flugmappe: Option<FlugmappeLink>,
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

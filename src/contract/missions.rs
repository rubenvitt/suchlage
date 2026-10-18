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

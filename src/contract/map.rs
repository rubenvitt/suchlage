use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::live::GeoPoint;

/// Die Formen, die Pilot 2 kennt (`resource.type` 0, 1, 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MapElementKind {
    Point,
    Line,
    Polygon,
}

/// Wo das Element entstanden ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum MapElementOrigin {
    Pilot,
    Web,
}

/// WGS84. Die Höhe kommt nur aus Pilot 2 und wird unverändert zurückgegeben.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapPoint {
    pub lat: f64,
    pub lon: f64,
    pub alt_m: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapElement {
    pub id: String,
    pub name: String,
    pub kind: MapElementKind,
    /// Ein Punkt, eine Linie ab zwei Punkten, eine Fläche ab drei Punkten ohne wiederholten Schlusspunkt.
    pub points: Vec<MapPoint>,
    /// `#RRGGBB`.
    pub color: String,
    pub clamp_to_ground: bool,
    /// Bezeichnung der Fernsteuerung oder Name des Kontos.
    pub author: String,
    pub origin: MapElementOrigin,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    #[ts(type = "number")]
    pub updated_at_ms: i64,
}

/// Neues Element aus dem Browser. Die ID vergibt der Server.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapElementInput {
    pub name: String,
    pub kind: MapElementKind,
    pub points: Vec<GeoPoint>,
    /// Eine der `MAP_COLORS`.
    pub color: String,
}

/// Umbenennen und Umfärben im Browser. Die Form ändert nur Pilot 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MapElementUpdate {
    pub name: String,
    pub color: String,
}

/// Die Farbtabelle der DJI-Doku (`10.create.md`). Die Doku benennt `#B620E0` als ORANGE und
/// `#212121` als PURPLE; die Oberfläche nennt die Farben nach ihrem Aussehen.
pub const MAP_COLORS: [&str; 6] = [
    "#2D8CF0", "#19BE6B", "#FFBB00", "#E23C39", "#B620E0", "#212121",
];

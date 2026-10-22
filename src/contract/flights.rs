use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct TrackPoint {
    #[ts(type = "number")]
    pub t_ms: i64,
    pub lat: f64,
    pub lon: f64,
    /// Höhe über Startpunkt (DJI `elevation`), nicht über Grund.
    pub elevation_m: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FlightEndReason {
    Landed,
    ConnectionLost,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlightSummary {
    #[ts(type = "number")]
    pub id: i64,
    pub aircraft_sn: String,
    pub gateway_sn: String,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    #[ts(type = "number | null")]
    pub ended_at_ms: Option<i64>,
    pub end_reason: Option<FlightEndReason>,
    pub max_elevation_m: Option<f64>,
    pub distance_m: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlightVideo {
    pub play_url: String,
    pub download_url: String,
    /// Zeitpunkt der ersten Sekunde im Video; damit springt ein Klick auf die Spur an die passende Stelle.
    #[ts(type = "number")]
    pub started_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlightDetail {
    pub flight: FlightSummary,
    pub segments: Vec<Vec<TrackPoint>>,
    pub video: Option<FlightVideo>,
}

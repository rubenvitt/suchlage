use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::flights::{FlightSummary, TrackPoint};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GeoPoint {
    pub lat: f64,
    pub lon: f64,
}

/// DJI `video_type`; die Werte sind die DJI-Schreibweise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "lowercase")]
#[ts(export)]
pub enum Lens {
    Normal,
    Wide,
    Zoom,
    Thermal,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct VideoSource {
    /// Unverändert aus `live_capacity` – nie selbst zusammensetzen.
    pub video_id: String,
    pub camera_index: String,
    /// „H20N“, „FPV“ …
    pub camera_label: String,
    pub lens: Lens,
    pub switchable_lenses: Vec<Lens>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct LinkQuality {
    /// 0–5, Bildübertragung RC ↔ Fluggerät.
    pub sdr_quality: Option<u8>,
    /// 0–5, Mobilfunk der RC.
    pub lte_quality: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GatewayState {
    pub sn: String,
    /// Name aus der Verwaltung („RC Plus 1“), sonst `null`.
    pub label: Option<String>,
    #[ts(type = "number")]
    pub last_seen_ms: i64,
    /// `null`: Fluggerät aus oder Funkverbindung weg (`update_topo` mit leerem `sub_devices`).
    pub aircraft_sn: Option<String>,
    pub position: Option<GeoPoint>,
    pub battery_percent: Option<u8>,
    pub link: LinkQuality,
    pub video_sources: Vec<VideoSource>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FlightPhase {
    Ground,
    Airborne,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RtkFix {
    NotStarted,
    Fixing,
    Fixed,
    Failed,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Battery {
    pub percent: Option<u8>,
    pub remaining_flight_s: Option<u32>,
    pub return_home_percent: Option<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Gnss {
    pub fix: Option<RtkFix>,
    pub quality: Option<u8>,
    pub gps_satellites: Option<u8>,
    pub rtk_satellites: Option<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlightMode {
    pub code: u8,
    /// Deutsche Bezeichnung („manueller Flug“).
    pub label: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Telemetry {
    pub position: Option<GeoPoint>,
    /// Ellipsoidische Höhe (DJI `height`), nicht MSL.
    pub height_ellipsoid_m: Option<f64>,
    /// Höhe über Startpunkt (DJI `elevation`), nicht über Grund.
    pub elevation_takeoff_m: Option<f64>,
    pub home_distance_m: Option<f64>,
    pub horizontal_speed_mps: Option<f64>,
    pub vertical_speed_mps: Option<f64>,
    /// Kompasskurs 0–360°.
    pub heading_deg: Option<f64>,
    pub battery: Option<Battery>,
    pub gnss: Option<Gnss>,
    pub mode: Option<FlightMode>,
    pub home: Option<GeoPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AircraftState {
    pub sn: String,
    pub gateway_sn: String,
    #[ts(type = "number")]
    pub last_seen_ms: i64,
    pub phase: FlightPhase,
    #[ts(type = "number | null")]
    pub flight_id: Option<i64>,
    pub telemetry: Telemetry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum StreamPhase {
    Idle,
    Starting,
    Live,
    Stopping,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamState {
    pub gateway_sn: String,
    pub phase: StreamPhase,
    pub video_id: Option<String>,
    pub lens: Option<Lens>,
    /// Deutscher Satz für die Oberfläche, nur bei `failed`.
    pub error: Option<String>,
    #[ts(type = "number")]
    pub updated_at_ms: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum BrokerStatus {
    NotConfigured,
    Connecting,
    Connected,
    Disconnected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct UnknownFieldCount {
    /// `aircraft.<feld>` oder `gateway.<feld>`.
    pub key: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SystemStatus {
    pub broker: BrokerStatus,
    pub unknown_fields: Vec<UnknownFieldCount>,
    pub invalid_messages: u32,
    /// Dateiname des laufenden Rohmitschnitts, `null` wenn aus.
    pub raw_recording: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FreshnessThresholds {
    #[ts(type = "number")]
    pub delayed_after_ms: i64,
    #[ts(type = "number")]
    pub lost_after_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ActiveFlight {
    pub flight: FlightSummary,
    pub segments: Vec<Vec<TrackPoint>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SituationSnapshot {
    #[ts(type = "number")]
    pub server_time_ms: i64,
    pub freshness: FreshnessThresholds,
    pub gateways: Vec<GatewayState>,
    pub aircraft: Vec<AircraftState>,
    pub active_flights: Vec<ActiveFlight>,
    pub streams: Vec<StreamState>,
    pub system: SystemStatus,
}

/// Eine SSE-Nachricht. Das Feld `type` unterscheidet die Varianten.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum LiveEvent {
    Snapshot {
        snapshot: SituationSnapshot,
    },
    Gateway {
        gateway: GatewayState,
    },
    Aircraft {
        aircraft: AircraftState,
    },
    TrackPoint {
        #[ts(type = "number")]
        flight_id: i64,
        segment: u32,
        point: TrackPoint,
    },
    FlightStarted {
        flight: FlightSummary,
    },
    FlightEnded {
        flight: FlightSummary,
    },
    Stream {
        stream: StreamState,
    },
    System {
        system: SystemStatus,
    },
}

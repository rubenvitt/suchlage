//! Was der Kern über die Welt erfährt – in eigenen Begriffen, ohne DJI-Typen.

use crate::contract::{BrokerStatus, GeoPoint, Lens, LinkQuality, Telemetry, VideoSource};

#[derive(Debug, Clone, PartialEq)]
pub struct Observed {
    /// Empfangszeit in ms seit Epoch.
    pub at_ms: i64,
    pub observation: Observation,
}

/// Flugzustand nach der Einteilung des Adapters (Referenz Kap. 4): Boden, Luft, unklar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeClass {
    Ground,
    Air,
    Unknown,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LiveStreamInfo {
    pub video_id: String,
    pub lens: Option<Lens>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamCommand {
    Start { video_id: String },
    Stop,
    ChangeLens { lens: Lens },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Observation {
    /// Topologie der Fernsteuerung; `aircraft_sn: None` heißt Fluggerät aus oder Funk weg.
    Topology {
        gateway_sn: String,
        aircraft_sn: Option<String>,
    },
    GatewayTelemetry {
        gateway_sn: String,
        position: Option<GeoPoint>,
        battery_percent: Option<u8>,
        link: LinkQuality,
    },
    /// `periodic`: aus dem periodischen Strom (osd) – nur solche Proben zählen für Phase und Spur.
    AircraftTelemetry {
        aircraft_sn: String,
        gateway_sn: String,
        telemetry: Telemetry,
        mode_class: ModeClass,
        periodic: bool,
    },
    VideoSources {
        gateway_sn: String,
        sources: Vec<VideoSource>,
    },
    /// Aktueller Stream laut Gerät; `live: None`: kein Stream läuft. `error`: deutscher Satz, wenn
    /// das Gerät in `live_status.error_status` einen Fehler meldet.
    StreamStatus {
        gateway_sn: String,
        live: Option<LiveStreamInfo>,
        error: Option<String>,
    },
    CommandIssued {
        gateway_sn: String,
        command: StreamCommand,
    },
    CommandFinished {
        gateway_sn: String,
        command: StreamCommand,
        result: Result<(), String>,
    },
    Broker(BrokerStatus),
    Diagnostics {
        unknown_fields: Vec<String>,
        invalid_messages: u32,
    },
    RawRecording(Option<String>),
    /// Namen der Fernsteuerungen aus der Verwaltung, vollständige Liste (SN, Name).
    Labels(Vec<(String, String)>),
}

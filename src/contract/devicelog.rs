//! Funkprotokoll: was zwischen Server, Broker und den Fernsteuerungen läuft, als Strom für die
//! Systemseite. Payloads kommen ohne Geheimnisse und gekürzt an.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceLogSource {
    /// MQTT von der Fernsteuerung oder dem Fluggerät an den Server.
    Device,
    /// MQTT vom Server an die Fernsteuerung.
    Server,
    /// Protokollzeile der Pilot-Seite in Pilot 2 (JSBridge).
    Pilot,
    /// Was der Server selbst feststellt: Broker, Zeitüberschreitungen.
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceLogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DeviceLogEntry {
    /// Laufende Nummer seit Serverstart; Lücken heißen „nicht mitbekommen“.
    #[ts(type = "number")]
    pub seq: u64,
    #[ts(type = "number")]
    pub at_ms: i64,
    pub source: DeviceLogSource,
    pub level: DeviceLogLevel,
    /// Seriennummer aus dem Topic (RC oder Fluggerät), bei der Pilot-Seite die gekoppelte RC.
    pub device_sn: Option<String>,
    pub topic: Option<String>,
    /// DJI-Methode (`live_start_push`, `update_topo` …), falls die Nachricht eine trägt.
    pub method: Option<String>,
    /// Deutscher Satz für die Liste.
    pub summary: String,
    /// Payload ohne Geheimnisse, ab 4 KB gekürzt.
    pub payload: Option<String>,
}

/// Eine SSE-Nachricht von `/api/device-log`. Zuerst `history`, dann einzelne `entry`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
#[ts(export)]
pub enum DeviceLogEvent {
    History {
        entries: Vec<DeviceLogEntry>,
    },
    Entry {
        entry: DeviceLogEntry,
    },
    /// Der Strom kam nicht hinterher; so viele Einträge fehlen.
    Missed {
        #[ts(type = "number")]
        count: u64,
    },
}

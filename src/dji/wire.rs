//! DJI-Nachrichten (Doku v1.11.3, Referenz `docs/reference/dji-cloud-api-m350.md`).
//! osd/state werden NICHT als feste Structs gelesen: sie kommen als Teil-Updates und mit
//! wechselnden Zahltypen. Der Übersetzer führt sie als JSON zusammen und liest Felder tolerant
//! über [`number`]/[`int`]. Feste Structs nur für Nachrichten mit stabiler Form.

use serde::Deserialize;
use serde_json::{Map, Value};

/// Fluggerät M350 RTK (`type`), RC Plus (`type`); `sub_type` jeweils 0.
pub const AIRCRAFT_TYPE: u32 = 89;
pub const GATEWAY_TYPE: u32 = 119;

#[derive(Debug, Clone, Deserialize)]
pub struct Envelope {
    pub tid: Option<String>,
    pub bid: Option<String>,
    pub timestamp: Option<i64>,
    pub method: Option<String>,
    pub gateway: Option<String>,
    pub need_reply: Option<u8>,
    #[serde(default)]
    pub data: Value,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct UpdateTopo {
    #[serde(rename = "type")]
    pub kind: Option<u32>,
    pub sub_type: Option<u32>,
    #[serde(default)]
    pub sub_devices: Vec<SubDevice>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct SubDevice {
    pub sn: String,
    #[serde(rename = "type")]
    pub kind: Option<u32>,
    pub sub_type: Option<u32>,
    pub index: Option<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Default)]
pub struct LiveCapacity {
    #[serde(default)]
    pub device_list: Vec<LiveDevice>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LiveDevice {
    pub sn: String,
    #[serde(default)]
    pub camera_list: Vec<LiveCamera>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LiveCamera {
    pub camera_index: String,
    #[serde(default)]
    pub video_list: Vec<LiveVideo>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LiveVideo {
    pub video_index: String,
    pub video_type: Option<String>,
    #[serde(default)]
    pub switchable_video_types: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct LiveStatusEntry {
    pub video_id: String,
    pub video_type: Option<String>,
    pub video_quality: Option<u8>,
    pub status: Option<u8>,
    pub error_status: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct ServiceReply {
    pub result: i64,
    pub output: Option<Value>,
}

/// Dokumentierte Felder der Fluggeräte-osd/state (Referenz Kap. 3). Alles andere zählt als unbekannt.
pub const AIRCRAFT_DOCUMENTED: &[&str] = &[
    "latitude",
    "longitude",
    "height",
    "elevation",
    "home_distance",
    "horizontal_speed",
    "vertical_speed",
    "attitude_head",
    "attitude_pitch",
    "attitude_roll",
    "wind_speed",
    "wind_direction",
    "battery",
    "low_battery_warning_threshold",
    "serious_low_battery_warning_threshold",
    "mode_code",
    "mode_code_reason",
    "home_latitude",
    "home_longitude",
    "position_state",
    "gear",
    "control_source",
    "track_id",
    "height_limit",
    "is_near_height_limit",
    "is_near_area_limit",
    "obstacle_avoidance",
    "night_lights_state",
    "total_flight_time",
    "total_flight_distance",
    "total_flight_sorties",
    "storage",
    "firmware_version",
    "country",
    "cameras",
];

/// Dokumentierte Felder der RC-osd/state (Referenz Kap. 3.11).
pub const GATEWAY_DOCUMENTED: &[&str] = &[
    "latitude",
    "longitude",
    "height",
    "capacity_percent",
    "wireless_link",
    "live_capacity",
    "live_status",
    "dongle_infos",
    "firmware_version",
];

/// Nutzlast-Structs heißen `{type}-{subtype}-{gimbalindex}`, z. B. `61-0-0`.
pub fn is_payload_key(key: &str) -> bool {
    let parts: Vec<_> = key.split('-').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

/// Ein Feld ist vorhanden, trägt aber keinen brauchbaren Zahlenwert (Text, Bruch, außer Bereich).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("Feld ist kein gültiger Zahlenwert")]
pub struct InvalidNumber;

/// Zahlenfeld tolerant lesen. `Ok(None)`: fehlt oder `null`. `Err(InvalidNumber)`: vorhanden, aber keine Zahl.
pub fn number(obj: &Map<String, Value>, key: &str) -> Result<Option<f64>, InvalidNumber> {
    match obj.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(n)) => n
            .as_f64()
            .filter(|f| f.is_finite())
            .map(Some)
            .ok_or(InvalidNumber),
        Some(_) => Err(InvalidNumber),
    }
}

/// Ganzzahl im Bereich von `T`; Kommazahlen mit Nachkommaanteil 0 werden angenommen.
pub fn int<T: TryFrom<i64>>(
    obj: &Map<String, Value>,
    key: &str,
) -> Result<Option<T>, InvalidNumber> {
    let Some(f) = number(obj, key)? else {
        return Ok(None);
    };
    if f.fract() != 0.0 {
        return Err(InvalidNumber);
    }
    T::try_from(f as i64).map(Some).map_err(|_| InvalidNumber)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn numbers_accept_int_and_float_and_reject_text() {
        let o = obj(json!({"a": 1, "b": 2.5, "c": "x", "d": null}));
        assert_eq!(number(&o, "a"), Ok(Some(1.0)));
        assert_eq!(number(&o, "b"), Ok(Some(2.5)));
        assert_eq!(number(&o, "c"), Err(InvalidNumber));
        assert_eq!(number(&o, "d"), Ok(None));
        assert_eq!(number(&o, "fehlt"), Ok(None));
    }

    #[test]
    fn ints_reject_fractions_and_out_of_range() {
        let o = obj(json!({"p": 63.0, "q": 63.5, "r": 300}));
        assert_eq!(int::<u8>(&o, "p"), Ok(Some(63)));
        assert_eq!(int::<u8>(&o, "q"), Err(InvalidNumber));
        assert_eq!(int::<u8>(&o, "r"), Err(InvalidNumber));
    }

    #[test]
    fn decodes_update_topo_from_the_rc_plus_example() {
        let env: Envelope = serde_json::from_value(json!({
            "tid": "t", "bid": "b", "method": "update_topo", "timestamp": 1,
            "data": {"domain": "3", "type": 119, "sub_type": 0, "thing_version": "1.1.2",
                     "sub_devices": [{"sn": "AC1", "domain": "0", "type": 89, "sub_type": 0, "index": "A"}]}
        })).unwrap();
        let topo: UpdateTopo = serde_json::from_value(env.data).unwrap();
        assert_eq!(topo.kind, Some(GATEWAY_TYPE));
        assert_eq!(topo.sub_devices[0].sn, "AC1");
        assert_eq!(topo.sub_devices[0].kind, Some(AIRCRAFT_TYPE));
    }

    #[test]
    fn payload_keys_are_recognised() {
        assert!(is_payload_key("61-0-0"));
        assert!(is_payload_key("39-0-7"));
        assert!(!is_payload_key("battery"));
        assert!(!is_payload_key("61-0"));
    }
}

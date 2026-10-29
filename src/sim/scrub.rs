//! Bereinigt einen echten Rohmitschnitt, damit er als Fixture ins Repo darf: nur Gerätenachrichten,
//! Seriennummern ersetzt, Koordinaten um einen festen Versatz verschoben, Zeit ab einem festen Start.
//! Der Rohmitschnitt selbst kommt nie ins Repo (`AGENTS.md`).

use std::collections::BTreeMap;

use serde_json::Value;

use crate::contract::GeoPoint;
use crate::contract::examples::HOME;
use crate::dji::record::{RecordLine, format_tst};
use crate::dji::topic::{self, DjiTopic};
use crate::sim::synth::FIXTURE_START_MS;

/// Zahlen in diesem Bereich sind Epoch-Millisekunden (2001 bis 2286) und laufen mit der Zeit mit.
const EPOCH_MS: std::ops::Range<i64> = 1_000_000_000_000..10_000_000_000_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Scrubbed {
    pub lines: Vec<RecordLine>,
    /// Original → Ersatz. Nur für die Ausgabe am Terminal, nie in eine Datei.
    pub serials: BTreeMap<String, String>,
    /// Zeilen ohne Gerätenachricht (etwa `services` mit RTMP-Schlüssel) oder mit unlesbarem Payload.
    pub dropped: usize,
    /// Versatz in Grad, der auf jede Breite und Länge addiert wurde.
    pub offset: GeoPoint,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ScrubError {
    #[error("Mitschnitt enthält keine Gerätenachricht")]
    Empty,
    #[error("Zeitstempel {0:?} nicht lesbar")]
    Timestamp(String),
    #[error("Seriennummer {0} steht nach der Bereinigung noch in Zeile {1}")]
    Leak(String, usize),
}

/// Bereinigt den Mitschnitt. Die erste Position landet auf `examples::HOME`, die erste Zeile auf
/// `FIXTURE_START_MS`. Abstände, Höhen und Zeitabstände bleiben, wie sie waren.
pub fn scrub(lines: Vec<RecordLine>) -> Result<Scrubbed, ScrubError> {
    let total = lines.len();
    let device: Vec<(RecordLine, Value)> = lines
        .into_iter()
        .filter(|l| is_device_topic(&l.topic))
        .filter_map(|l| {
            let payload = serde_json::from_str(&l.payload).ok()?;
            Some((l, payload))
        })
        .collect();
    let dropped = total - device.len();
    let t0 = device
        .first()
        .ok_or(ScrubError::Empty)?
        .0
        .at_ms()
        .map_err(|_| ScrubError::Timestamp(device[0].0.tst.clone()))?;
    let serials = serial_map(&device);
    let anchor = device
        .iter()
        .find_map(|(_, p)| first_position(p))
        .unwrap_or(HOME);
    let offset = GeoPoint {
        lat: HOME.lat - anchor.lat,
        lon: HOME.lon - anchor.lon,
    };
    let shift_ms = FIXTURE_START_MS - t0;

    let mut out = Vec::with_capacity(device.len());
    for (line, mut payload) in device {
        let at = line
            .at_ms()
            .map_err(|_| ScrubError::Timestamp(line.tst.clone()))?;
        rewrite(&mut payload, offset, shift_ms);
        let text = replace_serials(&payload.to_string(), &serials);
        out.push(RecordLine {
            tst: format_tst(at + shift_ms),
            topic: replace_serials(&line.topic, &serials),
            qos: line.qos,
            retain: line.retain,
            payloadlen: text.len(),
            payload: text,
        });
    }
    // Gegenprobe: Eine Seriennummer, die irgendwo anders als erwartet steht, darf nicht durchrutschen.
    for (i, line) in out.iter().enumerate() {
        for (original, _) in serials.iter().filter(|(o, sim)| o != sim) {
            if line.topic.contains(original) || line.payload.contains(original) {
                return Err(ScrubError::Leak(original.clone(), i + 1));
            }
        }
    }
    Ok(Scrubbed {
        lines: out,
        serials,
        dropped,
        offset,
    })
}

/// Was der Server abonniert und ein Gerät sendet. Antworten des Servers (`*_reply` auf `sys`,
/// `services`) tragen Zugangsdaten wie RTMP-Schlüssel oder Speicherzugänge und fallen weg.
fn is_device_topic(name: &str) -> bool {
    matches!(
        topic::parse(name),
        DjiTopic::Osd { .. }
            | DjiTopic::State { .. }
            | DjiTopic::Status { .. }
            | DjiTopic::ServicesReply { .. }
            | DjiTopic::Events { .. }
            | DjiTopic::Requests { .. }
    )
}

/// Fernsteuerungen werden `RCSIM…`, Fluggeräte `ACSIM…`, alle übrigen Seriennummern (Akkus,
/// Nutzlasten) `SNSIM…`, jeweils in der Reihenfolge ihres ersten Auftretens.
fn serial_map(device: &[(RecordLine, Value)]) -> BTreeMap<String, String> {
    let mut gateways: Vec<String> = Vec::new();
    let mut aircraft: Vec<String> = Vec::new();
    let mut others: Vec<String> = Vec::new();
    let push = |list: &mut Vec<String>, sn: &str| {
        if !list.iter().any(|s| s == sn) {
            list.push(sn.to_string());
        }
    };
    for (line, payload) in device {
        if let DjiTopic::Status { gateway } = topic::parse(&line.topic) {
            push(&mut gateways, gateway);
            for sub in payload["data"]["sub_devices"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if let Some(sn) = sub["sn"].as_str().filter(|s| topic::is_valid_sn(s)) {
                    push(&mut aircraft, sn);
                }
            }
        }
        if let Some(gateway) = payload["gateway"]
            .as_str()
            .filter(|s| topic::is_valid_sn(s))
        {
            push(&mut gateways, gateway);
        }
    }
    // Eine osd/state ohne update_topo vorher: Die SN im Topic ist dann ein Fluggerät.
    for (line, _) in device {
        if let DjiTopic::Osd { sn } | DjiTopic::State { sn } = topic::parse(&line.topic)
            && !gateways.iter().any(|g| g == sn)
        {
            push(&mut aircraft, sn);
        }
    }
    for (_, payload) in device {
        collect_serials(payload, &mut |sn| {
            if !gateways.iter().any(|g| g == sn) && !aircraft.iter().any(|a| a == sn) {
                push(&mut others, sn);
            }
        });
    }
    let mut map = BTreeMap::new();
    for (prefix, list) in [("RCSIM", gateways), ("ACSIM", aircraft), ("SNSIM", others)] {
        for (i, sn) in list.into_iter().enumerate() {
            map.insert(sn, format!("{prefix}{:04}", i + 1));
        }
    }
    map
}

/// Werte unter `sn`, `*_sn` und `gateway`, die wie eine Seriennummer aussehen.
fn collect_serials(value: &Value, found: &mut impl FnMut(&str)) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                let is_sn_key = key == "sn" || key.ends_with("_sn") || key == "gateway";
                match v.as_str() {
                    Some(s) if is_sn_key && looks_like_serial(s) => found(s),
                    _ => collect_serials(v, found),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|v| collect_serials(v, found)),
        _ => {}
    }
}

/// Echte Seriennummern sind lang genug, um sie gefahrlos per Textersatz zu tauschen. Ein Kürzel
/// wie `"1"` würde jede 1 im Payload treffen.
fn looks_like_serial(s: &str) -> bool {
    s.len() >= 6 && topic::is_valid_sn(s)
}

/// Ersetzt längere Seriennummern zuerst, damit keine kürzere in einer längeren steckt und sie zerteilt.
fn replace_serials(text: &str, serials: &BTreeMap<String, String>) -> String {
    let mut by_len: Vec<_> = serials.iter().collect();
    by_len.sort_by_key(|(sn, _)| std::cmp::Reverse(sn.len()));
    by_len.into_iter().fold(text.to_string(), |acc, (sn, sim)| {
        acc.replace(sn.as_str(), sim)
    })
}

fn is_latitude(key: &str) -> bool {
    key == "latitude" || key == "lat" || key.ends_with("_latitude")
}

fn is_longitude(key: &str) -> bool {
    key == "longitude" || key == "lon" || key == "lng" || key.ends_with("_longitude")
}

/// Die erste Breite/Länge ungleich 0 in einem Objekt. 0/0 sendet DJI ohne GPS-Fix.
fn first_position(value: &Value) -> Option<GeoPoint> {
    match value {
        Value::Object(map) => {
            let lat = map
                .iter()
                .find(|(k, _)| is_latitude(k))
                .and_then(|(_, v)| v.as_f64());
            let lon = map
                .iter()
                .find(|(k, _)| is_longitude(k))
                .and_then(|(_, v)| v.as_f64());
            if let (Some(lat), Some(lon)) = (lat, lon)
                && (lat != 0.0 || lon != 0.0)
            {
                return Some(GeoPoint { lat, lon });
            }
            map.values().find_map(first_position)
        }
        Value::Array(items) => items.iter().find_map(first_position),
        _ => None,
    }
}

/// Verschiebt Koordinaten und Epoch-Millisekunden. Eine 0 bleibt 0: „kein Fix“ soll es bleiben.
fn rewrite(value: &mut Value, offset: GeoPoint, shift_ms: i64) {
    match value {
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                let delta = if is_latitude(key) {
                    Some(offset.lat)
                } else if is_longitude(key) {
                    Some(offset.lon)
                } else {
                    None
                };
                match (delta, v.as_f64()) {
                    (Some(d), Some(x)) if x != 0.0 => *v = round7(x + d).into(),
                    (Some(_), Some(_)) => {}
                    _ => rewrite(v, offset, shift_ms),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|v| rewrite(v, offset, shift_ms)),
        Value::Number(n) => {
            if let Some(ms) = n.as_i64().filter(|ms| EPOCH_MS.contains(ms)) {
                *value = (ms + shift_ms).into();
            }
        }
        _ => {}
    }
}

/// Sieben Nachkommastellen (rund 1 cm), wie DJI sie liefert, ohne Rundungsrauschen der Addition.
fn round7(x: f64) -> f64 {
    (x * 1e7).round() / 1e7
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_791_000_000_000;

    fn line(dt: i64, topic: &str, payload: Value) -> RecordLine {
        RecordLine::new(T0 + dt, topic, payload.to_string().as_bytes())
    }

    fn raw() -> Vec<RecordLine> {
        vec![
            line(
                0,
                "sys/product/5YSZK1C0023RT7/status",
                serde_json::json!({
                    "method": "update_topo", "timestamp": T0,
                    "data": { "sub_devices": [{ "sn": "1581F5BKD225D00A0B5A", "domain": "0" }] }
                }),
            ),
            line(
                500,
                "thing/product/5YSZK1C0023RT7/services",
                serde_json::json!({ "method": "live_start_push", "data": { "url": "rtmp://x/live/geheim" } }),
            ),
            line(
                1_000,
                "thing/product/1581F5BKD225D00A0B5A/osd",
                serde_json::json!({
                    "gateway": "5YSZK1C0023RT7", "timestamp": T0 + 1_000,
                    "data": {
                        "latitude": 53.123_456_7, "longitude": 9.876_543_2, "height": 61.5,
                        "battery": { "batteries": [{ "sn": "4ERPK8B3L1001", "capacity_percent": 80 }] }
                    }
                }),
            ),
            line(
                2_000,
                "thing/product/1581F5BKD225D00A0B5A/state",
                serde_json::json!({
                    "gateway": "5YSZK1C0023RT7",
                    "data": { "home_latitude": 53.124_456_7, "home_longitude": 0.0 }
                }),
            ),
        ]
    }

    #[test]
    fn replaces_serials_by_role() {
        let s = scrub(raw()).unwrap();
        assert_eq!(s.serials["5YSZK1C0023RT7"], "RCSIM0001");
        assert_eq!(s.serials["1581F5BKD225D00A0B5A"], "ACSIM0001");
        assert_eq!(s.serials["4ERPK8B3L1001"], "SNSIM0001");
        assert_eq!(s.lines[0].topic, "sys/product/RCSIM0001/status");
        assert_eq!(s.lines[1].topic, "thing/product/ACSIM0001/osd");
        assert!(s.lines[1].payload.contains("\"gateway\":\"RCSIM0001\""));
        assert!(s.lines[1].payload.contains("SNSIM0001"));
    }

    #[test]
    fn drops_server_commands_with_secrets() {
        let s = scrub(raw()).unwrap();
        assert_eq!(s.dropped, 1);
        assert!(s.lines.iter().all(|l| !l.payload.contains("geheim")));
    }

    #[test]
    fn shifts_positions_by_one_offset_and_keeps_zero() {
        let s = scrub(raw()).unwrap();
        let osd: Value = serde_json::from_str(&s.lines[1].payload).unwrap();
        assert_eq!(osd["data"]["latitude"], HOME.lat);
        assert_eq!(osd["data"]["longitude"], HOME.lon);
        assert_eq!(osd["data"]["height"], 61.5, "Höhen bleiben");
        let state: Value = serde_json::from_str(&s.lines[2].payload).unwrap();
        assert_eq!(state["data"]["home_latitude"], round7(HOME.lat + 0.001));
        assert_eq!(
            state["data"]["home_longitude"], 0.0,
            "kein Fix bleibt kein Fix"
        );
    }

    #[test]
    fn times_start_at_fixture_start_and_keep_their_spacing() {
        let s = scrub(raw()).unwrap();
        assert_eq!(s.lines[0].at_ms().unwrap(), FIXTURE_START_MS);
        assert_eq!(s.lines[1].at_ms().unwrap(), FIXTURE_START_MS + 1_000);
        let osd: Value = serde_json::from_str(&s.lines[1].payload).unwrap();
        assert_eq!(osd["timestamp"], FIXTURE_START_MS + 1_000);
        assert_eq!(s.lines[1].payloadlen, s.lines[1].payload.len());
    }

    #[test]
    fn short_ids_are_not_treated_as_serials() {
        let mut lines = raw();
        lines.push(line(
            3_000,
            "thing/product/1581F5BKD225D00A0B5A/osd",
            serde_json::json!({ "data": { "payloads": [{ "sn": "1", "zoom": 1 }] } }),
        ));
        let s = scrub(lines).unwrap();
        assert!(!s.serials.contains_key("1"));
        assert!(s.lines[3].payload.contains("\"zoom\":1"));
    }

    #[test]
    fn empty_recording_is_an_error() {
        assert_eq!(scrub(vec![]), Err(ScrubError::Empty));
    }
}

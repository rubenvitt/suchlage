//! Beschreibt DJI-Nachrichten fürs Funkprotokoll: ein deutscher Satz, die Methode und der Payload
//! ohne Geheimnisse. Telemetrie (`osd`) kommt bis zu zweimal je Sekunde und wird gedrosselt.

use std::collections::HashMap;

use serde_json::Value;

use super::topic::{self, DjiTopic};
use super::translate::live_error_text;
use crate::contract::{DeviceLogEntry, DeviceLogLevel, DeviceLogSource};
use crate::devlog::truncate;

/// Je Topic höchstens ein Telemetrie-Eintrag in diesem Abstand.
const OSD_EVERY_MS: i64 = 10_000;

/// Felder, die nie ins Protokoll gehören: Gerätegeheimnisse aus `update_topo`, Zugangsdaten.
const SECRET_KEYS: &[&str] = &[
    "device_secret",
    "nonce",
    "password",
    "secret",
    "token",
    "license",
    "app_key",
];

/// Merkt sich je Telemetrie-Topic, wann zuletzt eingetragen wurde und wie viel seitdem fehlt.
#[derive(Default)]
pub struct Describer {
    osd: HashMap<String, (i64, u32)>,
}

impl Describer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Eingehend vom Gerät. `None` für Broker-Verwaltung und gedrosselte Telemetrie.
    pub fn inbound(&mut self, at_ms: i64, topic: &str, payload: &[u8]) -> Option<DeviceLogEntry> {
        let parsed = topic::parse(topic);
        let device_sn = match parsed {
            DjiTopic::Osd { sn } | DjiTopic::State { sn } => sn,
            DjiTopic::Status { gateway }
            | DjiTopic::ServicesReply { gateway }
            | DjiTopic::Events { gateway }
            | DjiTopic::Requests { gateway } => gateway,
            DjiTopic::Other => return None,
        };
        let mut skipped = 0;
        if let DjiTopic::Osd { .. } = parsed {
            let slot = self.osd.entry(topic.to_string()).or_insert((i64::MIN, 0));
            if at_ms.saturating_sub(slot.0) < OSD_EVERY_MS {
                slot.1 += 1;
                return None;
            }
            skipped = slot.1;
            *slot = (at_ms, 0);
        }
        let mut entry = base(at_ms, DeviceLogSource::Device, device_sn, topic, payload);
        let Some(msg) = json(payload) else {
            entry.level = DeviceLogLevel::Warn;
            entry.summary = format!("Nachricht nicht lesbar ({} Byte)", payload.len());
            return Some(entry);
        };
        let method = entry.method.clone().unwrap_or_default();
        entry.summary = match parsed {
            DjiTopic::Osd { .. } if skipped > 0 => {
                format!("Telemetrie (+{skipped} seit dem letzten Eintrag)")
            }
            DjiTopic::Osd { .. } => "Telemetrie".into(),
            DjiTopic::State { .. } => format!("Zustand: {}", keys(&msg["data"])),
            DjiTopic::Status { .. } if method == "update_topo" => topology(&msg["data"]),
            DjiTopic::Status { .. } => format!("Status {method}"),
            DjiTopic::ServicesReply { .. } => {
                let (level, text) = reply(&method, &msg["data"]);
                entry.level = level;
                text
            }
            DjiTopic::Events { .. } => format!("Ereignis {method}"),
            DjiTopic::Requests { .. } => format!("Anfrage {method}"),
            DjiTopic::Other => unreachable!("oben ausgeschlossen"),
        };
        entry.summary.push_str(&tid_suffix(&msg));
        Some(entry)
    }
}

/// Ausgehend vom Server. Nur `thing/` und `sys/`, nie Broker-Verwaltung (dort stehen Passwörter).
pub fn outbound(at_ms: i64, topic: &str, payload: &[u8]) -> Option<DeviceLogEntry> {
    if !(topic.starts_with("thing/") || topic.starts_with("sys/")) {
        return None;
    }
    let device_sn = topic.split('/').nth(2).unwrap_or_default();
    let mut entry = base(at_ms, DeviceLogSource::Server, device_sn, topic, payload);
    let method = entry.method.clone().unwrap_or_default();
    entry.summary = if topic.ends_with("/services") {
        format!("Befehl {method}")
    } else if topic.ends_with("_reply") {
        format!("Antwort auf {method}")
    } else {
        format!("Nachricht {method}")
    };
    if let Some(msg) = json(payload) {
        entry.summary.push_str(&tid_suffix(&msg));
    }
    Some(entry)
}

fn base(
    at_ms: i64,
    source: DeviceLogSource,
    device_sn: &str,
    topic: &str,
    payload: &[u8],
) -> DeviceLogEntry {
    let msg = json(payload);
    DeviceLogEntry {
        seq: 0,
        at_ms,
        source,
        level: DeviceLogLevel::Info,
        device_sn: (!device_sn.is_empty()).then(|| device_sn.to_string()),
        topic: Some(topic.to_string()),
        method: msg
            .as_ref()
            .and_then(|m| m["method"].as_str())
            .map(Into::into),
        summary: String::new(),
        payload: Some(redacted_payload(payload)),
    }
}

fn json(payload: &[u8]) -> Option<Value> {
    serde_json::from_slice::<Value>(payload)
        .ok()
        .filter(Value::is_object)
}

/// Payload als Text ohne Geheimnisse. Unlesbares JSON bleibt Text, aber ebenfalls geschwärzt.
pub fn redacted_payload(payload: &[u8]) -> String {
    let text = match serde_json::from_slice::<Value>(payload) {
        Ok(mut value) => {
            redact(&mut value);
            value.to_string()
        }
        Err(_) => mask_passwords(&String::from_utf8_lossy(payload)),
    };
    truncate(text)
}

fn redact(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            fields.retain(|k, _| !SECRET_KEYS.contains(&k.as_str()));
            fields.values_mut().for_each(redact);
        }
        Value::Array(items) => items.iter_mut().for_each(redact),
        Value::String(s) if s.contains("pass=") => *s = mask_passwords(s),
        _ => {}
    }
}

/// `…&pass=GEHEIM&…` → `…&pass=***&…`: Die RTMP-Adresse trägt den Stream-Schlüssel.
pub fn mask_passwords(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(i) = rest.find("pass=") {
        out.push_str(&rest[..i + 5]);
        out.push_str("***");
        let tail = &rest[i + 5..];
        let end = tail
            .find(|c: char| c == '&' || c == '"' || c.is_whitespace())
            .unwrap_or(tail.len());
        rest = &tail[end..];
    }
    out.push_str(rest);
    out
}

fn keys(data: &Value) -> String {
    let Some(fields) = data.as_object() else {
        return "ohne Daten".into();
    };
    let mut names: Vec<&str> = fields.keys().map(String::as_str).take(6).collect();
    if fields.len() > names.len() {
        names.push("…");
    }
    names.join(", ")
}

fn topology(data: &Value) -> String {
    let subs: Vec<&str> = data["sub_devices"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|d| d["sn"].as_str())
        .collect();
    if subs.is_empty() {
        "Topologie: kein Fluggerät verbunden".into()
    } else {
        format!("Topologie: Fluggerät {}", subs.join(", "))
    }
}

fn reply(method: &str, data: &Value) -> (DeviceLogLevel, String) {
    match data["result"].as_i64() {
        Some(0) => (DeviceLogLevel::Info, format!("Antwort auf {method}: ok")),
        Some(code) if method.starts_with("live_") => (
            DeviceLogLevel::Warn,
            format!("Antwort auf {method}: {}", live_error_text(code)),
        ),
        Some(code) => (
            DeviceLogLevel::Warn,
            format!("Antwort auf {method}: Fehler {code}"),
        ),
        None => (
            DeviceLogLevel::Warn,
            format!("Antwort auf {method} ohne Ergebnis"),
        ),
    }
}

/// Die ersten acht Zeichen der `tid` verbinden Befehl und Antwort in der Liste.
fn tid_suffix(msg: &Value) -> String {
    msg["tid"]
        .as_str()
        .filter(|t| !t.is_empty())
        .map(|t| format!(" · tid {}", t.chars().take(8).collect::<String>()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn bytes(v: Value) -> Vec<u8> {
        v.to_string().into_bytes()
    }

    #[test]
    fn topology_loses_device_secrets() {
        let mut d = Describer::new();
        let topo = json!({"tid": "abcdef123456", "method": "update_topo", "data": {
            "device_secret": "geheim-rc", "nonce": "n1",
            "sub_devices": [{"sn": "AC1", "device_secret": "geheim-ac", "nonce": "n2"}]
        }});
        let e = d
            .inbound(1, "sys/product/RC1/status", &bytes(topo))
            .unwrap();
        assert_eq!(e.summary, "Topologie: Fluggerät AC1 · tid abcdef12");
        assert_eq!(e.device_sn.as_deref(), Some("RC1"));
        assert_eq!(e.method.as_deref(), Some("update_topo"));
        let payload = e.payload.unwrap();
        assert!(
            !payload.contains("geheim") && !payload.contains("nonce"),
            "{payload}"
        );
        assert!(payload.contains("AC1"));
    }

    #[test]
    fn live_command_hides_the_stream_key() {
        let cmd = json!({"tid": "t1", "method": "live_start_push", "data": {
            "url": "rtmp://m:1935/live/RC1?user=rc-RC1&pass=STREAMKEY", "url_type": 1
        }});
        let e = outbound(1, "thing/product/RC1/services", &bytes(cmd)).unwrap();
        assert_eq!(e.summary, "Befehl live_start_push · tid t1");
        let payload = e.payload.unwrap();
        assert!(!payload.contains("STREAMKEY"), "{payload}");
        assert!(payload.contains("user=rc-RC1&pass=***"), "{payload}");
        assert_eq!(mask_passwords("a pass=x&b pass=y"), "a pass=***&b pass=***");
    }

    #[test]
    fn broker_control_never_reaches_the_log() {
        let dynsec = br#"{"commands":[{"command":"createClient","password":"p"}]}"#;
        assert!(outbound(1, "$CONTROL/dynamic-security/v1", dynsec).is_none());
        let mut d = Describer::new();
        assert!(
            d.inbound(1, "$CONTROL/dynamic-security/v1/response", b"{}")
                .is_none()
        );
    }

    #[test]
    fn refused_live_reply_is_a_warning_with_the_reason() {
        let mut d = Describer::new();
        let reply = json!({"tid": "t1", "method": "live_start_push", "data": {"result": 513016}});
        let e = d
            .inbound(1, "thing/product/RC1/services_reply", &bytes(reply))
            .unwrap();
        assert_eq!(e.level, DeviceLogLevel::Warn);
        assert!(
            e.summary.contains("Dekodierung fehlgeschlagen"),
            "{}",
            e.summary
        );
        let ok = json!({"method": "live_stop_push", "data": {"result": 0}});
        let e = d
            .inbound(2, "thing/product/RC1/services_reply", &bytes(ok))
            .unwrap();
        assert_eq!(
            (e.level, e.summary.as_str()),
            (DeviceLogLevel::Info, "Antwort auf live_stop_push: ok")
        );
    }

    #[test]
    fn telemetry_is_throttled_per_topic_and_counts_what_it_skipped() {
        let mut d = Describer::new();
        let osd = bytes(json!({"data": {"latitude": 52.0}}));
        let topic = "thing/product/AC1/osd";
        assert_eq!(d.inbound(0, topic, &osd).unwrap().summary, "Telemetrie");
        assert!(d.inbound(500, topic, &osd).is_none());
        assert!(d.inbound(9_000, topic, &osd).is_none());
        assert!(
            d.inbound(9_000, "thing/product/RC1/osd", &osd).is_some(),
            "eigenes Topic, eigene Drossel"
        );
        assert_eq!(
            d.inbound(10_000, topic, &osd).unwrap().summary,
            "Telemetrie (+2 seit dem letzten Eintrag)"
        );
    }

    #[test]
    fn unreadable_message_is_a_warning_and_state_lists_its_fields() {
        let mut d = Describer::new();
        let e = d.inbound(1, "thing/product/RC1/state", b"{kaputt").unwrap();
        assert_eq!(e.level, DeviceLogLevel::Warn);
        assert_eq!(e.summary, "Nachricht nicht lesbar (7 Byte)");
        let state = bytes(json!({"data": {"firmware_version": "1", "live_capacity": {}}}));
        let e = d.inbound(2, "thing/product/RC1/state", &state).unwrap();
        assert_eq!(e.summary, "Zustand: firmware_version, live_capacity");
    }
}

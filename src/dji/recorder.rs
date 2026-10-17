//! Rohmitschnitt der DJI-Nachrichten im M0-Format (Spec §4.5). Eine Datei je Sitzungsstart,
//! nichts wird überschrieben. Nur `thing/` und `sys/` – nie Broker-Verwaltung.

use std::borrow::Cow;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;

use super::record::{RecordLine, format_tst};

pub struct Recorder {
    file: File,
    name: String,
}

impl Recorder {
    /// Öffnet `<dir>/JJJJMMTTTHHMMSSZ.jsonl` zum Anhängen; der Ordner entsteht bei Bedarf.
    pub fn open(dir: &Path, at_ms: i64) -> io::Result<Recorder> {
        std::fs::create_dir_all(dir)?;
        let stamp: String = format_tst(at_ms)
            .chars()
            .filter(|c| c.is_ascii_digit())
            .take(14)
            .collect();
        let name = format!("{}T{}Z.jsonl", &stamp[..8], &stamp[8..]);
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(&name))?;
        Ok(Recorder { file, name })
    }

    pub fn file_name(&self) -> &str {
        &self.name
    }

    pub fn write(&mut self, line: &RecordLine) -> io::Result<()> {
        if !(line.topic.starts_with("thing/") || line.topic.starts_with("sys/")) {
            return Ok(());
        }
        let redacted;
        let line = if line.topic.starts_with("sys/") && line.topic.ends_with("/status") {
            match without_device_secrets(&line.payload) {
                None => return Ok(()),
                Some(Cow::Borrowed(_)) => line,
                Some(Cow::Owned(payload)) => {
                    redacted = RecordLine {
                        payloadlen: payload.len(),
                        payload,
                        ..line.clone()
                    };
                    &redacted
                }
            }
        } else {
            line
        };
        let mut text = serde_json::to_string(line).expect("RecordLine ist serialisierbar");
        text.push('\n');
        self.file.write_all(text.as_bytes())
    }
}

/// `update_topo` trägt `device_secret` und `nonce` von RC und Fluggerät (in `data` und
/// `data.sub_devices[]`). Aus Mitschnitten werden Fixtures, deshalb kommen sie nie auf die Platte.
/// Unlesbares JSON fällt ganz weg (`None`), statt ungeprüft geschrieben zu werden.
fn without_device_secrets(payload: &str) -> Option<Cow<'_, str>> {
    let mut msg: serde_json::Value = serde_json::from_str(payload).ok()?;
    let Some(data) = msg.get_mut("data") else {
        return Some(Cow::Borrowed(payload));
    };
    let mut removed = strip_secrets(data);
    if let Some(subs) = data.get_mut("sub_devices").and_then(|s| s.as_array_mut()) {
        for sub in subs {
            removed |= strip_secrets(sub);
        }
    }
    Some(if removed {
        Cow::Owned(msg.to_string())
    } else {
        Cow::Borrowed(payload)
    })
}

fn strip_secrets(value: &mut serde_json::Value) -> bool {
    let Some(fields) = value.as_object_mut() else {
        return false;
    };
    fields.remove("device_secret").is_some() | fields.remove("nonce").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dji::record::read_lines;

    #[test]
    fn writes_m0_format_and_skips_broker_control() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Recorder::open(dir.path(), 1_792_149_826_953).unwrap();
        assert_eq!(r.file_name(), "20261016T112346Z.jsonl");
        r.write(&RecordLine::new(1, "thing/product/AC1/osd", b"{\"a\":\n1}"))
            .unwrap();
        r.write(&RecordLine::new(2, "$CONTROL/dynamic-security/v1", b"{}"))
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join(r.file_name())).unwrap();
        let lines: Vec<_> = read_lines(text.as_bytes()).collect();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].as_ref().unwrap().payload, "{\"a\":\n1}");
    }

    #[test]
    fn status_loses_device_secrets_and_keeps_everything_else() {
        let dir = tempfile::tempdir().unwrap();
        let mut r = Recorder::open(dir.path(), 1_792_149_826_953).unwrap();
        let topo = serde_json::json!({
            "tid": "t1", "bid": "b1", "method": "update_topo",
            "data": {
                "type": 119, "sub_type": 0, "device_secret": "geheim-rc", "nonce": "nonce-rc",
                "sub_devices": [{
                    "sn": "AC1", "type": 89, "sub_type": 0, "index": "A",
                    "device_secret": "geheim-ac", "nonce": "nonce-ac"
                }]
            }
        })
        .to_string();
        let plain = r#"{"method":"update_topo","data":{"type":119,"sub_devices":[]}}"#;
        r.write(&RecordLine::new(
            1,
            "sys/product/RC1/status",
            topo.as_bytes(),
        ))
        .unwrap();
        r.write(&RecordLine::new(
            2,
            "sys/product/RC1/status",
            plain.as_bytes(),
        ))
        .unwrap();
        r.write(&RecordLine::new(
            3,
            "sys/product/RC1/status",
            b"{\"device_secret\":",
        ))
        .unwrap();
        let text = std::fs::read_to_string(dir.path().join(r.file_name())).unwrap();
        assert!(
            !text.contains("geheim") && !text.contains("nonce"),
            "{text}"
        );
        let lines: Vec<_> = read_lines(text.as_bytes()).map(Result::unwrap).collect();
        assert_eq!(lines.len(), 2, "unlesbarer Status fällt weg");
        let first: serde_json::Value = serde_json::from_str(&lines[0].payload).unwrap();
        assert_eq!(first["tid"], "t1");
        assert_eq!(first["data"]["type"], 119);
        assert_eq!(first["data"]["sub_devices"][0]["sn"], "AC1");
        assert_eq!(first["data"]["sub_devices"][0]["index"], "A");
        assert_eq!(lines[0].payloadlen, lines[0].payload.len());
        assert_eq!(lines[1].payload, plain, "ohne Geheimnisse unverändert");
    }

    #[test]
    fn reopening_appends_instead_of_overwriting() {
        let dir = tempfile::tempdir().unwrap();
        let at = 1_792_149_826_953;
        Recorder::open(dir.path(), at)
            .unwrap()
            .write(&RecordLine::new(1, "sys/product/RC1/status", b"{}"))
            .unwrap();
        let mut again = Recorder::open(dir.path(), at).unwrap();
        again
            .write(&RecordLine::new(2, "sys/product/RC1/status", b"{}"))
            .unwrap();
        let text = std::fs::read_to_string(dir.path().join(again.file_name())).unwrap();
        assert_eq!(read_lines(text.as_bytes()).count(), 2);
    }
}

//! Mitschnittformat: eine JSON-Zeile je MQTT-Nachricht, genau wie `mosquitto_sub -F %j` in M0.
//! Der Payload steht als String darin (escaped), also auch mehrzeilige Payloads in einer Zeile.

use std::io::BufRead;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecordLine {
    /// Empfangszeit, Format `2026-10-16T11:23:45.802821+0000`.
    pub tst: String,
    pub topic: String,
    #[serde(default)]
    pub qos: u8,
    #[serde(default)]
    pub retain: u8,
    #[serde(default)]
    pub payloadlen: usize,
    pub payload: String,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum RecordError {
    #[error("Zeile {line}: kein gültiges Mitschnitt-JSON ({reason})")]
    Json { line: usize, reason: String },
    #[error("Zeitstempel {0:?} nicht lesbar")]
    Timestamp(String),
    #[error("Lesefehler: {0}")]
    Io(String),
}

pub fn format_tst(at_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(at_ms)
        .unwrap_or_default()
        .format("%Y-%m-%dT%H:%M:%S%.6f%z")
        .to_string()
}

pub fn parse_tst(tst: &str) -> Result<i64, RecordError> {
    chrono::DateTime::parse_from_str(tst, "%Y-%m-%dT%H:%M:%S%.f%z")
        .map(|t| t.timestamp_millis())
        .map_err(|_| RecordError::Timestamp(tst.to_string()))
}

impl RecordLine {
    pub fn new(at_ms: i64, topic: &str, payload: &[u8]) -> Self {
        Self {
            tst: format_tst(at_ms),
            topic: topic.to_string(),
            qos: 0,
            retain: 0,
            payloadlen: payload.len(),
            payload: String::from_utf8_lossy(payload).into_owned(),
        }
    }

    pub fn at_ms(&self) -> Result<i64, RecordError> {
        parse_tst(&self.tst)
    }
}

pub fn read_lines(reader: impl BufRead) -> impl Iterator<Item = Result<RecordLine, RecordError>> {
    reader
        .lines()
        .enumerate()
        .filter_map(|(i, line)| match line {
            Err(e) => Some(Err(RecordError::Io(e.to_string()))),
            Ok(text) if text.trim().is_empty() => None,
            Ok(text) => Some(serde_json::from_str(&text).map_err(|e| RecordError::Json {
                line: i + 1,
                reason: e.to_string(),
            })),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Zeile aus dem M0-Smoke-Mitschnitt (records/20261016T112343Z.jsonl), Seriennummer TEST.
    const M0_LINE: &str = r#"{"tst":"2026-10-16T11:23:46.953829+0000","topic":"thing/product/TEST/state","qos":0,"retain":0,"payloadlen":8,"payload":"{\"a\":\n1}"}"#;

    #[test]
    fn reads_m0_line_with_multiline_payload() {
        let lines: Vec<_> = read_lines(M0_LINE.as_bytes()).collect();
        let line = lines[0].as_ref().unwrap();
        assert_eq!(line.topic, "thing/product/TEST/state");
        assert_eq!(line.payload, "{\"a\":\n1}");
        assert_eq!(line.at_ms().unwrap(), 1_792_149_826_953);
    }

    #[test]
    fn new_line_roundtrips_through_the_m0_format() {
        let line = RecordLine::new(1_792_149_826_953, "sys/product/RC1/status", b"{\"x\":1}");
        assert_eq!(line.tst, "2026-10-16T11:23:46.953000+0000");
        assert_eq!(line.payloadlen, 7);
        let json = serde_json::to_string(&line).unwrap();
        let back: Vec<_> = read_lines(json.as_bytes()).collect();
        assert_eq!(back[0].as_ref().unwrap(), &line);
    }

    #[test]
    fn broken_line_reports_its_number_and_reading_continues() {
        let input = format!("{M0_LINE}\nkaputt\n\n{M0_LINE}\n");
        let lines: Vec<_> = read_lines(input.as_bytes()).collect();
        assert_eq!(lines.len(), 3, "Leerzeilen werden übersprungen");
        assert!(matches!(lines[1], Err(RecordError::Json { line: 2, .. })));
        assert!(lines[2].is_ok());
    }
}

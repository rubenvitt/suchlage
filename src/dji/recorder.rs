//! Rohmitschnitt der DJI-Nachrichten im M0-Format (Spec §4.5). Eine Datei je Sitzungsstart,
//! nichts wird überschrieben. Nur `thing/` und `sys/` – nie Broker-Verwaltung.

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
        let mut text = serde_json::to_string(line).expect("RecordLine ist serialisierbar");
        text.push('\n');
        self.file.write_all(text.as_bytes())
    }
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

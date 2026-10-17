//! Funkprotokoll im Speicher: die letzten Einträge für neue Zuschauer und ein Broadcast für
//! laufende. Kennt keine DJI-Typen; was hineinkommt, ist schon beschrieben und geschwärzt.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use tokio::sync::{broadcast, watch};

use crate::contract::{DeviceLogEntry, DeviceLogLevel, DeviceLogSource};
use crate::time::now_ms;

/// So viele Einträge sieht, wer die Systemseite neu öffnet.
const HISTORY: usize = 500;
const CHANNEL: usize = 256;
/// Länger wird ein Payload nicht; danach steht „…“.
pub const PAYLOAD_LIMIT: usize = 4 * 1024;

struct Inner {
    history: VecDeque<DeviceLogEntry>,
    next_seq: u64,
}

#[derive(Clone)]
pub struct DeviceLog {
    inner: Arc<Mutex<Inner>>,
    tx: broadcast::Sender<DeviceLogEntry>,
    stop: watch::Receiver<bool>,
}

impl DeviceLog {
    /// `stop` beendet laufende Ströme beim Herunterfahren; sonst hielten sie `serve` offen.
    pub fn new(stop: watch::Receiver<bool>) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                history: VecDeque::with_capacity(HISTORY),
                next_seq: 1,
            })),
            tx: broadcast::channel(CHANNEL).0,
            stop,
        }
    }

    /// Kehrt zurück, sobald der Server herunterfährt.
    pub async fn stopped(&self) {
        let mut stop = self.stop.clone();
        let _ = stop.wait_for(|s| *s).await;
    }

    /// Vergibt `seq`, legt ab und verteilt. Wartet nie: Langsame Zuschauer verpassen Einträge.
    pub fn push(&self, mut entry: DeviceLogEntry) {
        let mut inner = self.inner.lock().expect("Funkprotokoll nicht vergiftet");
        entry.seq = inner.next_seq;
        inner.next_seq += 1;
        if inner.history.len() == HISTORY {
            inner.history.pop_front();
        }
        inner.history.push_back(entry.clone());
        // Unter der Sperre: So kommt kein Eintrag zwischen Verlauf und Strom doppelt oder gar nicht.
        let _ = self.tx.send(entry);
    }

    /// Kurzform für Feststellungen des Servers ohne Topic und Payload.
    pub fn system(&self, level: DeviceLogLevel, device_sn: Option<&str>, summary: String) {
        self.push(DeviceLogEntry {
            seq: 0,
            at_ms: now_ms(),
            source: DeviceLogSource::System,
            level,
            device_sn: device_sn.map(Into::into),
            topic: None,
            method: None,
            summary,
            payload: None,
        });
    }

    /// Verlauf und Strom ab genau diesem Stand.
    pub fn subscribe(&self) -> (Vec<DeviceLogEntry>, broadcast::Receiver<DeviceLogEntry>) {
        let inner = self.inner.lock().expect("Funkprotokoll nicht vergiftet");
        (inner.history.iter().cloned().collect(), self.tx.subscribe())
    }
}

/// Kürzt auf höchstens `PAYLOAD_LIMIT` Byte, an einer Zeichengrenze.
pub fn truncate(mut text: String) -> String {
    if text.len() > PAYLOAD_LIMIT {
        let mut cut = PAYLOAD_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push('…');
    }
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_entries_keeps_the_newest_and_streams_after_the_history() {
        let log = DeviceLog::new(watch::channel(false).1);
        for i in 0..(HISTORY + 3) {
            log.system(DeviceLogLevel::Info, None, format!("e{i}"));
        }
        let (history, mut rx) = log.subscribe();
        assert_eq!(history.len(), HISTORY);
        assert_eq!(history[0].seq, 4, "die ältesten fallen heraus");
        assert_eq!(history.last().unwrap().summary, format!("e{}", HISTORY + 2));
        log.system(DeviceLogLevel::Warn, Some("RC1"), "neu".into());
        let next = rx.try_recv().unwrap();
        assert_eq!(
            (next.seq, next.device_sn.as_deref()),
            (HISTORY as u64 + 4, Some("RC1"))
        );
    }

    #[test]
    fn truncates_on_a_char_boundary() {
        let long = "ä".repeat(PAYLOAD_LIMIT);
        let cut = truncate(long);
        assert!(cut.len() <= PAYLOAD_LIMIT + '…'.len_utf8());
        assert!(cut.ends_with('…'));
        assert_eq!(truncate("kurz".into()), "kurz");
    }
}

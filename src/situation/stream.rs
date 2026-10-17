//! Zustand des Livebilds je Fernsteuerung. Reine Übergänge; Befehle laufen außerhalb (runtime).

use crate::contract::{StreamPhase, StreamState};

use super::observation::{LiveStreamInfo, StreamCommand};

pub fn idle(gateway_sn: &str, at_ms: i64) -> StreamState {
    StreamState {
        gateway_sn: gateway_sn.into(),
        phase: StreamPhase::Idle,
        video_id: None,
        lens: None,
        error: None,
        updated_at_ms: at_ms,
    }
}

/// Prüft und übernimmt einen Befehl in einem Schritt, damit zwei gleichzeitige Aufrufer nicht
/// beide durchkommen. `None`: Der Befehl passt nicht zum Zustand (Start bei laufendem Bild, Stopp
/// oder Linsenwechsel ohne Bild); nichts ändert sich. Sonst `Some(true)`, wenn sich der Zustand
/// geändert hat.
pub fn on_issued(s: &mut StreamState, command: &StreamCommand, at_ms: i64) -> Option<bool> {
    let busy = matches!(
        s.phase,
        StreamPhase::Starting | StreamPhase::Live | StreamPhase::Stopping
    );
    let accepted = match command {
        // Ein zweites `live_start_push` lehnt Pilot 2 ab (513003).
        StreamCommand::Start { .. } => !busy,
        StreamCommand::Stop | StreamCommand::ChangeLens { .. } => s.phase == StreamPhase::Live,
    };
    if !accepted {
        return None;
    }
    let before = s.clone();
    match command {
        StreamCommand::Start { video_id } => {
            s.phase = StreamPhase::Starting;
            s.video_id = Some(video_id.clone());
            s.error = None;
        }
        StreamCommand::Stop => s.phase = StreamPhase::Stopping,
        // Die Linse gilt erst, wenn das Gerät den Wechsel bestätigt.
        StreamCommand::ChangeLens { .. } => {}
    }
    Some(changed(s, &before, at_ms))
}

/// Ein Fehler ändert die Phase nur, solange sie noch die dieses Befehls ist: Ein später
/// abgelehnter Doppelbefehl darf ein laufendes oder beendetes Bild nicht umwerfen.
pub fn on_finished(
    s: &mut StreamState,
    command: &StreamCommand,
    result: &Result<(), String>,
    at_ms: i64,
) -> bool {
    let before = s.clone();
    match (command, result) {
        (StreamCommand::Start { .. }, Ok(())) => {
            s.phase = StreamPhase::Live;
            s.error = None;
        }
        (StreamCommand::Start { .. }, Err(message)) if s.phase == StreamPhase::Starting => {
            s.phase = StreamPhase::Failed;
            s.error = Some(message.clone());
        }
        (StreamCommand::Stop, Ok(())) => {
            s.phase = StreamPhase::Idle;
            s.video_id = None;
        }
        // Stopp abgelehnt: Der Stream läuft weiter.
        (StreamCommand::Stop, Err(_)) if s.phase == StreamPhase::Stopping => {
            s.phase = StreamPhase::Live;
        }
        (StreamCommand::ChangeLens { lens }, Ok(())) => s.lens = Some(*lens),
        (_, Err(_)) => {}
    }
    changed(s, &before, at_ms)
}

/// Meldung des Geräts (`live_status`). `starting` bleibt bestehen, bis die Befehlsantwort kommt.
pub fn on_status(
    s: &mut StreamState,
    live: &Option<LiveStreamInfo>,
    error: Option<&str>,
    at_ms: i64,
) -> bool {
    let before = s.clone();
    match live {
        Some(info) => {
            s.phase = StreamPhase::Live;
            s.video_id = Some(info.video_id.clone());
            if info.lens.is_some() {
                s.lens = info.lens;
            }
            s.error = None;
        }
        // Das Gerät meldet selbst einen Fehler: Der Stream ist gescheitert, mit Grund.
        None if error.is_some() && matches!(s.phase, StreamPhase::Starting | StreamPhase::Live) => {
            s.phase = StreamPhase::Failed;
            s.error = error.map(String::from);
        }
        None if matches!(s.phase, StreamPhase::Live | StreamPhase::Stopping) => {
            s.phase = StreamPhase::Idle;
            s.video_id = None;
        }
        None => {}
    }
    changed(s, &before, at_ms)
}

/// Setzt `updated_at_ms` nur bei einer echten Änderung, damit unveränderte Meldungen kein Ereignis erzeugen.
fn changed(s: &mut StreamState, before: &StreamState, at_ms: i64) -> bool {
    let differs = s.phase != before.phase
        || s.video_id != before.video_id
        || s.lens != before.lens
        || s.error != before.error;
    if differs {
        s.updated_at_ms = at_ms;
    }
    differs
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Lens;

    fn start() -> StreamCommand {
        StreamCommand::Start {
            video_id: "AC1/61-0-0/normal-0".into(),
        }
    }

    #[test]
    fn start_goes_starting_then_live() {
        let mut s = idle("RC1", 0);
        assert_eq!(on_issued(&mut s, &start(), 1), Some(true));
        assert_eq!(s.phase, StreamPhase::Starting);
        assert_eq!(s.video_id.as_deref(), Some("AC1/61-0-0/normal-0"));
        assert!(on_finished(&mut s, &start(), &Ok(()), 2));
        assert_eq!(s.phase, StreamPhase::Live);
    }

    #[test]
    fn rejected_start_fails_with_message() {
        let mut s = idle("RC1", 0);
        on_issued(&mut s, &start(), 1);
        on_finished(
            &mut s,
            &start(),
            &Err("Livestream ist in Pilot 2 nicht aktiviert (513011).".into()),
            2,
        );
        assert_eq!(s.phase, StreamPhase::Failed);
        assert!(s.error.as_deref().unwrap().contains("513011"));
    }

    #[test]
    fn device_status_confirms_and_ends_streams() {
        let mut s = idle("RC1", 0);
        let live = Some(LiveStreamInfo {
            video_id: "AC1/61-0-0/normal-0".into(),
            lens: Some(Lens::Thermal),
        });
        assert!(on_status(&mut s, &live, None, 1));
        assert_eq!((s.phase, s.lens), (StreamPhase::Live, Some(Lens::Thermal)));
        assert!(!on_status(&mut s, &live, None, 2), "unverändert");
        assert!(on_status(&mut s, &None, None, 3));
        assert_eq!(s.phase, StreamPhase::Idle);
    }

    #[test]
    fn starting_survives_a_status_without_stream() {
        let mut s = idle("RC1", 0);
        on_issued(&mut s, &start(), 1);
        assert!(!on_status(&mut s, &None, None, 2));
        assert_eq!(s.phase, StreamPhase::Starting);
    }

    #[test]
    fn device_error_fails_the_stream_with_its_reason() {
        let mut s = idle("RC1", 0);
        on_issued(&mut s, &start(), 1);
        assert!(on_status(
            &mut s,
            &None,
            Some("Pilot 2 lehnt ab: Dekodierung fehlgeschlagen (513016)."),
            2
        ));
        assert_eq!(s.phase, StreamPhase::Failed);
        assert!(s.error.as_deref().unwrap().contains("513016"));
    }

    fn live() -> StreamState {
        let mut s = idle("RC1", 0);
        on_issued(&mut s, &start(), 1);
        on_finished(&mut s, &start(), &Ok(()), 2);
        s
    }

    #[test]
    fn second_start_is_refused_and_its_error_cannot_fail_a_live_stream() {
        let mut s = idle("RC1", 0);
        assert!(on_issued(&mut s, &start(), 1).is_some());
        assert_eq!(on_issued(&mut s, &start(), 1), None, "läuft bereits");
        on_finished(&mut s, &start(), &Ok(()), 2);
        assert!(!on_finished(&mut s, &start(), &Err("513003".into()), 3));
        assert_eq!((s.phase, s.error.as_deref()), (StreamPhase::Live, None));

        // Andere Reihenfolge: Der Fehler kommt zuerst, das Ok danach räumt ihn weg.
        let mut s = idle("RC1", 0);
        on_issued(&mut s, &start(), 1);
        on_finished(&mut s, &start(), &Err("513003".into()), 2);
        on_finished(&mut s, &start(), &Ok(()), 3);
        assert_eq!((s.phase, s.error.as_deref()), (StreamPhase::Live, None));
    }

    #[test]
    fn late_stop_error_keeps_a_stopped_stream_idle() {
        let mut s = live();
        assert!(on_issued(&mut s, &StreamCommand::Stop, 3).is_some());
        assert_eq!(on_issued(&mut s, &StreamCommand::Stop, 3), None);
        on_finished(&mut s, &StreamCommand::Stop, &Ok(()), 4);
        assert!(!on_finished(
            &mut s,
            &StreamCommand::Stop,
            &Err("x".into()),
            5
        ));
        assert_eq!((s.phase, s.video_id.as_deref()), (StreamPhase::Idle, None));
        assert_eq!(
            on_issued(&mut s, &StreamCommand::Stop, 6),
            None,
            "kein Bild"
        );
        assert!(on_issued(&mut s, &start(), 6).is_some(), "neu startbar");
    }

    #[test]
    fn rejected_stop_keeps_the_stream_live() {
        let mut s = live();
        on_issued(&mut s, &StreamCommand::Stop, 3);
        on_finished(&mut s, &StreamCommand::Stop, &Err("x".into()), 4);
        assert_eq!(s.phase, StreamPhase::Live);
    }

    #[test]
    fn lens_change_only_changes_lens_on_success() {
        let mut s = idle("RC1", 0);
        on_status(
            &mut s,
            &Some(LiveStreamInfo {
                video_id: "v".into(),
                lens: Some(Lens::Wide),
            }),
            None,
            1,
        );
        let cmd = StreamCommand::ChangeLens { lens: Lens::Zoom };
        assert_eq!(on_issued(&mut s, &cmd, 2), Some(false));
        assert!(!on_finished(&mut s, &cmd, &Err("x".into()), 3));
        assert_eq!(s.lens, Some(Lens::Wide));
        assert!(on_finished(&mut s, &cmd, &Ok(()), 4));
        assert_eq!(s.lens, Some(Lens::Zoom));
    }
}

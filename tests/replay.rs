//! Replay durch Übersetzer und Lage-Kern: der synthetische Flug und ein bereinigter echter
//! Mitschnitt (M350 mit RC Plus, Feldtest 22.10.2026, `suchlage-sim scrub`).

use std::io::BufReader;

use suchlage::contract::{FlightEndReason, LiveEvent};
use suchlage::dji::record::{RecordLine, read_lines};
use suchlage::dji::translate::Translator;
use suchlage::sim::scrub::scrub;
use suchlage::sim::synth::FIXTURE_PATH;

const FIELD_1: &str = "tests/fixtures/dji/m350-field-1.jsonl";
use suchlage::situation::state::{Config, Effect, Situation};

fn read(path: &str) -> Vec<RecordLine> {
    let file = BufReader::new(std::fs::File::open(path).unwrap());
    read_lines(file).map(Result::unwrap).collect()
}

fn replay(path: &str) -> (Situation, Vec<Effect>) {
    replay_lines(read(path))
}

fn replay_lines(lines: Vec<RecordLine>) -> (Situation, Vec<Effect>) {
    let mut translator = Translator::new();
    let mut situation = Situation::new(
        Config {
            auto_live_on_takeoff: true,
        },
        1,
        vec![],
    );
    let mut effects = Vec::new();
    for line in lines {
        let at = line.at_ms().unwrap();
        for obs in translator
            .handle(at, &line.topic, line.payload.as_bytes())
            .observations
        {
            effects.extend(situation.apply(obs));
        }
        effects.extend(situation.tick(at));
    }
    (situation, effects)
}

#[test]
fn synthetic_flight_yields_one_landed_flight_with_two_segments() {
    let (situation, effects) = replay(FIXTURE_PATH);
    let ended: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::Publish(LiveEvent::FlightEnded { flight }) => Some(flight),
            _ => None,
        })
        .collect();
    assert_eq!(ended.len(), 1, "genau ein Flug");
    let flight = ended[0];
    assert_eq!(flight.end_reason, Some(FlightEndReason::Landed));
    let duration_s = (flight.ended_at_ms.unwrap() - flight.started_at_ms) / 1000;
    assert!(
        (150..=170).contains(&duration_s),
        "Flugdauer {duration_s} s"
    );
    assert!(
        flight.distance_m > 800.0,
        "Strecke {} m (ohne die Lücke)",
        flight.distance_m
    );
    assert_eq!(flight.max_elevation_m, Some(60.0));

    let segments: std::collections::BTreeSet<u32> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::AppendTrackPoint { segment, .. } => Some(*segment),
            _ => None,
        })
        .collect();
    assert_eq!(
        segments.into_iter().collect::<Vec<_>>(),
        vec![0, 1],
        "die 15-s-Lücke trennt die Spur"
    );

    assert!(effects.contains(&Effect::RequestLiveStart {
        gateway_sn: "RCSIM0001".into()
    }));
    let snapshot = situation.snapshot(0);
    assert_eq!(
        snapshot.gateways[0].aircraft_sn, None,
        "Fluggerät am Ende ausgeschaltet"
    );
    assert_eq!(
        snapshot.system.unknown_fields[0].key,
        "aircraft.extra_probe"
    );
    assert_eq!(snapshot.system.invalid_messages, 0);
}

/// Die Bereinigung darf nichts an der Lage ändern: Der bereinigte synthetische Flug ergibt denselben Flug.
#[test]
fn scrubbed_flight_replays_like_the_original() {
    let flights = |effects: Vec<Effect>| -> Vec<_> {
        effects
            .into_iter()
            .filter_map(|e| match e {
                Effect::Publish(LiveEvent::FlightEnded { flight }) => Some(flight),
                _ => None,
            })
            .collect()
    };
    let (_, original) = replay(FIXTURE_PATH);
    let scrubbed = scrub(read(FIXTURE_PATH)).unwrap();
    assert_eq!(scrubbed.dropped, 0);
    let (_, again) = replay_lines(scrubbed.lines);
    let (a, b) = (flights(original), flights(again));
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_eq!(a[0].started_at_ms, b[0].started_at_ms);
    assert_eq!(a[0].ended_at_ms, b[0].ended_at_ms);
    assert_eq!(a[0].max_elevation_m, b[0].max_elevation_m);
    assert!(
        (a[0].distance_m - b[0].distance_m).abs() < 1.0,
        "Strecke bleibt"
    );
}

/// Echter Flug: zwei Fernsteuerungen online, an der zweiten kurz manuell geflogen und gelandet. Die Werte stammen
/// aus dem Mitschnitt, nicht aus der Doku; sie belegen die Annahmen aus Plan M1, Offene Punkte 3.
#[test]
fn field_flight_m350_yields_one_landed_flight() {
    let (situation, effects) = replay(FIELD_1);
    let ended: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::Publish(LiveEvent::FlightEnded { flight }) => Some(flight),
            _ => None,
        })
        .collect();
    assert_eq!(ended.len(), 1, "genau ein Flug");
    let flight = ended[0];
    assert_eq!(flight.gateway_sn, "RCSIM0002");
    assert_eq!(flight.aircraft_sn, "ACSIM0001");
    // Gelandet über mode_code 3 → 11 → 0: 11 ist die normale manuelle Landung, nicht nur Notfall.
    assert_eq!(flight.end_reason, Some(FlightEndReason::Landed));
    let duration_s = (flight.ended_at_ms.unwrap() - flight.started_at_ms) / 1000;
    assert!(
        (200..=215).contains(&duration_s),
        "Flugdauer {duration_s} s"
    );
    assert_eq!(flight.max_elevation_m, Some(33.4));
    assert!(
        (100.0..200.0).contains(&flight.distance_m),
        "Strecke {} m",
        flight.distance_m
    );

    let segments: std::collections::BTreeSet<u32> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::AppendTrackPoint { segment, .. } => Some(*segment),
            _ => None,
        })
        .collect();
    assert_eq!(
        segments.into_iter().collect::<Vec<_>>(),
        vec![0],
        "keine Lücke im Flug"
    );

    // `live_status` kam in der state-Nachricht der RC; der Auto-Start des Livebilds greift.
    assert!(effects.contains(&Effect::RequestLiveStart {
        gateway_sn: "RCSIM0002".into()
    }));

    // Einheiten: `height` (ellipsoidisch) und `elevation` (über Start) sind beide Meter, ihr
    // Abstand bleibt im Flug gleich; Geschwindigkeit in m/s und Abstand in m bleiben plausibel.
    let telemetry: Vec<_> = effects
        .iter()
        .filter_map(|e| match e {
            Effect::Publish(LiveEvent::Aircraft { aircraft }) => Some(&aircraft.telemetry),
            _ => None,
        })
        .collect();
    let offsets: Vec<f64> = telemetry
        .iter()
        .filter_map(|t| Some(t.height_ellipsoid_m? - t.elevation_takeoff_m?))
        .collect();
    let (lo, hi) = offsets
        .iter()
        .fold((f64::MAX, f64::MIN), |(lo, hi), o| (lo.min(*o), hi.max(*o)));
    assert!(
        hi - lo < 1.0,
        "height − elevation schwankt um {} m",
        hi - lo
    );
    let max = |f: fn(&&suchlage::contract::Telemetry) -> Option<f64>| {
        telemetry.iter().filter_map(f).fold(0.0, f64::max)
    };
    assert!((3.0..15.0).contains(&max(|t| t.horizontal_speed_mps)));
    assert!((20.0..60.0).contains(&max(|t| t.home_distance_m)));

    let snapshot = situation.snapshot(0);
    let gateway = snapshot
        .gateways
        .iter()
        .find(|g| g.sn == "RCSIM0002")
        .unwrap();
    assert_eq!(gateway.aircraft_sn.as_deref(), Some("ACSIM0001"));
    // Die RC ohne Fluggerät meldet `4g_quality` 65535 (kein LTE-Dongle); das ist kein Fehler.
    assert_eq!(snapshot.system.invalid_messages, 0);
}

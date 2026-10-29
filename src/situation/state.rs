//! Der Lage-Kern: hält den Live-Zustand aller Geräte, erkennt Flüge, baut Spuren und sagt der
//! Laufzeit per `Effect`, was zu speichern und zu verteilen ist. Rein und deterministisch:
//! Zeit kommt nur über `Observed::at_ms` und `tick(now_ms)`.

use std::collections::{BTreeMap, HashMap};

use crate::contract::*;

use super::observation::{ModeClass, Observation, Observed, StreamCommand};
use super::phase::{PhaseTracker, Sample, Transition};
use super::stream;
use super::track::TrackBuilder;

const SYSTEM_PUBLISH_EVERY_MS: i64 = 5_000;
/// Feldnamen bestimmt das Gerät; ohne Obergrenze wüchse die Systemansicht mit jeder Nachricht.
const MAX_UNKNOWN_FIELDS: usize = 200;
const MAX_UNKNOWN_FIELD_CHARS: usize = 128;
/// Punkte, die vor der bestätigten Startprobe liegen können (Entprellung über 2 Proben).
const PENDING_POINTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub auto_live_on_takeoff: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResumeFlight {
    pub flight: FlightSummary,
    pub segments: Vec<Vec<TrackPoint>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    Publish(LiveEvent),
    InsertFlight(FlightSummary),
    AppendTrackPoint {
        flight_id: i64,
        segment: u32,
        point: TrackPoint,
    },
    UpdateFlight(FlightSummary),
    RequestLiveStart {
        gateway_sn: String,
    },
}

struct OpenFlight {
    summary: FlightSummary,
    track: TrackBuilder,
    last_sample_ms: i64,
}

struct AircraftEntry {
    state: AircraftState,
    tracker: PhaseTracker,
    flight: Option<OpenFlight>,
    pending: Vec<TrackPoint>,
}

pub struct Situation {
    config: Config,
    next_flight_id: i64,
    gateways: BTreeMap<String, GatewayState>,
    aircraft: BTreeMap<String, AircraftEntry>,
    streams: BTreeMap<String, StreamState>,
    labels: HashMap<String, String>,
    broker: BrokerStatus,
    unknown: BTreeMap<String, u32>,
    invalid_messages: u32,
    raw_recording: Option<String>,
    system_dirty: bool,
    system_published_ms: i64,
}

impl Situation {
    pub fn new(config: Config, next_flight_id: i64, resume: Vec<ResumeFlight>) -> Self {
        let mut s = Self {
            config,
            next_flight_id,
            gateways: BTreeMap::new(),
            aircraft: BTreeMap::new(),
            streams: BTreeMap::new(),
            labels: HashMap::new(),
            broker: BrokerStatus::NotConfigured,
            unknown: BTreeMap::new(),
            invalid_messages: 0,
            raw_recording: None,
            system_dirty: false,
            system_published_ms: i64::MIN / 2,
        };
        for r in resume {
            let last_t = r
                .segments
                .iter()
                .flatten()
                .map(|p| p.t_ms)
                .max()
                .unwrap_or(r.flight.started_at_ms);
            let state = AircraftState {
                sn: r.flight.aircraft_sn.clone(),
                gateway_sn: r.flight.gateway_sn.clone(),
                last_seen_ms: last_t,
                phase: FlightPhase::Airborne,
                flight_id: Some(r.flight.id),
                telemetry: Telemetry::default(),
            };
            s.next_flight_id = s.next_flight_id.max(r.flight.id + 1);
            s.aircraft.insert(
                r.flight.aircraft_sn.clone(),
                AircraftEntry {
                    state,
                    tracker: PhaseTracker::new(FlightPhase::Airborne),
                    flight: Some(OpenFlight {
                        summary: r.flight,
                        track: TrackBuilder::from_segments(r.segments),
                        last_sample_ms: last_t,
                    }),
                    pending: Vec::new(),
                },
            );
        }
        s
    }

    pub fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    pub fn apply(&mut self, obs: Observed) -> Vec<Effect> {
        let at = obs.at_ms;
        let mut fx = Vec::new();
        match obs.observation {
            Observation::Topology {
                gateway_sn,
                aircraft_sn,
            } => {
                let g = self.gateway_entry(&gateway_sn, at);
                g.aircraft_sn = aircraft_sn;
                g.last_seen_ms = at;
                if g.aircraft_sn.is_none() {
                    g.video_sources.clear();
                }
                fx.push(Effect::Publish(LiveEvent::Gateway { gateway: g.clone() }));
            }
            Observation::GatewayTelemetry {
                gateway_sn,
                position,
                battery_percent,
                link,
            } => {
                let g = self.gateway_entry(&gateway_sn, at);
                g.position = position.or(g.position);
                g.battery_percent = battery_percent.or(g.battery_percent);
                g.link = link;
                g.last_seen_ms = at;
                fx.push(Effect::Publish(LiveEvent::Gateway { gateway: g.clone() }));
            }
            Observation::VideoSources {
                gateway_sn,
                sources,
            } => {
                let g = self.gateway_entry(&gateway_sn, at);
                g.video_sources = sources;
                fx.push(Effect::Publish(LiveEvent::Gateway { gateway: g.clone() }));
            }
            Observation::AircraftTelemetry {
                aircraft_sn,
                gateway_sn,
                telemetry,
                mode_class,
                periodic,
            } => {
                fx = self.on_aircraft(at, aircraft_sn, gateway_sn, telemetry, mode_class, periodic);
            }
            Observation::StreamStatus {
                gateway_sn,
                live,
                error,
            } => {
                let s = self.stream_entry(&gateway_sn, at);
                if stream::on_status(s, &live, error.as_deref(), at) {
                    fx.push(Effect::Publish(LiveEvent::Stream { stream: s.clone() }));
                }
            }
            Observation::CommandFinished {
                gateway_sn,
                command,
                result,
            } => {
                let s = self.stream_entry(&gateway_sn, at);
                if stream::on_finished(s, &command, &result, at) {
                    fx.push(Effect::Publish(LiveEvent::Stream { stream: s.clone() }));
                }
            }
            // Zustandswechsel von Broker und Mitschnitt gehen sofort raus, nur Zähler werden gedrosselt.
            Observation::Broker(status) => {
                self.broker = status;
                fx.push(self.publish_system(at));
            }
            Observation::Diagnostics {
                unknown_fields,
                invalid_messages,
            } => {
                for key in unknown_fields {
                    if let Some(count) = self.unknown.get_mut(&key) {
                        *count += 1;
                    } else if self.unknown.len() < MAX_UNKNOWN_FIELDS
                        && key.len() <= MAX_UNKNOWN_FIELD_CHARS
                    {
                        self.unknown.insert(key, 1);
                    }
                }
                self.invalid_messages += invalid_messages;
                self.system_dirty = true;
            }
            Observation::RawRecording(name) => {
                self.raw_recording = name;
                fx.push(self.publish_system(at));
            }
            Observation::Labels(labels) => {
                self.labels = labels.into_iter().collect();
                for g in self.gateways.values_mut() {
                    let label = self.labels.get(&g.sn).cloned();
                    if g.label != label {
                        g.label = label;
                        fx.push(Effect::Publish(LiveEvent::Gateway { gateway: g.clone() }));
                    }
                }
            }
        }
        fx
    }

    fn gateway_entry(&mut self, sn: &str, at: i64) -> &mut GatewayState {
        let label = self.labels.get(sn).cloned();
        self.gateways
            .entry(sn.to_string())
            .or_insert_with(|| GatewayState {
                sn: sn.into(),
                label,
                last_seen_ms: at,
                aircraft_sn: None,
                position: None,
                battery_percent: None,
                link: LinkQuality::default(),
                video_sources: vec![],
            })
    }

    fn stream_entry(&mut self, gateway_sn: &str, at: i64) -> &mut StreamState {
        self.streams
            .entry(gateway_sn.to_string())
            .or_insert_with(|| stream::idle(gateway_sn, at))
    }

    fn on_aircraft(
        &mut self,
        at: i64,
        sn: String,
        gateway_sn: String,
        telemetry: Telemetry,
        class: ModeClass,
        periodic: bool,
    ) -> Vec<Effect> {
        let mut fx = Vec::new();
        // Nach einem Serverneustart kommt kein update_topo – die Telemetrie verrät die Zuordnung.
        let g = self.gateway_entry(&gateway_sn, at);
        if g.aircraft_sn.as_deref() != Some(&sn) {
            g.aircraft_sn = Some(sn.clone());
            fx.push(Effect::Publish(LiveEvent::Gateway { gateway: g.clone() }));
        }
        let stream_busy = self
            .streams
            .get(&gateway_sn)
            .is_some_and(|s| matches!(s.phase, StreamPhase::Starting | StreamPhase::Live));
        let entry = self
            .aircraft
            .entry(sn.clone())
            .or_insert_with(|| AircraftEntry {
                state: AircraftState {
                    sn: sn.clone(),
                    gateway_sn: gateway_sn.clone(),
                    last_seen_ms: at,
                    phase: FlightPhase::Ground,
                    flight_id: None,
                    telemetry: Telemetry::default(),
                },
                tracker: PhaseTracker::new(FlightPhase::Ground),
                flight: None,
                pending: Vec::new(),
            });
        entry.state.gateway_sn = gateway_sn.clone();
        let point = telemetry.position.map(|p| TrackPoint {
            t_ms: at,
            lat: p.lat,
            lon: p.lon,
            elevation_m: telemetry.elevation_takeoff_m,
        });
        let sample = Sample {
            at_ms: at,
            class,
            elevation_m: telemetry.elevation_takeoff_m,
            vertical_speed_mps: telemetry.vertical_speed_mps,
        };
        // Der Übersetzer hat osd und state bereits gemergt; die Nachricht trägt den ganzen Stand.
        entry.state.telemetry = telemetry;

        // Nur der periodische Strom (osd) belegt Frische, Phase und Spur; eine seltene
        // state-Nachricht darf einen osd-Ausfall nicht überdecken.
        if periodic {
            entry.state.last_seen_ms = at;
            if let Some(p) = &point {
                entry.pending.push(p.clone());
                if entry.pending.len() > PENDING_POINTS {
                    entry.pending.remove(0);
                }
            }
            match entry.tracker.observe(&sample) {
                Some(Transition::TookOff { at_ms }) => {
                    let summary = FlightSummary {
                        id: self.next_flight_id,
                        aircraft_sn: sn,
                        gateway_sn: gateway_sn.clone(),
                        started_at_ms: at_ms,
                        ended_at_ms: None,
                        end_reason: None,
                        max_elevation_m: None,
                        distance_m: 0.0,
                    };
                    self.next_flight_id += 1;
                    fx.push(Effect::InsertFlight(summary.clone()));
                    fx.push(Effect::Publish(LiveEvent::FlightStarted {
                        flight: summary.clone(),
                    }));
                    let mut flight = OpenFlight {
                        summary,
                        track: TrackBuilder::new(),
                        last_sample_ms: at,
                    };
                    for p in entry.pending.drain(..).filter(|p| p.t_ms >= at_ms) {
                        push_point(&mut flight, p, &mut fx);
                    }
                    entry.flight = Some(flight);
                    if self.config.auto_live_on_takeoff && !stream_busy {
                        fx.push(Effect::RequestLiveStart { gateway_sn });
                    }
                }
                Some(Transition::Landed { at_ms }) => {
                    if let Some(flight) = entry.flight.take() {
                        let done = close(flight, at_ms, FlightEndReason::Landed);
                        fx.push(Effect::UpdateFlight(done.clone()));
                        fx.push(Effect::Publish(LiveEvent::FlightEnded { flight: done }));
                    }
                }
                None => {
                    if let Some(flight) = entry.flight.as_mut() {
                        // Auch eine Probe ohne Position hält den Flug offen.
                        flight.last_sample_ms = at;
                        if let Some(p) = point {
                            push_point(flight, p, &mut fx);
                        }
                    }
                }
            }
        }
        entry.state.phase = entry.tracker.phase();
        entry.state.flight_id = entry.flight.as_ref().map(|f| f.summary.id);
        fx.push(Effect::Publish(LiveEvent::Aircraft {
            aircraft: entry.state.clone(),
        }));
        fx
    }

    pub fn tick(&mut self, now_ms: i64) -> Vec<Effect> {
        let mut fx = Vec::new();
        for entry in self.aircraft.values_mut() {
            if let Some(flight) = entry
                .flight
                .take_if(|f| now_ms - f.last_sample_ms > FLIGHT_CLOSE_AFTER_MS)
            {
                let last = flight.last_sample_ms;
                let done = close(flight, last, FlightEndReason::ConnectionLost);
                entry.tracker = PhaseTracker::new(FlightPhase::Ground);
                entry.state.phase = FlightPhase::Ground;
                entry.state.flight_id = None;
                fx.push(Effect::UpdateFlight(done.clone()));
                fx.push(Effect::Publish(LiveEvent::FlightEnded { flight: done }));
                fx.push(Effect::Publish(LiveEvent::Aircraft {
                    aircraft: entry.state.clone(),
                }));
            }
        }
        if self.system_dirty && now_ms - self.system_published_ms >= SYSTEM_PUBLISH_EVERY_MS {
            fx.push(self.publish_system(now_ms));
        }
        fx
    }

    fn system(&self) -> SystemStatus {
        SystemStatus {
            broker: self.broker,
            unknown_fields: self
                .unknown
                .iter()
                .map(|(key, count)| UnknownFieldCount {
                    key: key.clone(),
                    count: *count,
                })
                .collect(),
            invalid_messages: self.invalid_messages,
            raw_recording: self.raw_recording.clone(),
        }
    }

    fn publish_system(&mut self, now_ms: i64) -> Effect {
        self.system_dirty = false;
        self.system_published_ms = now_ms;
        Effect::Publish(LiveEvent::System {
            system: self.system(),
        })
    }

    pub fn snapshot(&self, now_ms: i64) -> SituationSnapshot {
        SituationSnapshot {
            server_time_ms: now_ms,
            freshness: FreshnessThresholds {
                delayed_after_ms: FRESHNESS_DELAYED_MS,
                lost_after_ms: FRESHNESS_LOST_MS,
            },
            gateways: self.gateways.values().cloned().collect(),
            aircraft: self.aircraft.values().map(|a| a.state.clone()).collect(),
            active_flights: self
                .aircraft
                .values()
                .filter_map(|a| a.flight.as_ref())
                .map(|f| ActiveFlight {
                    flight: summary_now(f),
                    segments: f.track.segments().to_vec(),
                })
                .collect(),
            streams: self.streams.values().cloned().collect(),
            system: self.system(),
        }
    }

    /// Beginn eines Livebild-Befehls, geprüft und gesetzt in einem Schritt. `None`: abgelehnt,
    /// weil er nicht zum Zustand passt (siehe `stream::on_issued`).
    pub fn begin_command(
        &mut self,
        gateway_sn: &str,
        command: &StreamCommand,
        at: i64,
    ) -> Option<Vec<Effect>> {
        let s = self.stream_entry(gateway_sn, at);
        let changed = stream::on_issued(s, command, at)?;
        Some(if changed {
            vec![Effect::Publish(LiveEvent::Stream { stream: s.clone() })]
        } else {
            vec![]
        })
    }

    pub fn stream(&self, gateway_sn: &str) -> Option<&StreamState> {
        self.streams.get(gateway_sn)
    }

    pub fn gateway(&self, sn: &str) -> Option<&GatewayState> {
        self.gateways.get(sn)
    }
}

fn push_point(flight: &mut OpenFlight, p: TrackPoint, fx: &mut Vec<Effect>) {
    if let Some(appended) = flight.track.push(p) {
        let id = flight.summary.id;
        fx.push(Effect::AppendTrackPoint {
            flight_id: id,
            segment: appended.segment,
            point: appended.point.clone(),
        });
        fx.push(Effect::Publish(LiveEvent::TrackPoint {
            flight_id: id,
            segment: appended.segment,
            point: appended.point,
        }));
    }
}

/// Der Flug mit Strecke und Höchstwert aus der Spur; die gespeicherte Zusammenfassung kennt sie nur beim Start.
fn summary_now(f: &OpenFlight) -> FlightSummary {
    FlightSummary {
        distance_m: f.track.distance_m(),
        max_elevation_m: f.track.max_elevation_m(),
        ..f.summary.clone()
    }
}

fn close(f: OpenFlight, ended_at_ms: i64, reason: FlightEndReason) -> FlightSummary {
    FlightSummary {
        ended_at_ms: Some(ended_at_ms),
        end_reason: Some(reason),
        ..summary_now(&f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::situation::observation::LiveStreamInfo;

    const ON: Config = Config {
        auto_live_on_takeoff: true,
    };
    const OFF: Config = Config {
        auto_live_on_takeoff: false,
    };

    fn tel(at: i64, class: ModeClass, elevation: f64, lat: f64) -> Observed {
        Observed {
            at_ms: at,
            observation: Observation::AircraftTelemetry {
                aircraft_sn: "AC1".into(),
                gateway_sn: "RC1".into(),
                telemetry: Telemetry {
                    position: Some(GeoPoint { lat, lon: 10.0 }),
                    elevation_takeoff_m: Some(elevation),
                    ..Telemetry::default()
                },
                mode_class: class,
                periodic: true,
            },
        }
    }

    fn feed(s: &mut Situation, obs: Vec<Observed>) -> Vec<Effect> {
        obs.into_iter().flat_map(|o| s.apply(o)).collect()
    }

    fn fly(s: &mut Situation) -> Vec<Effect> {
        feed(
            s,
            vec![
                tel(0, ModeClass::Ground, 0.0, 52.000),
                tel(2_000, ModeClass::Air, 3.0, 52.001),
                tel(4_000, ModeClass::Air, 10.0, 52.002),
                tel(6_000, ModeClass::Air, 20.0, 52.003),
                tel(8_000, ModeClass::Ground, 0.0, 52.003),
                tel(10_000, ModeClass::Ground, 0.0, 52.003),
            ],
        )
    }

    fn updated_flight(effects: &[Effect]) -> Option<&FlightSummary> {
        effects.iter().find_map(|e| match e {
            Effect::UpdateFlight(f) => Some(f),
            _ => None,
        })
    }

    #[test]
    fn a_flight_is_detected_stored_and_published() {
        let mut s = Situation::new(OFF, 1, vec![]);
        let effects = fly(&mut s);
        let inserted: Vec<_> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::InsertFlight(f) => Some(f),
                _ => None,
            })
            .collect();
        assert_eq!(inserted.len(), 1);
        assert_eq!((inserted[0].id, inserted[0].started_at_ms), (1, 2_000));
        let points = effects
            .iter()
            .filter(|e| matches!(e, Effect::AppendTrackPoint { .. }))
            .count();
        assert_eq!(
            points, 4,
            "2 s, 4 s, 6 s und die Probe bei 8 s vor der Bestätigung der Landung"
        );
        let ended = effects
            .iter()
            .find_map(|e| match e {
                Effect::Publish(LiveEvent::FlightEnded { flight }) => Some(flight),
                _ => None,
            })
            .unwrap();
        assert_eq!(ended.ended_at_ms, Some(8_000));
        assert_eq!(ended.end_reason, Some(FlightEndReason::Landed));
        assert!(ended.distance_m > 200.0);
        assert!(effects.contains(&Effect::UpdateFlight(ended.clone())));
        assert!(s.snapshot(10_000).active_flights.is_empty());
    }

    #[test]
    fn takeoff_requests_live_video_only_when_enabled_and_idle() {
        let mut on = Situation::new(ON, 1, vec![]);
        assert!(fly(&mut on).contains(&Effect::RequestLiveStart {
            gateway_sn: "RC1".into()
        }));
        let mut off = Situation::new(OFF, 1, vec![]);
        assert!(
            !fly(&mut off)
                .iter()
                .any(|e| matches!(e, Effect::RequestLiveStart { .. }))
        );

        let mut live = Situation::new(ON, 1, vec![]);
        live.apply(Observed {
            at_ms: 0,
            observation: Observation::StreamStatus {
                gateway_sn: "RC1".into(),
                live: Some(LiveStreamInfo {
                    video_id: "v".into(),
                    lens: None,
                }),
                error: None,
            },
        });
        assert!(
            !fly(&mut live)
                .iter()
                .any(|e| matches!(e, Effect::RequestLiveStart { .. })),
            "läuft schon"
        );
    }

    #[test]
    fn resumed_flight_continues_without_a_second_insert() {
        let open = ResumeFlight {
            flight: FlightSummary {
                id: 41,
                aircraft_sn: "AC1".into(),
                gateway_sn: "RC1".into(),
                started_at_ms: 0,
                ended_at_ms: None,
                end_reason: None,
                max_elevation_m: Some(20.0),
                distance_m: 0.0,
            },
            segments: vec![vec![TrackPoint {
                t_ms: 0,
                lat: 52.0,
                lon: 10.0,
                elevation_m: Some(20.0),
            }]],
        };
        let mut s = Situation::new(OFF, 42, vec![open]);
        let effects = feed(
            &mut s,
            vec![
                tel(60_000, ModeClass::Air, 30.0, 52.01),
                tel(62_000, ModeClass::Air, 30.0, 52.011),
            ],
        );
        assert!(!effects.iter().any(|e| matches!(e, Effect::InsertFlight(_))));
        let segments: Vec<u32> = effects
            .iter()
            .filter_map(|e| match e {
                Effect::AppendTrackPoint {
                    flight_id: 41,
                    segment,
                    ..
                } => Some(*segment),
                _ => None,
            })
            .collect();
        assert_eq!(
            segments,
            vec![1, 1],
            "nach der Neustartlücke ein neues Segment"
        );
        assert_eq!(s.snapshot(62_000).aircraft[0].flight_id, Some(41));
    }

    #[test]
    fn silence_closes_the_flight_as_connection_lost() {
        let mut s = Situation::new(OFF, 1, vec![]);
        feed(
            &mut s,
            vec![
                tel(0, ModeClass::Air, 5.0, 52.0),
                tel(2_000, ModeClass::Air, 6.0, 52.001),
            ],
        );
        assert!(
            updated_flight(&s.tick(2_000 + FLIGHT_CLOSE_AFTER_MS)).is_none(),
            "genau an der Grenze noch offen"
        );
        let effects = s.tick(2_000 + FLIGHT_CLOSE_AFTER_MS + 1);
        let ended = updated_flight(&effects).unwrap();
        assert_eq!(ended.end_reason, Some(FlightEndReason::ConnectionLost));
        assert_eq!(ended.ended_at_ms, Some(2_000));
    }

    #[test]
    fn samples_without_position_keep_the_flight_open() {
        let mut s = Situation::new(OFF, 1, vec![]);
        feed(
            &mut s,
            vec![
                tel(0, ModeClass::Air, 5.0, 52.0),
                tel(2_000, ModeClass::Air, 6.0, 52.001),
            ],
        );
        let mut no_fix = tel(400_000, ModeClass::Air, 6.0, 52.0);
        if let Observation::AircraftTelemetry { telemetry, .. } = &mut no_fix.observation {
            telemetry.position = None;
        }
        s.apply(no_fix);
        assert!(
            updated_flight(&s.tick(2_000 + FLIGHT_CLOSE_AFTER_MS + 1)).is_none(),
            "Probe ohne GPS ist trotzdem eine Probe"
        );
        let effects = s.tick(400_000 + FLIGHT_CLOSE_AFTER_MS + 1);
        assert_eq!(
            updated_flight(&effects).unwrap().ended_at_ms,
            Some(400_000),
            "Ende ist die letzte Probe, nicht der letzte Spurpunkt"
        );
    }

    #[test]
    fn aircraft_off_in_topology_keeps_the_flight_open() {
        let mut s = Situation::new(OFF, 1, vec![]);
        feed(
            &mut s,
            vec![
                tel(0, ModeClass::Air, 5.0, 52.0),
                tel(2_000, ModeClass::Air, 6.0, 52.001),
            ],
        );
        let effects = s.apply(Observed {
            at_ms: 3_000,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: None,
            },
        });
        assert!(updated_flight(&effects).is_none());
        assert_eq!(s.snapshot(3_000).active_flights.len(), 1);
        assert_eq!(s.snapshot(3_000).gateways[0].aircraft_sn, None);
    }

    #[test]
    fn diagnostics_are_aggregated_and_published_throttled() {
        let mut s = Situation::new(OFF, 1, vec![]);
        let diag = |at| Observed {
            at_ms: at,
            observation: Observation::Diagnostics {
                unknown_fields: vec!["aircraft.neu".into()],
                invalid_messages: 1,
            },
        };
        assert!(s.apply(diag(0)).is_empty());
        s.apply(diag(1));
        let first = s.tick(1_000);
        assert_eq!(first.len(), 1);
        assert!(s.tick(2_000).is_empty(), "nichts Neues");
        s.apply(diag(2_500));
        assert!(s.tick(3_000).is_empty(), "gedrosselt");
        let published = s.tick(6_000);
        let [Effect::Publish(LiveEvent::System { system })] = &published[..] else {
            panic!("genau ein System-Ereignis: {published:?}")
        };
        assert_eq!(
            system.unknown_fields,
            vec![UnknownFieldCount {
                key: "aircraft.neu".into(),
                count: 3
            }]
        );
        assert_eq!(system.invalid_messages, 3);
    }

    #[test]
    fn unknown_field_names_stay_bounded() {
        let mut s = Situation::new(OFF, 1, vec![]);
        let mut fields: Vec<String> = (0..MAX_UNKNOWN_FIELDS + 50)
            .map(|i| format!("aircraft.f{i}"))
            .collect();
        fields.push("x".repeat(MAX_UNKNOWN_FIELD_CHARS + 1));
        s.apply(Observed {
            at_ms: 0,
            observation: Observation::Diagnostics {
                unknown_fields: fields,
                invalid_messages: 0,
            },
        });
        assert_eq!(s.unknown.len(), MAX_UNKNOWN_FIELDS);
    }

    #[test]
    fn stream_commands_flow_into_state_and_events() {
        let mut s = Situation::new(OFF, 1, vec![]);
        let cmd = StreamCommand::Start {
            video_id: "v".into(),
        };
        let e = s.begin_command("RC1", &cmd, 1).unwrap();
        assert!(
            s.begin_command("RC1", &cmd, 1).is_none(),
            "zweiter Start abgelehnt"
        );
        assert!(
            matches!(&e[..], [Effect::Publish(LiveEvent::Stream { stream })] if stream.phase == StreamPhase::Starting)
        );
        s.apply(Observed {
            at_ms: 2,
            observation: Observation::CommandFinished {
                gateway_sn: "RC1".into(),
                command: cmd,
                result: Err("nein".into()),
            },
        });
        assert_eq!(s.stream("RC1").unwrap().phase, StreamPhase::Failed);
    }

    #[test]
    fn state_messages_do_not_refresh_aircraft_freshness() {
        let mut s = Situation::new(OFF, 1, vec![]);
        s.apply(tel(0, ModeClass::Ground, 0.0, 52.0));
        let mut state_only = tel(20_000, ModeClass::Unknown, 0.0, 52.0);
        if let Observation::AircraftTelemetry { periodic, .. } = &mut state_only.observation {
            *periodic = false;
        }
        s.apply(state_only);
        assert_eq!(s.snapshot(20_000).aircraft[0].last_seen_ms, 0);
    }

    #[test]
    fn labels_and_topology_build_the_gateway() {
        let mut s = Situation::new(OFF, 1, vec![]);
        s.apply(Observed {
            at_ms: 0,
            observation: Observation::Labels(vec![("RC1".into(), "RC Plus 1".into())]),
        });
        s.apply(Observed {
            at_ms: 1,
            observation: Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: Some("AC1".into()),
            },
        });
        let g = s.gateway("RC1").unwrap();
        assert_eq!(
            (g.label.as_deref(), g.aircraft_sn.as_deref(), g.last_seen_ms),
            (Some("RC Plus 1"), Some("AC1"), 1)
        );
    }
}

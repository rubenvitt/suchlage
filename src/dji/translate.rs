//! Übersetzt DJI-MQTT in `Observation`s. Zustandsbehaftet nur, weil DJI Teil-Updates schickt
//! (Referenz Kap. 1): je Gerät wird der letzte Stand gehalten und jede Nachricht hineingemergt.

use std::collections::{HashMap, HashSet};

use serde_json::{Map, Value, json};

use crate::contract::{
    Battery, FlightMode, GeoPoint, Gnss, Lens, LinkQuality, RtkFix, Telemetry, VideoSource,
};
use crate::situation::observation::{LiveStreamInfo, ModeClass, Observation, Observed};

use super::topic::{self, DjiTopic};
use super::wire::{self, Envelope, LiveCapacity, LiveStatusEntry, ServiceReply, UpdateTopo};

#[derive(Debug, Clone, PartialEq)]
pub struct Outgoing {
    pub topic: String,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CommandReply {
    pub gateway_sn: String,
    pub tid: String,
    pub method: String,
    pub result: i64,
}

#[derive(Debug, Default, PartialEq)]
pub struct Translated {
    pub observations: Vec<Observed>,
    pub reply: Option<Outgoing>,
    pub command_reply: Option<CommandReply>,
}

#[derive(Default)]
pub struct Translator {
    merged: HashMap<String, Map<String, Value>>,
    gateways: HashSet<String>,
    aircraft_gateway: HashMap<String, String>,
}

pub fn mode_class(code: u8) -> ModeClass {
    match code {
        0..=2 => ModeClass::Ground,
        3..=12 | 15..=18 => ModeClass::Air,
        _ => ModeClass::Unknown,
    }
}

pub fn mode_label(code: u8) -> &'static str {
    match code {
        0 => "Bereitschaft",
        1 => "Startvorbereitung",
        2 => "startbereit",
        3 => "manueller Flug",
        4 => "automatischer Start",
        5 => "Wegpunktflug",
        6 => "Panoramaaufnahme",
        7 => "Verfolgung",
        8 => "ADS-B-Ausweichen",
        9 => "Rückkehr zum Startpunkt",
        10 => "automatische Landung",
        11 => "Zwangslandung",
        12 => "Landung (Three-blade)",
        13 => "Firmware-Update",
        14 => "nicht verbunden",
        15 => "Hinderniserkennung (APAS)",
        16 => "Virtual Stick",
        17 => "Live-Flugsteuerung",
        18 => "RTK-Fixing in der Luft",
        _ => "unbekannter Modus",
    }
}

/// Kameraname aus dem `type` im Kameraindex (Referenz Kap. 0).
pub fn camera_label(camera_index: &str) -> String {
    match camera_index.split('-').next().unwrap_or("") {
        "39" => "FPV".into(),
        "42" => "H20".into(),
        "43" => "H20T".into(),
        "61" => "H20N".into(),
        "82" => "H30".into(),
        "83" => "H30T".into(),
        other => format!("Kamera {other}"),
    }
}

/// Livestream-Fehlercodes (Referenz Kap. 5, `71.error-code.md`) als deutscher Satz.
pub fn live_error_text(code: i64) -> String {
    let reason = match code {
        513002 => "Kamera existiert nicht oder Kameratyp falsch",
        513003 => "Livebild läuft bereits",
        513005 => "Qualitätseinstellung falsch",
        513006 => "Start fehlgeschlagen",
        513008 => "Bildübertragungsdaten fehlerhaft",
        513010 => "Gerät ohne Netz",
        513011 => "Livestream ist in Pilot 2 nicht aktiviert",
        513012 => "Linsenwechsel während des Streams nicht möglich",
        513013 => "Protokoll nicht unterstützt",
        513014 => "Parameter falsch oder unvollständig",
        513015 => "Netz zu langsam",
        513016 => "Dekodierung fehlgeschlagen – ist die Flugansicht in Pilot 2 offen?",
        513017 => "pausiert während eines Datei-Downloads",
        513099 => "allgemeiner Fehler",
        _ => "unbekannter Fehler",
    };
    format!("Pilot 2 lehnt ab: {reason} ({code}).")
}

impl Translator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn handle(&mut self, at_ms: i64, topic: &str, payload: &[u8]) -> Translated {
        let mut out = Translated::default();
        let parsed = topic::parse(topic);
        if parsed == DjiTopic::Other {
            return out;
        }
        let Ok(env) = serde_json::from_slice::<Envelope>(payload) else {
            out.observations.push(diag(at_ms, vec![], 1));
            return out;
        };
        match parsed {
            DjiTopic::Status { gateway } => self.on_status(at_ms, gateway, env, &mut out),
            DjiTopic::Osd { sn } => self.on_props(at_ms, sn, env, true, &mut out),
            DjiTopic::State { sn } => self.on_props(at_ms, sn, env, false, &mut out),
            DjiTopic::ServicesReply { gateway } => {
                if let (Some(tid), Some(method), Ok(reply)) = (
                    env.tid,
                    env.method,
                    serde_json::from_value::<ServiceReply>(env.data),
                ) {
                    out.command_reply = Some(CommandReply {
                        gateway_sn: gateway.into(),
                        tid,
                        method,
                        result: reply.result,
                    });
                } else {
                    out.observations.push(diag(at_ms, vec![], 1));
                }
            }
            // Für M350/RC Plus nicht dokumentiert (Referenz Kap. 0); bewusst ignoriert.
            DjiTopic::Events { .. } | DjiTopic::Requests { .. } | DjiTopic::Other => {}
        }
        out
    }

    fn on_status(&mut self, at_ms: i64, gateway: &str, env: Envelope, out: &mut Translated) {
        if env.method.as_deref() != Some("update_topo") {
            return;
        }
        let Ok(topo) = serde_json::from_value::<UpdateTopo>(env.data) else {
            out.observations.push(diag(at_ms, vec![], 1));
            return;
        };
        // update_topo ist maßgeblich: Die eigene SN ist Fernsteuerung, die Unterknoten sind keine.
        self.gateways.insert(gateway.to_string());
        self.aircraft_gateway.remove(gateway);
        for device in &topo.sub_devices {
            self.gateways.remove(&device.sn);
        }
        let aircraft = topo
            .sub_devices
            .iter()
            .find(|d| d.kind == Some(wire::AIRCRAFT_TYPE) || d.kind.is_none())
            .map(|d| d.sn.clone())
            .filter(|sn| topic::is_valid_sn(sn));
        if let Some(sn) = &aircraft {
            self.aircraft_gateway
                .insert(sn.clone(), gateway.to_string());
        }
        let body = json!({
            "tid": env.tid,
            "bid": env.bid,
            "method": "update_topo",
            "timestamp": at_ms,
            "data": { "result": 0 },
        });
        out.reply = Some(Outgoing {
            topic: topic::status_reply(gateway),
            payload: body.to_string().into_bytes(),
        });
        out.observations.push(Observed {
            at_ms,
            observation: Observation::Topology {
                gateway_sn: gateway.into(),
                aircraft_sn: aircraft,
            },
        });
    }

    fn on_props(
        &mut self,
        at_ms: i64,
        sn: &str,
        env: Envelope,
        periodic: bool,
        out: &mut Translated,
    ) {
        let Value::Object(incoming) = env.data else {
            out.observations.push(diag(at_ms, vec![], 1));
            return;
        };
        // Ein bekanntes Fluggerät wird nie zur Fernsteuerung, auch nicht durch eine Nachricht mit
        // `gateway` = eigene SN; sonst bliebe es das bis zum nächsten update_topo und darüber hinaus.
        let is_gateway = !self.aircraft_gateway.contains_key(sn)
            && (env.gateway.as_deref() == Some(sn) || self.gateways.contains(sn));
        let (prefix, documented) = if is_gateway {
            ("gateway", wire::GATEWAY_DOCUMENTED)
        } else {
            ("aircraft", wire::AIRCRAFT_DOCUMENTED)
        };
        let unknown: Vec<String> = incoming
            .keys()
            .filter(|k| !documented.contains(&k.as_str()) && !wire::is_payload_key(k))
            .map(|k| format!("{prefix}.{k}"))
            .collect();
        let merged = self.merged.entry(sn.to_string()).or_default();
        deep_merge(merged, &incoming);
        let merged = merged.clone();
        // Gültigkeit an der eingehenden Nachricht prüfen, damit ein falscher Wert genau einmal zählt;
        // die Werte selbst kommen aus dem gemergten Stand.
        let mut invalid = false;
        let mut ignored = false;

        if is_gateway {
            self.gateways.insert(sn.to_string());
            gateway_values(&incoming, &mut invalid);
            let (position, battery_percent, link) = gateway_values(&merged, &mut ignored);
            out.observations.push(Observed {
                at_ms,
                observation: Observation::GatewayTelemetry {
                    gateway_sn: sn.into(),
                    position,
                    battery_percent,
                    link,
                },
            });
            if let Some(cap) = incoming.get("live_capacity") {
                match serde_json::from_value::<LiveCapacity>(cap.clone()) {
                    Ok(cap) => out.observations.push(Observed {
                        at_ms,
                        observation: Observation::VideoSources {
                            gateway_sn: sn.into(),
                            sources: video_sources(&cap),
                        },
                    }),
                    Err(_) => invalid = true,
                }
            }
            if let Some(status) = incoming.get("live_status") {
                match serde_json::from_value::<Vec<LiveStatusEntry>>(status.clone()) {
                    Ok(entries) => {
                        let (live, error) = stream_status(&entries);
                        out.observations.push(Observed {
                            at_ms,
                            observation: Observation::StreamStatus {
                                gateway_sn: sn.into(),
                                live,
                                error,
                            },
                        });
                    }
                    Err(_) => invalid = true,
                }
            }
        } else {
            let gateway = env
                .gateway
                .clone()
                .filter(|g| topic::is_valid_sn(g))
                .or_else(|| self.aircraft_gateway.get(sn).cloned());
            if let Some(g) = &gateway {
                self.aircraft_gateway.insert(sn.to_string(), g.clone());
            }
            match gateway {
                Some(gateway_sn) => {
                    telemetry(&incoming, &mut invalid);
                    let (telemetry, mode_code) = telemetry(&merged, &mut ignored);
                    out.observations.push(Observed {
                        at_ms,
                        observation: Observation::AircraftTelemetry {
                            aircraft_sn: sn.into(),
                            gateway_sn,
                            telemetry,
                            mode_class: mode_code.map_or(ModeClass::Unknown, mode_class),
                            periodic,
                        },
                    });
                }
                None => invalid = true,
            }
        }
        if !unknown.is_empty() || invalid {
            out.observations
                .push(diag(at_ms, unknown, u32::from(invalid)));
        }
    }

    pub fn persisted(&self, sn: &str) -> Option<(String, Map<String, Value>)> {
        let gateway = if self.gateways.contains(sn) {
            sn.to_string()
        } else {
            self.aircraft_gateway.get(sn)?.clone()
        };
        Some((gateway, self.merged.get(sn)?.clone()))
    }

    pub fn restore(
        &mut self,
        at_ms: i64,
        sn: &str,
        gateway_sn: &str,
        data: Map<String, Value>,
    ) -> Translated {
        let payload = json!({ "gateway": gateway_sn, "data": data }).to_string();
        let mut out = self.handle(
            at_ms,
            &format!("thing/product/{sn}/state"),
            payload.as_bytes(),
        );
        // Was beim ersten Empfang schon gezählt wurde, zählt nach dem Neustart nicht noch einmal.
        out.observations
            .retain(|o| !matches!(o.observation, Observation::Diagnostics { .. }));
        out
    }
}

fn gateway_values(
    m: &Map<String, Value>,
    invalid: &mut bool,
) -> (Option<GeoPoint>, Option<u8>, LinkQuality) {
    let position = geo(m, "latitude", "longitude", invalid);
    let link = m
        .get("wireless_link")
        .and_then(Value::as_object)
        .map(|w| LinkQuality {
            sdr_quality: ok(wire::int(w, "sdr_quality"), invalid),
            lte_quality: ok(wire::int(w, "4g_quality"), invalid),
        });
    (
        position,
        ok(wire::int(m, "capacity_percent"), invalid),
        link.unwrap_or_default(),
    )
}

fn diag(at_ms: i64, unknown_fields: Vec<String>, invalid_messages: u32) -> Observed {
    Observed {
        at_ms,
        observation: Observation::Diagnostics {
            unknown_fields,
            invalid_messages,
        },
    }
}

fn ok<T>(r: Result<Option<T>, wire::InvalidNumber>, invalid: &mut bool) -> Option<T> {
    r.unwrap_or_else(|wire::InvalidNumber| {
        *invalid = true;
        None
    })
}

fn deep_merge(target: &mut Map<String, Value>, patch: &Map<String, Value>) {
    for (k, v) in patch {
        match (target.get_mut(k), v) {
            (Some(Value::Object(t)), Value::Object(p)) => deep_merge(t, p),
            _ => {
                target.insert(k.clone(), v.clone());
            }
        }
    }
}

fn geo(obj: &Map<String, Value>, lat: &str, lon: &str, invalid: &mut bool) -> Option<GeoPoint> {
    // Beide Werte prüfen, bevor `?` greift: Sonst bliebe ein falscher Längengrad ungezählt.
    let la = ok(wire::number(obj, lat), invalid);
    let lo = ok(wire::number(obj, lon), invalid);
    let (la, lo) = (la?, lo?);
    // DJI meldet vor dem GNSS-Fix 0/0; das ist keine Position.
    let in_range =
        (-90.0..=90.0).contains(&la) && (-180.0..=180.0).contains(&lo) && !(la == 0.0 && lo == 0.0);
    in_range.then_some(GeoPoint { lat: la, lon: lo })
}

fn telemetry(m: &Map<String, Value>, invalid: &mut bool) -> (Telemetry, Option<u8>) {
    let mode_code: Option<u8> = ok(wire::int(m, "mode_code"), invalid);
    let battery = m
        .get("battery")
        .and_then(Value::as_object)
        .map(|b| Battery {
            percent: ok(wire::int(b, "capacity_percent"), invalid),
            remaining_flight_s: ok(wire::int(b, "remain_flight_time"), invalid),
            return_home_percent: ok(wire::int(b, "return_home_power"), invalid),
        });
    let gnss = m
        .get("position_state")
        .and_then(Value::as_object)
        .map(|p| Gnss {
            fix: ok(wire::int::<u8>(p, "is_fixed"), invalid).and_then(|v| match v {
                0 => Some(RtkFix::NotStarted),
                1 => Some(RtkFix::Fixing),
                2 => Some(RtkFix::Fixed),
                3 => Some(RtkFix::Failed),
                _ => None,
            }),
            quality: ok(wire::int(p, "quality"), invalid),
            gps_satellites: ok(wire::int(p, "gps_number"), invalid),
            rtk_satellites: ok(wire::int(p, "rtk_number"), invalid),
        });
    let t = Telemetry {
        position: geo(m, "latitude", "longitude", invalid),
        height_ellipsoid_m: ok(wire::number(m, "height"), invalid),
        elevation_takeoff_m: ok(wire::number(m, "elevation"), invalid),
        home_distance_m: ok(wire::number(m, "home_distance"), invalid),
        horizontal_speed_mps: ok(wire::number(m, "horizontal_speed"), invalid),
        vertical_speed_mps: ok(wire::number(m, "vertical_speed"), invalid),
        heading_deg: ok(wire::number(m, "attitude_head"), invalid).map(|h| (h + 360.0) % 360.0),
        battery,
        gnss,
        mode: mode_code.map(|code| FlightMode {
            code,
            label: mode_label(code).into(),
        }),
        home: geo(m, "home_latitude", "home_longitude", invalid),
    };
    (t, mode_code)
}

fn parse_lens(video_type: &str) -> Lens {
    match video_type {
        "wide" => Lens::Wide,
        "zoom" => Lens::Zoom,
        // Kamera-Einstellungen nennen Wärmebild „ir“, live_lens_change „thermal“ (Referenz 5.4).
        "thermal" | "ir" => Lens::Thermal,
        _ => Lens::Normal,
    }
}

/// Laufender Stream laut Gerät; ein Fehlercode zählt nur, solange kein Stream läuft.
fn stream_status(entries: &[LiveStatusEntry]) -> (Option<LiveStreamInfo>, Option<String>) {
    let live = entries
        .iter()
        .find(|e| e.status == Some(1))
        .map(|e| LiveStreamInfo {
            video_id: e.video_id.clone(),
            lens: e.video_type.as_deref().map(parse_lens),
        });
    let error = if live.is_some() {
        None
    } else {
        entries
            .iter()
            .find_map(|e| e.error_status.filter(|c| *c != 0))
            .map(live_error_text)
    };
    (live, error)
}

fn video_sources(cap: &LiveCapacity) -> Vec<VideoSource> {
    let mut out = Vec::new();
    for device in &cap.device_list {
        for camera in &device.camera_list {
            for video in &camera.video_list {
                out.push(VideoSource {
                    // Alle drei Teile stammen aus live_capacity (Referenz 5.1).
                    video_id: format!(
                        "{}/{}/{}",
                        device.sn, camera.camera_index, video.video_index
                    ),
                    camera_index: camera.camera_index.clone(),
                    camera_label: camera_label(&camera.camera_index),
                    lens: video.video_type.as_deref().map_or(Lens::Normal, parse_lens),
                    switchable_lenses: video
                        .switchable_video_types
                        .iter()
                        .map(|v| parse_lens(v))
                        .collect(),
                });
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::examples::{AIRCRAFT_SN, GATEWAY_SN};
    use crate::dji::record::read_lines;
    use crate::sim::synth::FIXTURE_PATH;

    fn run(t: &mut Translator, at: i64, topic: &str, v: Value) -> Translated {
        t.handle(at, topic, v.to_string().as_bytes())
    }

    fn aircraft_telemetry(tr: &Translated) -> Option<&Telemetry> {
        tr.observations.iter().find_map(|o| match &o.observation {
            Observation::AircraftTelemetry { telemetry, .. } => Some(telemetry),
            _ => None,
        })
    }

    #[test]
    fn update_topo_is_answered_and_reported() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            5,
            "sys/product/RC1/status",
            json!({
                "tid": "t1", "bid": "b1", "method": "update_topo", "timestamp": 1,
                "data": {"type": 119, "sub_type": 0, "sub_devices": [{"sn": "AC1", "type": 89, "sub_type": 0}]}
            }),
        );
        let reply = tr.reply.unwrap();
        assert_eq!(reply.topic, "sys/product/RC1/status_reply");
        let body: Value = serde_json::from_slice(&reply.payload).unwrap();
        assert_eq!(
            body,
            json!({"tid": "t1", "bid": "b1", "method": "update_topo", "timestamp": 5, "data": {"result": 0}})
        );
        assert_eq!(
            tr.observations[0].observation,
            Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: Some("AC1".into())
            }
        );
    }

    #[test]
    fn empty_sub_devices_means_aircraft_off() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "sys/product/RC1/status",
            json!({"tid": "t", "bid": "b", "method": "update_topo", "data": {"sub_devices": []}}),
        );
        assert_eq!(
            tr.observations[0].observation,
            Observation::Topology {
                gateway_sn: "RC1".into(),
                aircraft_sn: None
            }
        );
    }

    #[test]
    fn partial_osd_updates_are_merged() {
        let mut t = Translator::new();
        run(
            &mut t,
            0,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.0, "longitude": 10.0, "mode_code": 3, "battery": {"capacity_percent": 70}}}),
        );
        let tr = run(
            &mut t,
            2_000,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.001, "attitude_head": -90}}),
        );
        let tel = aircraft_telemetry(&tr).unwrap();
        assert_eq!(
            tel.position,
            Some(GeoPoint {
                lat: 52.001,
                lon: 10.0
            })
        );
        assert_eq!(
            tel.battery.as_ref().unwrap().percent,
            Some(70),
            "Akku bleibt aus der ersten Nachricht"
        );
        assert_eq!(
            tel.heading_deg,
            Some(270.0),
            "Kompasskurs = (x + 360) % 360"
        );
        assert_eq!(tel.mode.as_ref().unwrap().label, "manueller Flug");
    }

    #[test]
    fn state_message_is_not_periodic_and_carries_home_point() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "thing/product/AC1/state",
            json!({"gateway": "RC1", "data": {"home_latitude": 52.0, "home_longitude": 10.0}}),
        );
        match &tr.observations[0].observation {
            Observation::AircraftTelemetry {
                periodic,
                telemetry,
                gateway_sn,
                ..
            } => {
                assert!(!periodic);
                assert_eq!(gateway_sn, "RC1");
                assert_eq!(
                    telemetry.home,
                    Some(GeoPoint {
                        lat: 52.0,
                        lon: 10.0
                    })
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn rc_messages_yield_gateway_telemetry_video_sources_and_stream_status() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "thing/product/RC1/state",
            json!({"gateway": "RC1", "data": {
                "live_capacity": {"device_list": [{"sn": "AC1", "camera_list": [{"camera_index": "61-0-0", "video_list": [{"video_index": "normal-0", "video_type": "wide", "switchable_video_types": ["wide", "thermal"]}]}]}]},
                "live_status": [{"video_id": "AC1/61-0-0/normal-0", "video_type": "thermal", "status": 1}]
            }}),
        );
        let kinds: Vec<_> = tr
            .observations
            .iter()
            .map(|o| std::mem::discriminant(&o.observation))
            .collect();
        assert_eq!(kinds.len(), 3);
        let sources = tr
            .observations
            .iter()
            .find_map(|o| match &o.observation {
                Observation::VideoSources { sources, .. } => Some(sources.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(sources[0].video_id, "AC1/61-0-0/normal-0");
        assert_eq!(sources[0].camera_label, "H20N");
        assert_eq!(
            sources[0].switchable_lenses,
            vec![Lens::Wide, Lens::Thermal]
        );
        assert!(tr.observations.iter().any(|o| o.observation
            == Observation::StreamStatus {
                gateway_sn: "RC1".into(),
                live: Some(LiveStreamInfo {
                    video_id: "AC1/61-0-0/normal-0".into(),
                    lens: Some(Lens::Thermal)
                }),
                error: None,
            }));
    }

    #[test]
    fn device_reported_stream_error_becomes_a_german_sentence() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "thing/product/RC1/state",
            json!({"gateway": "RC1", "data": {
                "live_status": [{"video_id": "AC1/61-0-0/normal-0", "status": 0, "error_status": 513016}]
            }}),
        );
        let error = tr.observations.iter().find_map(|o| match &o.observation {
            Observation::StreamStatus {
                live: None, error, ..
            } => error.clone(),
            _ => None,
        });
        assert!(error.unwrap().contains("513016"));
        assert_eq!(
            live_error_text(513011),
            "Pilot 2 lehnt ab: Livestream ist in Pilot 2 nicht aktiviert (513011)."
        );
        assert!(live_error_text(513099).contains("allgemeiner Fehler (513099)"));
        assert!(live_error_text(1).contains("unbekannter Fehler (1)"));
    }

    #[test]
    fn restored_state_yields_video_sources_and_home_without_diagnostics() {
        let mut first = Translator::new();
        run(
            &mut first,
            0,
            "thing/product/RC1/state",
            json!({"gateway": "RC1", "data": {
                "live_capacity": {"device_list": [{"sn": "AC1", "camera_list": [{"camera_index": "61-0-0", "video_list": [{"video_index": "normal-0", "video_type": "wide"}]}]}]},
                "neu_bei_dji": 1
            }}),
        );
        run(
            &mut first,
            1,
            "thing/product/AC1/state",
            json!({"gateway": "RC1", "data": {"home_latitude": 52.0, "home_longitude": 10.0}}),
        );
        let (rc_gw, rc_data) = first.persisted("RC1").unwrap();
        let (ac_gw, ac_data) = first.persisted("AC1").unwrap();
        assert_eq!((rc_gw.as_str(), ac_gw.as_str()), ("RC1", "RC1"));

        // Neuer Prozess: der Stand kommt aus der DB zurück.
        let mut second = Translator::new();
        let rc = second.restore(5, "RC1", &rc_gw, rc_data);
        let ac = second.restore(5, "AC1", &ac_gw, ac_data);
        assert!(rc.observations.iter().any(|o| matches!(&o.observation, Observation::VideoSources { sources, .. } if sources.len() == 1)));
        assert_eq!(
            aircraft_telemetry(&ac).unwrap().home,
            Some(GeoPoint {
                lat: 52.0,
                lon: 10.0
            })
        );
        assert!(
            !rc.observations
                .iter()
                .chain(&ac.observations)
                .any(|o| matches!(o.observation, Observation::Diagnostics { .. })),
            "nichts doppelt zählen"
        );
        let tr = run(
            &mut second,
            6,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.1, "longitude": 10.1}}),
        );
        assert_eq!(
            aircraft_telemetry(&tr).unwrap().home,
            Some(GeoPoint {
                lat: 52.0,
                lon: 10.0
            }),
            "osd mergt in den eingespielten Stand"
        );
    }

    #[test]
    fn unknown_fields_are_counted_not_fatal() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.0, "neu_bei_dji": 1, "61-0-0": {"gimbal_pitch": -90}}}),
        );
        assert!(tr.observations.iter().any(|o| o.observation
            == Observation::Diagnostics {
                unknown_fields: vec!["aircraft.neu_bei_dji".into()],
                invalid_messages: 0
            }));
        assert!(aircraft_telemetry(&tr).is_some());
    }

    #[test]
    fn invalid_payload_and_wrong_types_are_counted() {
        let mut t = Translator::new();
        let tr = t.handle(0, "thing/product/AC1/osd", b"kein json");
        assert_eq!(tr.observations.len(), 1);
        assert_eq!(
            tr.observations[0].observation,
            Observation::Diagnostics {
                unknown_fields: vec![],
                invalid_messages: 1
            }
        );
        let tr = run(
            &mut t,
            1,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": "x", "longitude": 10.0}}),
        );
        assert!(tr.observations.iter().any(|o| matches!(
            o.observation,
            Observation::Diagnostics {
                invalid_messages: 1,
                ..
            }
        )));
        assert_eq!(
            aircraft_telemetry(&tr).unwrap().position,
            None,
            "halbe Position wird verworfen"
        );
        let tr = run(
            &mut t,
            2,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"longitude": 10.1}}),
        );
        assert!(
            !tr.observations
                .iter()
                .any(|o| matches!(o.observation, Observation::Diagnostics { .. })),
            "der alte Fehler zählt nicht noch einmal"
        );
    }

    #[test]
    fn wrong_longitude_alone_is_counted() {
        let mut t = Translator::new();
        run(
            &mut t,
            0,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.0, "longitude": 10.0}}),
        );
        let tr = run(
            &mut t,
            1,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"longitude": "x"}}),
        );
        assert!(tr.observations.iter().any(|o| matches!(
            o.observation,
            Observation::Diagnostics {
                invalid_messages: 1,
                ..
            }
        )));
        assert_eq!(aircraft_telemetry(&tr).unwrap().position, None);
    }

    #[test]
    fn aircraft_claiming_to_be_its_own_gateway_stays_aircraft() {
        let mut t = Translator::new();
        run(
            &mut t,
            0,
            "sys/product/RC1/status",
            json!({"tid": "t", "bid": "b", "method": "update_topo", "data": {"sub_devices": [{"sn": "AC1", "type": 89}]}}),
        );
        run(
            &mut t,
            1,
            "thing/product/AC1/osd",
            json!({"gateway": "AC1", "data": {"latitude": 52.0, "longitude": 10.0}}),
        );
        let tr = run(
            &mut t,
            2,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.001}}),
        );
        assert!(aircraft_telemetry(&tr).is_some());
        assert_eq!(t.persisted("AC1").unwrap().0, "RC1");
    }

    #[test]
    fn update_topo_corrects_a_wrongly_classified_gateway() {
        let mut t = Translator::new();
        run(
            &mut t,
            0,
            "thing/product/AC1/osd",
            json!({"gateway": "AC1", "data": {"latitude": 52.0, "longitude": 10.0}}),
        );
        run(
            &mut t,
            1,
            "sys/product/RC1/status",
            json!({"tid": "t", "bid": "b", "method": "update_topo", "data": {"sub_devices": [{"sn": "AC1", "type": 89}]}}),
        );
        let tr = run(
            &mut t,
            2,
            "thing/product/AC1/osd",
            json!({"gateway": "RC1", "data": {"latitude": 52.001}}),
        );
        assert!(aircraft_telemetry(&tr).is_some());
        assert_eq!(t.persisted("AC1").unwrap().0, "RC1");
    }

    #[test]
    fn services_reply_is_passed_on_for_correlation() {
        let mut t = Translator::new();
        let tr = run(
            &mut t,
            0,
            "thing/product/RC1/services_reply",
            json!({"tid": "t9", "bid": "b9", "method": "live_start_push", "data": {"result": 513011}}),
        );
        assert_eq!(
            tr.command_reply,
            Some(CommandReply {
                gateway_sn: "RC1".into(),
                tid: "t9".into(),
                method: "live_start_push".into(),
                result: 513011
            })
        );
    }

    #[test]
    fn whole_fixture_translates_without_invalid_messages() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);
        let file = std::io::BufReader::new(std::fs::File::open(path).unwrap());
        let mut t = Translator::new();
        let (mut periodic, mut invalid) = (0, 0);
        for line in read_lines(file) {
            let line = line.unwrap();
            for o in t
                .handle(line.at_ms().unwrap(), &line.topic, line.payload.as_bytes())
                .observations
            {
                match o.observation {
                    Observation::AircraftTelemetry {
                        periodic: true,
                        ref aircraft_sn,
                        ref gateway_sn,
                        ..
                    } => {
                        assert_eq!(
                            (aircraft_sn.as_str(), gateway_sn.as_str()),
                            (AIRCRAFT_SN, GATEWAY_SN)
                        );
                        periodic += 1;
                    }
                    Observation::Diagnostics {
                        invalid_messages, ..
                    } => invalid += invalid_messages,
                    _ => {}
                }
            }
        }
        assert!(periodic > 80, "{periodic} periodische Proben");
        assert_eq!(invalid, 0);
    }
}

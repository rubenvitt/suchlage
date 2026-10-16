//! Synthetischer M350-Flug im DJI-Format (Doku v1.11.3). Grundlage für Replay-Tests und den
//! Simulator. Koordinaten und Seriennummern sind erfunden.

use serde_json::{Value, json};

use crate::contract::GeoPoint;
use crate::contract::examples::{AIRCRAFT_SN, GATEWAY_SN, HOME};
use crate::dji::record::RecordLine;

pub const FIXTURE_PATH: &str = "tests/fixtures/dji/synthetic-flight.jsonl";
pub const FIXTURE_START_MS: i64 = 1_792_150_000_000;

#[derive(Debug, Clone)]
pub struct Scenario {
    pub gateway_sn: String,
    pub aircraft_sn: String,
    pub home: GeoPoint,
    pub osd_interval_ms: i64,
    /// Zeitfenster ohne Fluggeräte-osd, relativ zum Start.
    pub gap: Option<(i64, i64)>,
}

impl Scenario {
    pub fn fixture() -> Self {
        Self {
            gateway_sn: GATEWAY_SN.into(),
            aircraft_sn: AIRCRAFT_SN.into(),
            home: HOME,
            osd_interval_ms: 2_000,
            gap: Some((80_000, 95_000)),
        }
    }

    pub fn live(gateway_sn: &str, aircraft_sn: &str) -> Self {
        Self {
            gateway_sn: gateway_sn.into(),
            aircraft_sn: aircraft_sn.into(),
            home: HOME,
            osd_interval_ms: 1_000,
            gap: None,
        }
    }
}

// Zeitachse des Szenarios relativ zum Start: RC-osd im festen 2-s-Takt, Topologie, Fluggerät.
const RC_OSD_INTERVAL_MS: i64 = 2_000;
const AIRCRAFT_ON_MS: i64 = 4_000;
const AIRCRAFT_FIRST_MS: i64 = 6_000;
const AIRCRAFT_LAST_MS: i64 = 190_000;
const AIRCRAFT_OFF_MS: i64 = 192_000;

/// Meter je Breitengrad, für die Umrechnung der Offsets in Koordinaten.
const METERS_PER_DEGREE: f64 = 111_320.0;

/// Ein Wegpunkt des Flugplans; zwischen zwei Wegpunkten wird linear interpoliert.
#[derive(Debug, Clone, Copy)]
struct Waypoint {
    at_ms: i64,
    north_m: f64,
    east_m: f64,
    elevation_m: f64,
    mode_code: u8,
}

const fn wp(at_ms: i64, north_m: f64, east_m: f64, elevation_m: f64, mode_code: u8) -> Waypoint {
    Waypoint {
        at_ms,
        north_m,
        east_m,
        elevation_m,
        mode_code,
    }
}

/// Bodenphase (0, 1, 2), automatischer Start (4), Suchrechteck in 60 m (3), RTH (9), Landung (10), am Boden (0).
const FLIGHT_PLAN: [Waypoint; 13] = [
    wp(6_000, 0.0, 0.0, 0.0, 0),
    wp(12_000, 0.0, 0.0, 0.0, 1),
    wp(18_000, 0.0, 0.0, 0.0, 2),
    wp(22_000, 0.0, 0.0, 0.0, 4),
    wp(30_000, 0.0, 0.0, 60.0, 3),
    wp(60_000, 300.0, 0.0, 60.0, 3),
    wp(90_000, 300.0, 250.0, 60.0, 3),
    wp(120_000, 0.0, 250.0, 60.0, 3),
    wp(150_000, 0.0, 120.0, 60.0, 9),
    wp(170_000, 0.0, 0.0, 30.0, 10),
    wp(180_000, 0.0, 0.0, 0.0, 10),
    wp(182_000, 0.0, 0.0, 0.0, 0),
    wp(190_000, 0.0, 0.0, 0.0, 0),
];

/// Position zum Zeitpunkt `t` plus Kurs zum nächsten Wegpunkt (DJI `attitude_head`, −180…180°).
fn interpolate(t: i64) -> (Waypoint, f64) {
    let i = FLIGHT_PLAN.iter().rposition(|p| p.at_ms <= t).unwrap_or(0);
    let a = FLIGHT_PLAN[i];
    let b = FLIGHT_PLAN.get(i + 1).copied().unwrap_or(a);
    let f = if b.at_ms > a.at_ms {
        (t - a.at_ms) as f64 / (b.at_ms - a.at_ms) as f64
    } else {
        0.0
    };
    let at = Waypoint {
        at_ms: t,
        north_m: a.north_m + (b.north_m - a.north_m) * f,
        east_m: a.east_m + (b.east_m - a.east_m) * f,
        elevation_m: a.elevation_m + (b.elevation_m - a.elevation_m) * f,
        mode_code: a.mode_code,
    };
    let heading = (b.east_m - a.east_m)
        .atan2(b.north_m - a.north_m)
        .to_degrees();
    (at, heading)
}

struct Builder<'a> {
    s: &'a Scenario,
    start: i64,
    out: Vec<(i64, RecordLine)>,
    seq: u64,
}

impl Builder<'_> {
    fn push(&mut self, t: i64, topic: String, data: Value, method: Option<&str>) {
        self.seq += 1;
        let mut env = json!({
            "tid": format!("sim-t-{}", self.seq),
            "bid": format!("sim-b-{}", self.seq),
            "timestamp": self.start + t,
            "gateway": self.s.gateway_sn,
            "data": data,
        });
        if let Some(m) = method {
            env["method"] = json!(m);
            env.as_object_mut().unwrap().remove("gateway"); // update_topo trägt kein gateway (Referenz Kap. 1)
        }
        let line = RecordLine::new(self.start + t, &topic, env.to_string().as_bytes());
        self.out.push((t, line));
    }

    fn topo(&mut self, t: i64, with_aircraft: bool) {
        let subs = if with_aircraft {
            json!([{ "sn": self.s.aircraft_sn, "domain": "0", "type": 89, "sub_type": 0, "index": "A", "thing_version": "1.1.2" }])
        } else {
            json!([])
        };
        let topic = format!("sys/product/{}/status", self.s.gateway_sn);
        let data = json!({ "domain": "3", "type": 119, "sub_type": 0, "thing_version": "1.1.2", "sub_devices": subs });
        self.push(t, topic, data, Some("update_topo"));
    }

    fn rc_osd(&mut self, t: i64, position: GeoPoint) {
        let topic = format!("thing/product/{}/osd", self.s.gateway_sn);
        let data = json!({
            "latitude": position.lat, "longitude": position.lon, "height": 110.2, "capacity_percent": 81,
            "wireless_link": { "sdr_quality": 4, "sdr_link_state": 1, "4g_link_state": 1, "4g_quality": 3, "4g_uav_quality": 3, "4g_gnd_quality": 3 }
        });
        self.push(t, topic, data, None);
    }

    /// Videoquellen der RC: H20N (`61-0-0`, umschaltbar) und FPV (`39-0-7`).
    fn rc_live_capacity(&mut self, t: i64) {
        let topic = format!("thing/product/{}/state", self.s.gateway_sn);
        let data = json!({
            "live_capacity": { "available_video_number": 2, "coexist_video_number_max": 1, "device_list": [{
                "sn": self.s.aircraft_sn, "available_video_number": 2, "coexist_video_number_max": 1,
                "camera_list": [
                    { "camera_index": "61-0-0", "available_video_number": 1, "coexist_video_number_max": 1,
                      "video_list": [{ "video_index": "normal-0", "video_type": "wide", "switchable_video_types": ["wide", "zoom", "thermal"] }] },
                    { "camera_index": "39-0-7", "available_video_number": 1, "coexist_video_number_max": 1,
                      "video_list": [{ "video_index": "normal-0", "video_type": "normal", "switchable_video_types": [] }] }
                ]
            }]}
        });
        self.push(t, topic, data, None);
    }

    fn aircraft_home(&mut self, t: i64) {
        let topic = format!("thing/product/{}/state", self.s.aircraft_sn);
        let home = self.s.home;
        let data = json!({
            "home_latitude": home.lat, "home_longitude": home.lon, "mode_code_reason": 0, "firmware_version": "10.01.00.09"
        });
        self.push(t, topic, data, None);
    }

    /// Fluggeräte-osd. Akku und Positionsgüte nur in jeder zweiten Nachricht (Teil-Updates);
    /// `extra_probe` ist undokumentiert und füttert die Zählung unbekannter Felder.
    fn aircraft_osd(&mut self, t: i64, n_msg: u32) {
        let (at, heading) = interpolate(t);
        let position = offset(self.s.home, at.north_m, at.east_m);
        let mode = at.mode_code;
        let mut data = json!({
            "latitude": position.lat, "longitude": position.lon, "elevation": at.elevation_m, "height": at.elevation_m + 109.4,
            "attitude_head": heading.round() as i64, "mode_code": mode,
            "horizontal_speed": if mode == 3 || mode == 9 { 10.0 } else { 0.0 },
            "vertical_speed": 0.0, "home_distance": round_to(at.north_m.hypot(at.east_m), 10.0),
            "extra_probe": n_msg,
        });
        if n_msg.is_multiple_of(2) {
            let battery_pct = 98 - t / 6_000;
            data["battery"] = json!({ "capacity_percent": battery_pct, "remain_flight_time": 1500 - t / 1000, "return_home_power": 20, "landing_power": 10,
                "batteries": [{ "index": 0, "capacity_percent": battery_pct, "voltage": 46_500, "temperature": 31.5 },
                              { "index": 1, "capacity_percent": battery_pct, "voltage": 46_480, "temperature": 31.2 }] });
            data["position_state"] =
                json!({ "is_fixed": 2, "quality": 5, "gps_number": 21, "rtk_number": 18 });
        }
        let topic = format!("thing/product/{}/osd", self.s.aircraft_sn);
        self.push(t, topic, data, None);
    }
}

/// Punkt `north_m`/`east_m` Meter neben `origin` (flache Näherung, genügt für wenige hundert Meter).
/// Auf 7 Nachkommastellen (≈ 1 cm) gerundet, damit die Fixture nicht von den letzten Bits der
/// libm abhängt und auf jeder Plattform gleich entsteht.
fn offset(origin: GeoPoint, north_m: f64, east_m: f64) -> GeoPoint {
    GeoPoint {
        lat: round_to(origin.lat + north_m / METERS_PER_DEGREE, 1e7),
        lon: round_to(
            origin.lon + east_m / (METERS_PER_DEGREE * origin.lat.to_radians().cos()),
            1e7,
        ),
    }
}

fn round_to(value: f64, scale: f64) -> f64 {
    (value * scale).round() / scale
}

pub fn generate(scenario: &Scenario, start_ms: i64) -> Vec<RecordLine> {
    let mut b = Builder {
        s: scenario,
        start: start_ms,
        out: Vec::new(),
        seq: 0,
    };

    b.topo(0, false);
    b.topo(AIRCRAFT_ON_MS, true);
    b.topo(AIRCRAFT_OFF_MS, false);

    let rc_position = offset(scenario.home, -45.0, -20.0);
    for t in (0..=AIRCRAFT_OFF_MS).step_by(RC_OSD_INTERVAL_MS as usize) {
        b.rc_osd(t, rc_position);
    }
    b.rc_live_capacity(AIRCRAFT_ON_MS);
    b.aircraft_home(AIRCRAFT_FIRST_MS);

    let mut n_msg = 0;
    let mut t = AIRCRAFT_FIRST_MS;
    while t <= AIRCRAFT_LAST_MS {
        let in_gap = scenario.gap.is_some_and(|(from, to)| t >= from && t < to);
        if !in_gap {
            b.aircraft_osd(t, n_msg);
            n_msg += 1;
        }
        t += scenario.osd_interval_ms;
    }

    // Stabil sortiert: bei gleichem Zeitpunkt bleibt die Einfügereihenfolge, Topologie zuerst.
    b.out.sort_by_key(|(t, _)| *t);
    b.out.into_iter().map(|(_, l)| l).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dji::record::read_lines;

    fn render(lines: &[RecordLine]) -> String {
        lines
            .iter()
            .map(|l| serde_json::to_string(l).unwrap() + "\n")
            .collect()
    }

    #[test]
    fn committed_fixture_matches_the_generator() {
        let fresh = render(&generate(&Scenario::fixture(), FIXTURE_START_MS));
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(FIXTURE_PATH);
        if std::env::var_os("UPDATE_FIXTURES").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &fresh).unwrap();
        }
        let committed = std::fs::read_to_string(&path)
            .expect("Fixture fehlt: UPDATE_FIXTURES=1 cargo test sim::synth");
        assert_eq!(
            committed, fresh,
            "Fixture veraltet: UPDATE_FIXTURES=1 cargo test sim::synth"
        );
    }

    #[test]
    fn scenario_covers_topology_partial_updates_and_gap() {
        let lines = generate(&Scenario::fixture(), FIXTURE_START_MS);
        let topos: Vec<_> = lines
            .iter()
            .filter(|l| l.topic.ends_with("/status"))
            .collect();
        assert_eq!(topos.len(), 3);
        assert!(topos[0].payload.contains("\"sub_devices\":[]"));
        assert!(topos[1].payload.contains(AIRCRAFT_SN));

        let osd: Vec<_> = lines
            .iter()
            .filter(|l| l.topic == format!("thing/product/{AIRCRAFT_SN}/osd"))
            .collect();
        let with_battery = osd
            .iter()
            .filter(|l| l.payload.contains("capacity_percent"))
            .count();
        assert!(
            with_battery > 0 && with_battery < osd.len(),
            "Akku nur in einem Teil der Nachrichten"
        );

        let times: Vec<i64> = osd
            .iter()
            .map(|l| l.at_ms().unwrap() - FIXTURE_START_MS)
            .collect();
        assert!(
            times.windows(2).any(|w| w[1] - w[0] >= 15_000),
            "Lücke von mindestens 15 s"
        );
        assert!(
            lines
                .windows(2)
                .all(|w| w[0].at_ms().unwrap() <= w[1].at_ms().unwrap()),
            "zeitlich sortiert"
        );
    }

    #[test]
    fn every_line_is_valid_json_in_the_m0_format() {
        let text = render(&generate(&Scenario::fixture(), FIXTURE_START_MS));
        for line in read_lines(text.as_bytes()) {
            let line = line.unwrap();
            let _: Value = serde_json::from_str(&line.payload).unwrap();
        }
    }
}

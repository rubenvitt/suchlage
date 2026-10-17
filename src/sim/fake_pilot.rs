//! Pilot-2-Attrappe für den Simulator: beantwortet Livebild-Services wie Pilot 2 und schreibt
//! Mitschnitte auf die Seriennummer der gekoppelten Fernsteuerung um.

use std::time::Duration;

use serde_json::{Value, json};

use crate::dji::record::RecordLine;
use crate::dji::topic::{self, DjiTopic};

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    StartVideo { url: String, video_id: String },
    StopVideo,
    ChangeLens { video_type: String },
    None,
}

/// Antwort (`services_reply`-Nutzlast) und was der Simulator tun soll.
pub fn handle_service(payload: &[u8]) -> Option<(Vec<u8>, Action)> {
    let env: Value = serde_json::from_slice(payload).ok()?;
    let method = env["method"].as_str()?;
    let data = &env["data"];
    let (result, action) = match method {
        // Wie Pilot 2: über MQTT nur RTMP (url_type 1), sonst „Protokoll nicht unterstützt“.
        "live_start_push" if data["url_type"] == 1 => (
            0,
            Action::StartVideo {
                url: data["url"].as_str()?.into(),
                video_id: data["video_id"].as_str()?.into(),
            },
        ),
        "live_start_push" => (513013, Action::None),
        "live_stop_push" => (0, Action::StopVideo),
        "live_lens_change" => (
            0,
            Action::ChangeLens {
                video_type: data["video_type"].as_str()?.into(),
            },
        ),
        _ => (513099, Action::None),
    };
    let reply = json!({
        "tid": env["tid"], "bid": env["bid"], "timestamp": env["timestamp"],
        "method": method, "data": { "result": result }
    });
    Some((reply.to_string().into_bytes(), action))
}

/// Der Stream, den die Attrappe gerade sendet, so wie Pilot 2 ihn in `live_status` meldet.
#[derive(Debug, Clone, PartialEq)]
pub struct LiveVideo {
    pub video_id: String,
    pub video_type: String,
}

/// `state`-Nutzlast der Fernsteuerung mit `live_status`; ohne Stream eine leere Liste.
pub fn live_status(gateway_sn: &str, live: Option<&LiveVideo>) -> Vec<u8> {
    let entries: Vec<Value> = live
        .map(|l| {
            json!({ "video_id": l.video_id, "video_type": l.video_type, "video_quality": 0, "status": 1, "error_status": 0 })
        })
        .into_iter()
        .collect();
    json!({ "gateway": gateway_sn, "data": { "live_status": entries } })
        .to_string()
        .into_bytes()
}

pub fn rewrite_gateway(line: &RecordLine, from: &str, to: &str) -> RecordLine {
    RecordLine {
        topic: line.topic.replace(from, to),
        payload: line.payload.replace(from, to),
        ..line.clone()
    }
}

/// Mitschnitt zum Abspielen: nur, was ein Gerät sendet, und auf die gekoppelte SN umgeschrieben.
pub fn replay_script(lines: Vec<RecordLine>, rc_sn: &str) -> Vec<RecordLine> {
    // Die Gateway-SN des Mitschnitts steht im ersten `sys/product/<SN>/status` (update_topo).
    let from = lines.iter().find_map(|l| match topic::parse(&l.topic) {
        DjiTopic::Status { gateway } => Some(gateway.to_string()),
        _ => None,
    });
    lines
        .into_iter()
        .filter(|l| {
            matches!(
                topic::parse(&l.topic),
                DjiTopic::Osd { .. }
                    | DjiTopic::State { .. }
                    | DjiTopic::Status { .. }
                    | DjiTopic::Events { .. }
                    | DjiTopic::Requests { .. }
            )
        })
        .map(|l| match &from {
            Some(from) => rewrite_gateway(&l, from, rc_sn),
            None => l,
        })
        .collect()
}

/// Was der Simulator mit einer Antwort von `/pilot/api/connect` tut. `None`: Server nicht erreichbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectOutcome {
    Ready,
    Retry,
    GiveUp,
}

pub fn classify_connect(status: Option<u16>) -> ConnectOutcome {
    match status {
        Some(s) if (200..300).contains(&s) => ConnectOutcome::Ready,
        // Server startet noch oder die Broker-Sitzung steht noch nicht.
        None | Some(502..=504) => ConnectOutcome::Retry,
        _ => ConnectOutcome::GiveUp,
    }
}

/// 0,5 s, 1 s, 2 s, 4 s, dann alle 5 s – zusammen gut 60 s.
pub fn retry_delays() -> Vec<Duration> {
    let mut d: Vec<u64> = vec![500, 1_000, 2_000, 4_000];
    while d.iter().sum::<u64>() < 60_000 {
        d.push(5_000);
    }
    d.into_iter().map(Duration::from_millis).collect()
}

/// Host und Port aus der Broker-Adresse für Pilot 2 (`tcp://host:port`).
pub fn broker_address(mqtt_host: &str) -> Result<(String, u16), String> {
    let invalid = || format!("Broker-Adresse {mqtt_host:?} ist nicht host:port");
    let (host, port) = mqtt_host
        .trim_start_matches("tcp://")
        .rsplit_once(':')
        .ok_or_else(invalid)?;
    if host.is_empty() {
        return Err(invalid());
    }
    Ok((host.to_string(), port.parse().map_err(|_| invalid())?))
}

#[derive(Debug, Clone, PartialEq)]
pub struct Args {
    pub server: String,
    pub code: String,
    pub rc_sn: String,
    pub aircraft_sn: String,
    pub replay: Option<String>,
    pub speed: f64,
    pub repeat: bool,
    pub video: bool,
}

const USAGE: &str = "Aufruf: suchlage-sim --server <url> --code <kopplungscode> [--rc-sn SN] [--aircraft-sn SN] [--replay datei.jsonl] [--speed 1] [--loop] [--no-video]";

pub fn parse_args(mut it: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut a = Args {
        server: String::new(),
        code: String::new(),
        rc_sn: "SIMRC0001".into(),
        aircraft_sn: "SIMAC0001".into(),
        replay: None,
        speed: 1.0,
        repeat: false,
        video: true,
    };
    while let Some(flag) = it.next() {
        let mut value = || {
            it.next()
                .ok_or_else(|| format!("{flag} braucht einen Wert"))
        };
        match flag.as_str() {
            "--server" => a.server = value()?,
            "--code" => a.code = value()?,
            "--rc-sn" => a.rc_sn = serial_number(&flag, value()?)?,
            "--aircraft-sn" => a.aircraft_sn = serial_number(&flag, value()?)?,
            "--replay" => a.replay = Some(value()?),
            "--speed" => a.speed = speed(&value()?)?,
            "--loop" => a.repeat = true,
            "--no-video" => a.video = false,
            other => return Err(format!("unbekannte Option {other}\n{USAGE}")),
        }
    }
    if a.server.is_empty() || a.code.is_empty() {
        return Err(USAGE.into());
    }
    Ok(a)
}

/// Die SN landet in Topics; sie muss so aussehen, wie der Server sie annimmt.
fn serial_number(flag: &str, sn: String) -> Result<String, String> {
    if topic::is_valid_sn(&sn) {
        Ok(sn)
    } else {
        Err(format!("{flag}: 1 bis 64 Buchstaben oder Ziffern erwartet"))
    }
}

fn speed(value: &str) -> Result<f64, String> {
    match value.parse::<f64>() {
        Ok(s) if s.is_finite() && s > 0.0 => Ok(s),
        _ => Err("--speed: Zahl größer als 0 erwartet".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(method: &str, data: Value) -> Vec<u8> {
        json!({"tid": "t1", "bid": "b1", "timestamp": 1, "method": method, "data": data})
            .to_string()
            .into_bytes()
    }

    fn args(list: &[&str]) -> Result<Args, String> {
        parse_args(list.iter().map(|s| s.to_string()))
    }

    #[test]
    fn start_push_is_acknowledged_and_starts_video() {
        let (reply, action) = handle_service(&call(
            "live_start_push",
            json!({"url_type": 1, "url": "rtmp://h/live/RC1?user=rc-RC1&pass=k", "video_id": "AC1/61-0-0/normal-0", "video_quality": 0}),
        ))
        .unwrap();
        let v: Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(
            (
                v["tid"].as_str(),
                v["bid"].as_str(),
                v["method"].as_str(),
                v["data"]["result"].as_i64()
            ),
            (Some("t1"), Some("b1"), Some("live_start_push"), Some(0))
        );
        assert_eq!(
            action,
            Action::StartVideo {
                url: "rtmp://h/live/RC1?user=rc-RC1&pass=k".into(),
                video_id: "AC1/61-0-0/normal-0".into()
            }
        );
    }

    #[test]
    fn non_rtmp_start_is_refused_like_pilot_would() {
        let (reply, action) = handle_service(&call(
            "live_start_push",
            json!({"url_type": 0, "url": "x", "video_id": "v"}),
        ))
        .unwrap();
        let v: Value = serde_json::from_slice(&reply).unwrap();
        assert_eq!(v["data"]["result"], 513013);
        assert_eq!(action, Action::None);
    }

    #[test]
    fn stop_and_lens_change_are_acknowledged() {
        assert_eq!(
            handle_service(&call("live_stop_push", json!({"video_id": "v"})))
                .unwrap()
                .1,
            Action::StopVideo
        );
        assert_eq!(
            handle_service(&call(
                "live_lens_change",
                json!({"video_id": "v", "video_type": "thermal"})
            ))
            .unwrap()
            .1,
            Action::ChangeLens {
                video_type: "thermal".into()
            }
        );
        assert!(handle_service(b"kaputt").is_none());
    }

    #[test]
    fn unknown_service_is_answered_with_an_error_code() {
        let (reply, action) = handle_service(&call("flighttask_prepare", json!({}))).unwrap();
        let v: Value = serde_json::from_slice(&reply).unwrap();
        assert_ne!(v["data"]["result"], 0, "nur bekannte Services gelingen");
        assert_eq!(action, Action::None);
    }

    #[test]
    fn live_status_reports_the_running_stream_or_none() {
        let live = LiveVideo {
            video_id: "AC1/61-0-0/normal-0".into(),
            video_type: "thermal".into(),
        };
        let v: Value = serde_json::from_slice(&live_status("RC1", Some(&live))).unwrap();
        assert_eq!(v["gateway"], "RC1");
        let entry = &v["data"]["live_status"][0];
        assert_eq!(
            (
                entry["video_id"].as_str(),
                entry["video_type"].as_str(),
                entry["status"].as_i64(),
                entry["error_status"].as_i64()
            ),
            (
                Some("AC1/61-0-0/normal-0"),
                Some("thermal"),
                Some(1),
                Some(0)
            )
        );
        let v: Value = serde_json::from_slice(&live_status("RC1", None)).unwrap();
        assert_eq!(v["data"]["live_status"], json!([]));
    }

    #[test]
    fn replay_is_rewritten_to_the_paired_serial_number() {
        let line = RecordLine::new(
            1,
            "thing/product/RCSIM0001/osd",
            br#"{"gateway":"RCSIM0001","data":{}}"#,
        );
        let out = rewrite_gateway(&line, "RCSIM0001", "RCREAL01");
        assert_eq!(out.topic, "thing/product/RCREAL01/osd");
        assert!(out.payload.contains("\"gateway\":\"RCREAL01\""));
    }

    #[test]
    fn replay_script_keeps_only_device_messages_of_the_paired_rc() {
        let recorded = vec![
            RecordLine::new(
                1,
                "sys/product/RCOLD01/status",
                br#"{"method":"update_topo"}"#,
            ),
            RecordLine::new(2, "thing/product/RCOLD01/osd", br#"{"gateway":"RCOLD01"}"#),
            RecordLine::new(3, "thing/product/AC1/osd", br#"{"gateway":"RCOLD01"}"#),
            RecordLine::new(4, "thing/product/RCOLD01/services", b"{}"),
            RecordLine::new(5, "thing/product/RCOLD01/services_reply", b"{}"),
            RecordLine::new(6, "sys/product/RCOLD01/status_reply", b"{}"),
            RecordLine::new(7, "thing/product/RCOLD01/events", b"{}"),
        ];
        let script = replay_script(recorded, "RCNEW01");
        let topics: Vec<&str> = script.iter().map(|l| l.topic.as_str()).collect();
        assert_eq!(
            topics,
            [
                "sys/product/RCNEW01/status",
                "thing/product/RCNEW01/osd",
                "thing/product/AC1/osd",
                "thing/product/RCNEW01/events"
            ],
            "Server-Richtung und Antworten aus dem Mitschnitt spielt die Attrappe nicht ab"
        );
        assert!(script[2].payload.contains("RCNEW01"));
    }

    #[test]
    fn replay_without_status_line_keeps_its_serial_numbers() {
        let recorded = vec![RecordLine::new(1, "thing/product/RC9/osd", b"{}")];
        assert_eq!(
            replay_script(recorded, "RCNEW01")[0].topic,
            "thing/product/RC9/osd"
        );
    }

    #[test]
    fn connect_retries_only_while_the_server_is_not_ready() {
        assert_eq!(classify_connect(None), ConnectOutcome::Retry);
        assert_eq!(classify_connect(Some(503)), ConnectOutcome::Retry);
        assert_eq!(classify_connect(Some(200)), ConnectOutcome::Ready);
        assert_eq!(
            classify_connect(Some(401)),
            ConnectOutcome::GiveUp,
            "falscher Code wird nicht besser"
        );
        assert_eq!(classify_connect(Some(409)), ConnectOutcome::GiveUp);
        let total: u128 = retry_delays().iter().map(|d| d.as_millis()).sum();
        assert!((60_000..70_000).contains(&total), "{total}");
    }

    #[test]
    fn broker_address_needs_host_and_port() {
        assert_eq!(
            broker_address("tcp://127.0.0.1:1883"),
            Ok(("127.0.0.1".into(), 1883))
        );
        assert_eq!(
            broker_address("mqtt.vpn:1884"),
            Ok(("mqtt.vpn".into(), 1884))
        );
        assert!(broker_address("tcp://127.0.0.1").is_err());
        assert!(broker_address("tcp://:1883").is_err());
        assert!(broker_address("tcp://h:port").is_err());
    }

    #[test]
    fn args_need_server_and_code() {
        let a = args(&[
            "--server",
            "http://127.0.0.1:8080",
            "--code",
            "abc",
            "--speed",
            "4",
            "--loop",
        ])
        .unwrap();
        assert_eq!(
            (a.speed, a.repeat, a.video, a.rc_sn.as_str()),
            (4.0, true, true, "SIMRC0001")
        );
        assert!(args(&["--server", "x"]).unwrap_err().contains("--code"));
        assert!(
            args(&["--server", "x", "--code"])
                .unwrap_err()
                .contains("--code")
        );
        assert!(
            args(&["--server", "x", "--code", "c", "--fly"])
                .unwrap_err()
                .contains("--fly")
        );
    }

    #[test]
    fn args_refuse_values_that_would_break_the_run() {
        let base = ["--server", "x", "--code", "c"];
        for speed in ["0", "-1", "NaN", "inf", "schnell"] {
            let err = args(&[&base[..], &["--speed", speed]].concat()).unwrap_err();
            assert!(err.contains("--speed"), "{speed}: {err}");
        }
        for (flag, sn) in [
            ("--rc-sn", "RC/1"),
            ("--aircraft-sn", "AC#1"),
            ("--rc-sn", ""),
        ] {
            let err = args(&[&base[..], &[flag, sn]].concat()).unwrap_err();
            assert!(err.contains(flag), "{flag} {sn:?}: {err}");
        }
        let a = args(
            &[
                &base[..],
                &["--replay", "f.jsonl", "--no-video", "--speed", "0.5"],
            ]
            .concat(),
        )
        .unwrap();
        assert_eq!(
            (a.replay.as_deref(), a.video, a.speed),
            (Some("f.jsonl"), false, 0.5)
        );
    }
}

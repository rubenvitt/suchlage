//! Kanonische Beispielwerte. Quelle für `fixtures.generated.ts` (Frontend-Tests, MSW) und für
//! Rust-Tests. Seriennummern und Koordinaten sind erfunden.

use serde::Serialize;
use serde::de::DeserializeOwned;

use super::*;

pub const T0: i64 = 1_792_150_000_000;
pub const GATEWAY_SN: &str = "RCSIM0001";
pub const AIRCRAFT_SN: &str = "ACSIM0001";
pub const HOME: GeoPoint = GeoPoint {
    lat: 52.430_00,
    lon: 10.010_00,
};

pub fn thresholds() -> FreshnessThresholds {
    FreshnessThresholds {
        delayed_after_ms: FRESHNESS_DELAYED_MS,
        lost_after_ms: FRESHNESS_LOST_MS,
    }
}

pub fn system_ok() -> SystemStatus {
    SystemStatus {
        broker: BrokerStatus::Connected,
        unknown_fields: vec![UnknownFieldCount {
            key: "aircraft.wind_speed_extra".into(),
            count: 3,
        }],
        invalid_messages: 0,
        raw_recording: Some("20261016T120000Z.jsonl".into()),
    }
}

pub fn video_sources() -> Vec<VideoSource> {
    vec![
        VideoSource {
            video_id: format!("{AIRCRAFT_SN}/61-0-0/normal-0"),
            camera_index: "61-0-0".into(),
            camera_label: "H20N".into(),
            lens: Lens::Wide,
            switchable_lenses: vec![Lens::Wide, Lens::Zoom, Lens::Thermal],
        },
        VideoSource {
            video_id: format!("{AIRCRAFT_SN}/39-0-7/normal-0"),
            camera_index: "39-0-7".into(),
            camera_label: "FPV".into(),
            lens: Lens::Normal,
            switchable_lenses: vec![],
        },
    ]
}

pub fn gateway(aircraft: bool) -> GatewayState {
    GatewayState {
        sn: GATEWAY_SN.into(),
        label: Some("RC Plus 1".into()),
        last_seen_ms: T0,
        aircraft_sn: aircraft.then(|| AIRCRAFT_SN.into()),
        position: Some(GeoPoint {
            lat: HOME.lat - 0.000_4,
            lon: HOME.lon - 0.000_3,
        }),
        battery_percent: Some(81),
        link: LinkQuality {
            sdr_quality: Some(4),
            lte_quality: Some(3),
        },
        video_sources: if aircraft { video_sources() } else { vec![] },
    }
}

pub fn telemetry_flying() -> Telemetry {
    Telemetry {
        position: Some(GeoPoint {
            lat: 52.432_10,
            lon: 10.013_40,
        }),
        height_ellipsoid_m: Some(171.4),
        elevation_takeoff_m: Some(62.0),
        home_distance_m: Some(412.0),
        horizontal_speed_mps: Some(8.4),
        vertical_speed_mps: Some(0.0),
        heading_deg: Some(274.0),
        battery: Some(Battery {
            percent: Some(63),
            remaining_flight_s: Some(1100),
            return_home_percent: Some(22),
        }),
        gnss: Some(Gnss {
            fix: Some(RtkFix::Fixed),
            quality: Some(5),
            gps_satellites: Some(21),
            rtk_satellites: Some(18),
        }),
        mode: Some(FlightMode {
            code: 3,
            label: "manueller Flug".into(),
        }),
        home: Some(HOME),
    }
}

pub fn track_segments() -> Vec<Vec<TrackPoint>> {
    let p = |dt: i64, lat: f64, lon: f64, e: f64| TrackPoint {
        t_ms: T0 - 600_000 + dt,
        lat,
        lon,
        elevation_m: Some(e),
    };
    vec![
        vec![
            p(0, 52.430_00, 10.010_00, 2.0),
            p(2_000, 52.430_20, 10.010_30, 15.0),
            p(4_000, 52.430_60, 10.010_90, 40.0),
        ],
        vec![
            p(30_000, 52.431_50, 10.012_00, 61.0),
            p(32_000, 52.432_10, 10.013_40, 62.0),
        ],
    ]
}

pub fn flight_running() -> FlightSummary {
    FlightSummary {
        id: 7,
        aircraft_sn: AIRCRAFT_SN.into(),
        gateway_sn: GATEWAY_SN.into(),
        started_at_ms: T0 - 600_000,
        ended_at_ms: None,
        end_reason: None,
        max_elevation_m: Some(62.0),
        distance_m: 431.0,
    }
}

pub fn flight_landed(id: i64, started_at_ms: i64) -> FlightSummary {
    FlightSummary {
        id,
        aircraft_sn: AIRCRAFT_SN.into(),
        gateway_sn: GATEWAY_SN.into(),
        started_at_ms,
        ended_at_ms: Some(started_at_ms + 1_080_000),
        end_reason: Some(FlightEndReason::Landed),
        max_elevation_m: Some(85.0),
        distance_m: 2_310.0,
    }
}

pub fn aircraft_flying() -> AircraftState {
    AircraftState {
        sn: AIRCRAFT_SN.into(),
        gateway_sn: GATEWAY_SN.into(),
        last_seen_ms: T0,
        phase: FlightPhase::Airborne,
        flight_id: Some(7),
        telemetry: telemetry_flying(),
    }
}

pub fn stream_live() -> StreamState {
    StreamState {
        gateway_sn: GATEWAY_SN.into(),
        phase: StreamPhase::Live,
        video_id: Some(format!("{AIRCRAFT_SN}/61-0-0/normal-0")),
        lens: Some(Lens::Wide),
        error: None,
        updated_at_ms: T0 - 590_000,
    }
}

pub fn stream_failed() -> StreamState {
    StreamState {
        phase: StreamPhase::Failed,
        error: Some("Pilot 2 lehnt ab: Livestream ist in Pilot 2 nicht aktiviert (513011).".into()),
        ..stream_live()
    }
}

pub fn snapshot_empty() -> SituationSnapshot {
    SituationSnapshot {
        server_time_ms: T0,
        freshness: thresholds(),
        gateways: vec![],
        aircraft: vec![],
        active_flights: vec![],
        streams: vec![],
        system: SystemStatus {
            broker: BrokerStatus::Connected,
            unknown_fields: vec![],
            invalid_messages: 0,
            raw_recording: None,
        },
    }
}

pub fn snapshot_gateway_only() -> SituationSnapshot {
    SituationSnapshot {
        gateways: vec![gateway(false)],
        ..snapshot_empty()
    }
}

pub fn snapshot_flying() -> SituationSnapshot {
    SituationSnapshot {
        server_time_ms: T0 + 1_000,
        freshness: thresholds(),
        gateways: vec![gateway(true)],
        aircraft: vec![aircraft_flying()],
        active_flights: vec![ActiveFlight {
            flight: flight_running(),
            segments: track_segments(),
        }],
        streams: vec![stream_live()],
        system: system_ok(),
    }
}

/// Eine Fläche aus dem Browser und ein Punkt aus Pilot 2 (mit Höhe, wie Pilot 2 sie schickt).
pub fn map_elements() -> Vec<MapElement> {
    let corner = |lat: f64, lon: f64| MapPoint {
        lat,
        lon,
        alt_m: None,
    };
    vec![
        MapElement {
            id: "3f0c1a52-6d1e-4c39-9a51-0b7e2f4d8c11".into(),
            name: "Suchgebiet Nord".into(),
            kind: MapElementKind::Polygon,
            points: vec![
                corner(52.433_0, 10.008_0),
                corner(52.433_0, 10.016_0),
                corner(52.430_5, 10.016_0),
                corner(52.430_5, 10.008_0),
            ],
            color: "#FFBB00".into(),
            clamp_to_ground: true,
            author: "Gruppenführer I&K".into(),
            origin: MapElementOrigin::Web,
            created_at_ms: T0 - 300_000,
            updated_at_ms: T0 - 300_000,
            mission_id: None,
        },
        MapElement {
            id: "9b2e4f60-1c3d-4e5f-8a7b-6c5d4e3f2a10".into(),
            name: "Pilot 1".into(),
            kind: MapElementKind::Point,
            points: vec![MapPoint {
                lat: 52.431_8,
                lon: 10.012_4,
                alt_m: Some(84.2),
            }],
            color: "#E23C39".into(),
            clamp_to_ground: false,
            author: "RC Plus 1".into(),
            origin: MapElementOrigin::Pilot,
            created_at_ms: T0 - 60_000,
            updated_at_ms: T0 - 60_000,
            mission_id: None,
        },
    ]
}

pub fn device_log() -> Vec<DeviceLogEvent> {
    let entry = |seq: u64, source, level, method: Option<&str>, summary: &str| DeviceLogEntry {
        seq,
        at_ms: T0 + seq as i64 * 1_000,
        source,
        level,
        device_sn: Some(GATEWAY_SN.into()),
        topic: match source {
            DeviceLogSource::Device => Some(format!("thing/product/{GATEWAY_SN}/services_reply")),
            DeviceLogSource::Server => Some(format!("thing/product/{GATEWAY_SN}/services")),
            _ => None,
        },
        method: method.map(Into::into),
        summary: summary.into(),
        payload: method.map(|m| format!(r#"{{"method":"{m}","data":{{"result":0}}}}"#)),
    };
    vec![
        DeviceLogEvent::History {
            entries: vec![
                entry(
                    1,
                    DeviceLogSource::Pilot,
                    DeviceLogLevel::Warn,
                    None,
                    "Verbindung zu Suchlage: getrennt",
                ),
                entry(
                    2,
                    DeviceLogSource::Server,
                    DeviceLogLevel::Info,
                    Some("live_start_push"),
                    "Befehl live_start_push",
                ),
            ],
        },
        DeviceLogEvent::Entry {
            entry: entry(
                3,
                DeviceLogSource::System,
                DeviceLogLevel::Error,
                Some("live_start_push"),
                "live_start_push: keine Antwort in 10 s",
            ),
        },
        DeviceLogEvent::Missed { count: 4 },
    ]
}

pub fn live_events() -> Vec<LiveEvent> {
    vec![
        LiveEvent::Snapshot {
            snapshot: snapshot_flying(),
        },
        LiveEvent::Gateway {
            gateway: gateway(true),
        },
        LiveEvent::Aircraft {
            aircraft: aircraft_flying(),
        },
        LiveEvent::TrackPoint {
            flight_id: 7,
            segment: 1,
            point: TrackPoint {
                t_ms: T0 + 2_000,
                lat: 52.432_30,
                lon: 10.013_10,
                elevation_m: Some(62.0),
            },
        },
        LiveEvent::FlightStarted {
            flight: flight_running(),
        },
        LiveEvent::FlightEnded {
            flight: flight_landed(7, T0 - 600_000),
        },
        LiveEvent::Stream {
            stream: stream_failed(),
        },
        LiveEvent::System {
            system: system_ok(),
        },
        LiveEvent::MapElements {
            elements: map_elements(),
        },
        LiveEvent::MapElement {
            element: map_elements()[1].clone(),
        },
        LiveEvent::MapElementDeleted {
            id: map_elements()[0].id.clone(),
        },
        LiveEvent::PhotoShot {
            shot: photo_shots()[0].clone(),
        },
        LiveEvent::MissionsChanged,
    ]
}

pub fn flight_list() -> Vec<FlightSummary> {
    vec![
        flight_running(),
        flight_landed(6, T0 - 7_200_000),
        flight_landed(5, T0 - 86_400_000),
    ]
}

pub fn flight_detail() -> FlightDetail {
    FlightDetail {
        flight: flight_landed(6, T0 - 7_200_000),
        segments: track_segments(),
        video: Some(FlightVideo {
            play_url: "/api/flights/6/video".into(),
            download_url: "/api/flights/6/video?download=1".into(),
        }),
    }
}

/// Zwei Auslösungen im Flug 6: eine mit drei Linsen und Position, eine ohne Position, deren
/// Vorschaubild noch entsteht.
pub fn photo_shots() -> Vec<PhotoShot> {
    let photo = |id: i64, name: &str, lens: PhotoLens, thumb: bool| Photo {
        id,
        name: name.into(),
        lens,
        thumb_url: thumb.then(|| format!("/api/photos/{id}/thumb?v=1792248620000")),
        original_url: format!("/api/photos/{id}/original?v=1792248620000"),
    };
    vec![
        PhotoShot {
            id: format!("{GATEWAY_SN}/DJI_20261017143012_0001"),
            flight_id: Some(6),
            gateway_sn: GATEWAY_SN.into(),
            aircraft_sn: Some(AIRCRAFT_SN.into()),
            taken_at_ms: T0 - 7_100_000,
            position: Some(GeoPoint {
                lat: 52.431_9,
                lon: 10.013_0,
            }),
            absolute_altitude_m: Some(142.3),
            relative_altitude_m: Some(61.5),
            gimbal_yaw_deg: Some(87.4),
            mission_id: None,
            photos: vec![
                photo(
                    11,
                    "DJI_20261017143012_0001_T.JPG",
                    PhotoLens::Thermal,
                    true,
                ),
                photo(12, "DJI_20261017143012_0001_W.JPG", PhotoLens::Wide, true),
                photo(13, "DJI_20261017143012_0001_Z.JPG", PhotoLens::Zoom, true),
            ],
        },
        PhotoShot {
            id: format!("{GATEWAY_SN}/DJI_20261017143140_0002"),
            flight_id: Some(6),
            gateway_sn: GATEWAY_SN.into(),
            aircraft_sn: Some(AIRCRAFT_SN.into()),
            taken_at_ms: T0 - 7_010_000,
            position: None,
            absolute_altitude_m: None,
            relative_altitude_m: None,
            gimbal_yaw_deg: None,
            mission_id: None,
            photos: vec![photo(
                14,
                "DJI_20261017143140_0002_T.JPG",
                PhotoLens::Thermal,
                false,
            )],
        },
    ]
}

pub fn settings_view() -> SettingsView {
    SettingsView {
        oidc: Some(OidcSettingsView {
            issuer: "https://id.example.org".into(),
            client_id: "suchlage".into(),
            has_client_secret: true,
            redirect_url: "https://suchlage.example.org/api/auth/oidc/callback".into(),
        }),
        dji: Some(DjiSettingsView {
            app_id: "123456".into(),
            has_app_key: true,
            has_license: true,
            workspace_id: "00000000-0000-4000-8000-000000000001".into(),
        }),
        broker: Some(BrokerSettingsView {
            host: "mqtt".into(),
            port: 1883,
            admin_username: "suchlage-admin".into(),
            has_admin_password: true,
            rc_url: "tcp://100.64.0.10:1883".into(),
        }),
        media: Some(MediaSettings {
            webrtc_url: "http://media:8889".into(),
            hls_url: "http://media:8888".into(),
            playback_url: "http://media:9996".into(),
            rtmp_url: "rtmp://100.64.0.10:1935".into(),
        }),
        photo_storage: Some(PhotoStorageSettingsView {
            s3_url: "http://fotos:3900".into(),
            rc_url: "http://100.64.0.10:3900".into(),
            admin_url: "http://fotos:3903".into(),
            bucket: "suchlage-fotos".into(),
            region: "garage".into(),
            access_key_id: "GK31c2f218a2e44f485b94239e".into(),
            has_secret_access_key: true,
            has_admin_token: true,
        }),
        general: GeneralSettings {
            auto_live_on_takeoff: true,
            record_raw_mqtt: true,
        },
    }
}

pub fn settings_empty() -> SettingsView {
    SettingsView {
        oidc: None,
        dji: None,
        broker: None,
        media: None,
        photo_storage: None,
        general: GeneralSettings {
            auto_live_on_takeoff: true,
            record_raw_mqtt: true,
        },
    }
}

pub fn remote_controllers() -> Vec<RemoteControllerView> {
    vec![
        RemoteControllerView {
            id: 1,
            label: "RC Plus 1".into(),
            sn: Some(GATEWAY_SN.into()),
            paired_at_ms: Some(T0 - 86_400_000),
        },
        RemoteControllerView {
            id: 2,
            label: "RC Plus Reserve".into(),
            sn: None,
            paired_at_ms: None,
        },
    ]
}

pub fn kiosk_tokens() -> Vec<KioskTokenView> {
    vec![KioskTokenView {
        id: 1,
        label: "FüKw Großbild".into(),
        created_at_ms: T0 - 3_600_000,
    }]
}

pub fn auth_options(oidc: bool, dev_login: bool, setup_required: bool) -> AuthOptions {
    AuthOptions {
        oidc,
        dev_login,
        setup_required,
        redirect_url: "https://suchlage.example.org/api/auth/oidc/callback".into(),
    }
}

pub fn health() -> Health {
    Health {
        status: "ok".into(),
        version: "0.2.3".into(),
        commit: Some("a5a73ed".into()),
    }
}

pub struct Fixture {
    pub name: &'static str,
    pub ts_type: &'static str,
    pub json: serde_json::Value,
}

fn fx<T: Serialize>(name: &'static str, ts_type: &'static str, value: &T) -> Fixture {
    Fixture {
        name,
        ts_type,
        json: serde_json::to_value(value).unwrap(),
    }
}

pub fn all() -> Vec<Fixture> {
    vec![
        fx("snapshotEmpty", "SituationSnapshot", &snapshot_empty()),
        fx(
            "snapshotGatewayOnly",
            "SituationSnapshot",
            &snapshot_gateway_only(),
        ),
        fx("snapshotFlying", "SituationSnapshot", &snapshot_flying()),
        fx("liveEvents", "LiveEvent[]", &live_events()),
        fx("deviceLog", "DeviceLogEvent[]", &device_log()),
        fx("flightList", "FlightSummary[]", &flight_list()),
        fx("flightDetail", "FlightDetail", &flight_detail()),
        fx("settingsView", "SettingsView", &settings_view()),
        fx("settingsEmpty", "SettingsView", &settings_empty()),
        fx(
            "remoteControllers",
            "RemoteControllerView[]",
            &remote_controllers(),
        ),
        fx("kioskTokens", "KioskTokenView[]", &kiosk_tokens()),
        fx("mapElements", "MapElement[]", &map_elements()),
        fx("photoShots", "PhotoShot[]", &photo_shots()),
        fx(
            "meAdmin",
            "Me",
            &Me {
                display_name: "Erika Mustermann".into(),
                role: Role::Admin,
            },
        ),
        fx(
            "meOperator",
            "Me",
            &Me {
                display_name: "Gruppenführer I&K".into(),
                role: Role::Operator,
            },
        ),
        fx(
            "meKiosk",
            "Me",
            &Me {
                display_name: "FüKw Großbild".into(),
                role: Role::Kiosk,
            },
        ),
        fx(
            "authOptionsDev",
            "AuthOptions",
            &auth_options(false, true, false),
        ),
        fx(
            "authOptionsSetup",
            "AuthOptions",
            &auth_options(false, false, true),
        ),
        fx(
            "authOptionsOidc",
            "AuthOptions",
            &auth_options(true, false, false),
        ),
        fx("health", "Health", &health()),
    ]
}

fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
    let json = serde_json::to_string(&value).unwrap();
    assert_eq!(serde_json::from_str::<T>(&json).unwrap(), value, "{json}");
}

pub fn assert_roundtrips() {
    roundtrip(snapshot_flying());
    roundtrip(live_events());
    roundtrip(device_log());
    roundtrip(flight_detail());
    roundtrip(settings_view());
    roundtrip(remote_controllers());
    roundtrip(map_elements());
    roundtrip(photo_shots());
    roundtrip(health());
}

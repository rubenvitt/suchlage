//! Reine Regeln der Fotos: Linse, Auslösung, Position, Aufnahmezeit, Ablageort. Ohne I/O, damit
//! sie ohne Speicher und Pilot 2 testbar sind.

use std::collections::BTreeMap;

use chrono::{Datelike, NaiveDate, NaiveDateTime, Weekday};

use crate::contract::{GeoPoint, Photo, PhotoLens, PhotoShot};
use crate::store::photos::PhotoRow;

/// Dateiname ohne Endung, z. B. `DJI_20261017143012_0001_T`.
fn stem(name: &str) -> &str {
    let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
    base.rsplit_once('.').map_or(base, |(stem, _)| stem)
}

/// Linse aus der Endung des DJI-Dateinamens.
pub fn lens_of(name: &str) -> PhotoLens {
    match stem(name).rsplit_once('_').map(|(_, suffix)| suffix) {
        Some("T") => PhotoLens::Thermal,
        Some("W") => PhotoLens::Wide,
        Some("Z") => PhotoLens::Zoom,
        Some("V") => PhotoLens::Visible,
        _ => PhotoLens::Unknown,
    }
}

/// Fernsteuerung und Dateiname ohne Linsen-Endung: Alle Linsen einer Auslösung tragen denselben
/// Namen bis auf die Endung (`DJI_<Zeit>_<Nummer>_<Linse>`). Ohne erkannte Linse ist jede Datei
/// eine eigene Auslösung.
pub fn shot_key(gateway_sn: &str, name: &str) -> String {
    let stem = stem(name);
    let base = match lens_of(name) {
        PhotoLens::Unknown => stem,
        _ => stem.rsplit_once('_').map_or(stem, |(base, _)| base),
    };
    format!("{gateway_sn}/{base}")
}

/// Gültige Position oder `None`: Pilot 2 schickt ohne GNSS 0/0.
pub fn position(lat: Option<f64>, lon: Option<f64>) -> Option<GeoPoint> {
    let (lat, lon) = (lat?, lon?);
    let valid = lat.is_finite()
        && lon.is_finite()
        && (-90.0..=90.0).contains(&lat)
        && (-180.0..=180.0).contains(&lon)
        && !(lat == 0.0 && lon == 0.0);
    valid.then_some(GeoPoint { lat, lon })
}

/// Letzter Sonntag eines Monats um 01:00 UTC: Beginn und Ende der Sommerzeit in der EU.
fn last_sunday_1utc(year: i32, month: u32) -> Option<i64> {
    let last = NaiveDate::from_ymd_opt(year, month + 1, 1)?.pred_opt()?;
    let back = last.weekday().num_days_from_sunday();
    let sunday = last - chrono::Duration::days(i64::from(back));
    debug_assert_eq!(sunday.weekday(), Weekday::Sun);
    Some(sunday.and_hms_opt(1, 0, 0)?.and_utc().timestamp_millis())
}

/// Ortszeit Europe/Berlin (MEZ/MESZ) als UTC-Millisekunden. In der doppelten Stunde im Oktober
/// gilt die Sommerzeit, in der übersprungenen im März die Winterzeit.
fn berlin_to_utc_ms(local: NaiveDateTime) -> Option<i64> {
    let year = local.year();
    let (start, end) = (last_sunday_1utc(year, 3)?, last_sunday_1utc(year, 10)?);
    let as_utc = local.and_utc().timestamp_millis();
    let summer = as_utc - 2 * 3_600_000;
    if (start..end).contains(&summer) {
        Some(summer)
    } else {
        Some(as_utc - 3_600_000)
    }
}

/// Aufnahmezeit aus `metadata.created_time`. Das Format ist nicht dokumentiert: RFC 3339 gilt genau,
/// eine Zeit ohne Offset als Ortszeit Europe/Berlin, eine reine Zahl als Unix-Zeit (s oder ms).
pub fn taken_at_ms(created_time: &str) -> Option<i64> {
    let v = created_time.trim();
    if v.is_empty() {
        return None;
    }
    if let Ok(n) = v.parse::<i64>() {
        return Some(if n > 100_000_000_000 { n } else { n * 1000 });
    }
    if let Ok(t) = chrono::DateTime::parse_from_rfc3339(v) {
        return Some(t.timestamp_millis());
    }
    for format in [
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y:%m:%d %H:%M:%S",
    ] {
        if let Ok(local) = NaiveDateTime::parse_from_str(v, format) {
            return berlin_to_utc_ms(local);
        }
    }
    None
}

/// Ablageort einer Fernsteuerung im Bucket (`object_key_prefix` für Pilot 2).
pub fn object_key_prefix(rc_sn: &str) -> String {
    format!("fotos/{rc_sn}")
}

/// Liegt der Schlüssel unter dem Ablageort dieser Fernsteuerung, ohne Ausbruch über `..`?
pub fn object_key_allowed(object_key: &str, rc_sn: &str) -> bool {
    let prefix = format!("{}/", object_key_prefix(rc_sn));
    object_key.len() <= 1024
        && object_key
            .strip_prefix(&prefix)
            .is_some_and(|rest| !rest.is_empty())
        && !object_key
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
}

/// Name für die Anzeige: der Name aus Pilot 2, ohne ihn das Ende des Objektschlüssels.
pub fn display_name(name: &str, object_key: &str) -> String {
    let name = name.trim();
    if name.is_empty() {
        object_key.rsplit('/').next().unwrap_or(object_key).into()
    } else {
        name.chars().take(255).collect()
    }
}

fn photo(p: &PhotoRow) -> Photo {
    Photo {
        id: p.id,
        name: p.name.clone(),
        lens: p.lens,
        // Version: Nach einem Löschen vergibt SQLite die ID neu; der Browser-Cache darf nicht treffen.
        thumb_url: p
            .thumb_ready
            .then(|| format!("/api/photos/{}/thumb?v={}", p.id, p.uploaded_at_ms)),
        original_url: format!("/api/photos/{}/original?v={}", p.id, p.uploaded_at_ms),
    }
}

/// Fasst Fotos zu Auslösungen zusammen, älteste zuerst. Ort und Höhen kommen vom Wärmebild, sonst
/// vom ersten Foto mit Position: Alle Linsen der H20N sitzen in derselben Kamera.
pub fn shots(photos: &[PhotoRow]) -> Vec<PhotoShot> {
    let mut groups: BTreeMap<&str, Vec<&PhotoRow>> = BTreeMap::new();
    for p in photos {
        groups.entry(&p.shot_key).or_default().push(p);
    }
    let mut out: Vec<PhotoShot> = groups
        .into_iter()
        .map(|(key, mut members)| {
            members.sort_by_key(|p| (p.lens, p.id));
            let lead = members
                .iter()
                .find(|p| position(p.lat, p.lon).is_some())
                .unwrap_or(&members[0]);
            PhotoShot {
                id: key.to_string(),
                flight_id: members.iter().find_map(|p| p.flight_id),
                gateway_sn: lead.gateway_sn.clone(),
                aircraft_sn: members.iter().find_map(|p| p.aircraft_sn.clone()),
                taken_at_ms: members
                    .iter()
                    .map(|p| p.taken_at_ms.unwrap_or(p.uploaded_at_ms))
                    .min()
                    .expect("eine Gruppe hat mindestens ein Foto"),
                position: position(lead.lat, lead.lon),
                absolute_altitude_m: lead.absolute_altitude_m,
                relative_altitude_m: lead.relative_altitude_m,
                gimbal_yaw_deg: lead.gimbal_yaw_deg,
                mission_id: members.iter().find_map(|p| p.mission_id),
                photos: members.iter().map(|p| photo(p)).collect(),
            }
        })
        .collect();
    out.sort_by(|a, b| a.taken_at_ms.cmp(&b.taken_at_ms).then(a.id.cmp(&b.id)));
    out
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use super::*;

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32, s: u32) -> i64 {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, s)
            .unwrap()
            .timestamp_millis()
    }

    #[test]
    fn lens_comes_from_the_dji_suffix() {
        assert_eq!(lens_of("DJI_20261017143012_0001_T.JPG"), PhotoLens::Thermal);
        assert_eq!(lens_of("DJI_20261017143012_0001_W.jpg"), PhotoLens::Wide);
        assert_eq!(lens_of("DJI_20261017143012_0001_Z.JPG"), PhotoLens::Zoom);
        assert_eq!(lens_of("DJI_20261017143012_0001_V.JPG"), PhotoLens::Visible);
        assert_eq!(lens_of("DJI_20261017143012_0001_S.JPG"), PhotoLens::Unknown);
        assert_eq!(lens_of("DJI_0001.JPG"), PhotoLens::Unknown);
        assert_eq!(lens_of("ordner/DJI_1_T.JPG"), PhotoLens::Thermal);
    }

    #[test]
    fn all_lenses_of_one_shot_share_a_key() {
        let t = shot_key("RC1", "DJI_20261017143012_0001_T.JPG");
        assert_eq!(t, "RC1/DJI_20261017143012_0001");
        assert_eq!(shot_key("RC1", "DJI_20261017143012_0001_W.JPG"), t);
        assert_eq!(shot_key("RC1", "DJI_20261017143012_0001_Z.JPG"), t);
        assert_ne!(shot_key("RC2", "DJI_20261017143012_0001_Z.JPG"), t);
        assert_ne!(shot_key("RC1", "DJI_20261017143015_0002_T.JPG"), t);
        assert_eq!(
            shot_key("RC1", "DJI_20261017143012_0001_S.JPG"),
            "RC1/DJI_20261017143012_0001_S",
            "unbekannte Endung bleibt eine eigene Auslösung"
        );
    }

    #[test]
    fn invalid_positions_are_dropped() {
        assert_eq!(
            position(Some(52.96), Some(10.56)),
            Some(GeoPoint {
                lat: 52.96,
                lon: 10.56
            })
        );
        for (lat, lon) in [
            (Some(0.0), Some(0.0)),
            (Some(91.0), Some(10.0)),
            (Some(52.0), Some(181.0)),
            (Some(f64::NAN), Some(10.0)),
            (None, Some(10.0)),
            (Some(52.0), None),
        ] {
            assert_eq!(position(lat, lon), None, "{lat:?}/{lon:?}");
        }
    }

    #[test]
    fn created_time_with_offset_is_exact_without_offset_berlin() {
        assert_eq!(
            taken_at_ms("2026-10-17T14:30:12+02:00"),
            Some(utc(2026, 10, 17, 12, 30, 12))
        );
        assert_eq!(
            taken_at_ms("2026-10-17T12:30:12Z"),
            Some(utc(2026, 10, 17, 12, 30, 12))
        );
        // Sommerzeit: MESZ = UTC+2
        assert_eq!(
            taken_at_ms("2026-10-17 14:30:12"),
            Some(utc(2026, 10, 17, 12, 30, 12))
        );
        // Winterzeit: MEZ = UTC+1 (Umstellung am 25.10.2026)
        assert_eq!(
            taken_at_ms("2026-11-02 14:30:12"),
            Some(utc(2026, 11, 2, 13, 30, 12))
        );
        assert_eq!(
            taken_at_ms("2026:03:28 10:00:00"),
            Some(utc(2026, 3, 28, 9, 0, 0)),
            "EXIF-Schreibweise, Samstag vor der Umstellung"
        );
        assert_eq!(
            taken_at_ms("2026-03-29 04:00:00"),
            Some(utc(2026, 3, 29, 2, 0, 0)),
            "nach der Umstellung am 29.03.2026"
        );
        assert_eq!(taken_at_ms("1792248612"), Some(1_792_248_612_000));
        assert_eq!(taken_at_ms("1792248612000"), Some(1_792_248_612_000));
        assert_eq!(taken_at_ms(""), None);
        assert_eq!(taken_at_ms("gestern"), None);
    }

    #[test]
    fn last_sundays_match_the_2026_switches() {
        assert_eq!(last_sunday_1utc(2026, 3), Some(utc(2026, 3, 29, 1, 0, 0)));
        assert_eq!(last_sunday_1utc(2026, 10), Some(utc(2026, 10, 25, 1, 0, 0)));
    }

    #[test]
    fn object_keys_stay_below_the_prefix_of_their_controller() {
        assert_eq!(object_key_prefix("RC1"), "fotos/RC1");
        assert!(object_key_allowed("fotos/RC1/DJI_1_T.JPG", "RC1"));
        assert!(object_key_allowed("fotos/RC1/Ordner/DJI_1_T.JPG", "RC1"));
        for bad in [
            "fotos/RC2/DJI_1_T.JPG",
            "fotos/RC1/",
            "fotos/RC1",
            "fotos/RC1/../RC2/x.jpg",
            "fotos/RC1//x.jpg",
            "andere/RC1/x.jpg",
            "fotos/RC10/x.jpg",
        ] {
            assert!(!object_key_allowed(bad, "RC1"), "{bad}");
        }
    }

    fn stored(id: i64, name: &str, at: Option<GeoPoint>) -> PhotoRow {
        PhotoRow {
            id,
            name: name.into(),
            shot_key: shot_key("RC1", name),
            lens: lens_of(name),
            gateway_sn: "RC1".into(),
            aircraft_sn: Some("AC1".into()),
            flight_id: Some(6),
            mission_id: None,
            taken_at_ms: Some(1_000 + id),
            uploaded_at_ms: 5_000,
            lat: at.map(|p| p.lat),
            lon: at.map(|p| p.lon),
            absolute_altitude_m: Some(120.0),
            relative_altitude_m: Some(80.0 + id as f64),
            gimbal_yaw_deg: None,
            thumb_ready: id != 2,
        }
    }

    #[test]
    fn photos_group_into_shots_with_thermal_first() {
        let here = GeoPoint {
            lat: 52.96,
            lon: 10.56,
        };
        let shots = shots(&[
            stored(1, "DJI_20261017143012_0001_W.JPG", None),
            stored(2, "DJI_20261017143012_0001_T.JPG", Some(here)),
            stored(3, "DJI_20261017143012_0001_Z.JPG", None),
            stored(4, "DJI_20261017143020_0002_T.JPG", None),
        ]);
        assert_eq!(shots.len(), 2);
        let first = &shots[0];
        assert_eq!(first.id, "RC1/DJI_20261017143012_0001");
        assert_eq!(
            first.photos.iter().map(|p| p.lens).collect::<Vec<_>>(),
            [PhotoLens::Thermal, PhotoLens::Wide, PhotoLens::Zoom]
        );
        assert_eq!(first.position, Some(here));
        assert_eq!(
            first.relative_altitude_m,
            Some(82.0),
            "vom Foto mit Position"
        );
        assert_eq!(first.taken_at_ms, 1_001, "früheste Aufnahmezeit");
        assert_eq!(first.photos[0].thumb_url, None, "Vorschaubild fehlt noch");
        assert_eq!(
            first.photos[1].thumb_url.as_deref(),
            Some("/api/photos/1/thumb?v=5000"),
            "Version: eine wiederverwendete ID trifft keinen Browser-Cache"
        );
        assert_eq!(
            first.photos[1].original_url,
            "/api/photos/1/original?v=5000"
        );
        assert_eq!(shots[1].position, None, "ohne Position nicht auf der Karte");
    }

    #[test]
    fn display_name_falls_back_to_the_object_key() {
        assert_eq!(display_name("  ", "fotos/RC1/DJI_1_T.JPG"), "DJI_1_T.JPG");
        assert_eq!(display_name("DJI_1_T.JPG", "x"), "DJI_1_T.JPG");
    }
}

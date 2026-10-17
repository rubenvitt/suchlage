//! Prüfregeln für Kartenelemente. Rein: kein Speicher, keine Laufzeit.

use crate::contract::{MAP_COLORS, MapElementKind, MapPoint};

/// So viele Eckpunkte hält ein Element höchstens. Größere Flächen zeichnet niemand von Hand, und
/// der Body bleibt unter der Grenze des Routers (64 KiB).
pub const MAX_POINTS: usize = 1_000;
pub const MAX_NAME_CHARS: usize = 120;
/// Farbe, wenn Pilot 2 keine oder eine unlesbare schickt: Blau, die erste der DJI-Tabelle.
pub const DEFAULT_COLOR: &str = MAP_COLORS[0];

pub fn name(raw: &str) -> Result<String, String> {
    let name = raw.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_NAME_CHARS {
        return Err(format!("Name: 1 bis {MAX_NAME_CHARS} Zeichen"));
    }
    Ok(name.to_string())
}

/// Standardname, wenn Pilot 2 keinen schickt.
pub fn default_name(kind: MapElementKind) -> &'static str {
    match kind {
        MapElementKind::Point => "Punkt",
        MapElementKind::Line => "Linie",
        MapElementKind::Polygon => "Fläche",
    }
}

/// `#RRGGBB` in Großbuchstaben aus `#rrggbb`, `rrggbb` oder `0xRRGGBB`; sonst `None`.
pub fn normalize_color(raw: &str) -> Option<String> {
    let raw = raw.trim();
    let hex = raw
        .strip_prefix('#')
        .or_else(|| raw.strip_prefix("0x"))
        .or_else(|| raw.strip_prefix("0X"))
        .unwrap_or(raw);
    (hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| format!("#{}", hex.to_ascii_uppercase()))
}

/// Im Browser gilt nur die Palette: Pilot 2 kennt keine anderen Farben.
pub fn palette_color(raw: &str) -> Result<String, String> {
    normalize_color(raw)
        .filter(|c| MAP_COLORS.contains(&c.as_str()))
        .ok_or_else(|| "Farbe: eine der sechs Kartenfarben".to_string())
}

/// Prüft die Punkte zur Form. Ein geschlossener Ring verliert seinen wiederholten Schlusspunkt.
pub fn shape(kind: MapElementKind, mut points: Vec<MapPoint>) -> Result<Vec<MapPoint>, String> {
    if let Some(bad) = points.iter().find(|p| !valid(p)) {
        return Err(format!(
            "Koordinate außerhalb von WGS84: {} / {}",
            bad.lat, bad.lon
        ));
    }
    if kind == MapElementKind::Polygon
        && points.len() > 1
        && points.first().map(|p| (p.lat, p.lon)) == points.last().map(|p| (p.lat, p.lon))
    {
        points.pop();
    }
    let ok = match kind {
        MapElementKind::Point => points.len() == 1,
        MapElementKind::Line => (2..=MAX_POINTS).contains(&points.len()),
        MapElementKind::Polygon => (3..=MAX_POINTS).contains(&points.len()),
    };
    if !ok {
        return Err(match kind {
            MapElementKind::Point => "Ein Punkt hat genau eine Koordinate".into(),
            MapElementKind::Line => format!("Eine Linie hat 2 bis {MAX_POINTS} Punkte"),
            MapElementKind::Polygon => format!("Eine Fläche hat 3 bis {MAX_POINTS} Eckpunkte"),
        });
    }
    Ok(points)
}

fn valid(p: &MapPoint) -> bool {
    p.lat.is_finite()
        && p.lon.is_finite()
        && (-90.0..=90.0).contains(&p.lat)
        && (-180.0..=180.0).contains(&p.lon)
        && p.alt_m.is_none_or(f64::is_finite)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pt(lat: f64, lon: f64) -> MapPoint {
        MapPoint {
            lat,
            lon,
            alt_m: None,
        }
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(name("  Suchgebiet Nord ").unwrap(), "Suchgebiet Nord");
        assert!(name("   ").is_err());
        assert!(
            name(&"ä".repeat(MAX_NAME_CHARS)).is_ok(),
            "zählt Zeichen, nicht Bytes"
        );
        assert!(name(&"a".repeat(MAX_NAME_CHARS + 1)).is_err());
    }

    #[test]
    fn colors_are_normalized_from_every_dji_spelling() {
        assert_eq!(normalize_color("#0091ff").as_deref(), Some("#0091FF"));
        assert_eq!(normalize_color("0x2D8CF0").as_deref(), Some("#2D8CF0"));
        assert_eq!(normalize_color("e23c39").as_deref(), Some("#E23C39"));
        assert_eq!(normalize_color("#12345"), None);
        assert_eq!(normalize_color("blau"), None);
    }

    #[test]
    fn the_browser_only_gets_the_palette() {
        assert_eq!(palette_color("#ffbb00").unwrap(), "#FFBB00");
        assert!(
            palette_color("#0091FF").is_err(),
            "DJI-Beispielfarbe, nicht in der Tabelle"
        );
    }

    #[test]
    fn each_kind_needs_its_number_of_points() {
        let p = pt(52.43, 10.01);
        assert!(shape(MapElementKind::Point, vec![p]).is_ok());
        assert!(shape(MapElementKind::Point, vec![p, p]).is_err());
        assert!(shape(MapElementKind::Point, vec![]).is_err());
        assert!(shape(MapElementKind::Line, vec![p]).is_err());
        assert!(shape(MapElementKind::Line, vec![p, pt(52.44, 10.02)]).is_ok());
        assert!(shape(MapElementKind::Polygon, vec![p, pt(52.44, 10.02)]).is_err());
        let too_many = vec![p; MAX_POINTS + 1];
        assert!(shape(MapElementKind::Line, too_many).is_err());
    }

    #[test]
    fn a_closed_ring_loses_its_repeated_last_point() {
        let ring = vec![
            pt(52.0, 10.0),
            pt(52.1, 10.0),
            pt(52.1, 10.1),
            pt(52.0, 10.0),
        ];
        let open = shape(MapElementKind::Polygon, ring).unwrap();
        assert_eq!(open.len(), 3);
        // Ein Dreieck mit Schlusspunkt hat danach nur noch zwei Ecken und ist keine Fläche.
        let degenerate = vec![pt(52.0, 10.0), pt(52.1, 10.0), pt(52.0, 10.0)];
        assert!(shape(MapElementKind::Polygon, degenerate).is_err());
        // Bei Linien ist ein gleicher Anfang und Schluss erlaubt (Rundweg).
        let round = vec![pt(52.0, 10.0), pt(52.1, 10.0), pt(52.0, 10.0)];
        assert_eq!(shape(MapElementKind::Line, round).unwrap().len(), 3);
    }

    #[test]
    fn coordinates_outside_wgs84_are_rejected() {
        for bad in [
            pt(91.0, 10.0),
            pt(52.0, 181.0),
            pt(f64::NAN, 10.0),
            MapPoint {
                lat: 52.0,
                lon: 10.0,
                alt_m: Some(f64::INFINITY),
            },
        ] {
            assert!(shape(MapElementKind::Point, vec![bad]).is_err(), "{bad:?}");
        }
    }
}

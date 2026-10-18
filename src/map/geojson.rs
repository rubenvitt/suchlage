//! Einsatzkarte als GeoJSON für die Flugmappe (SUL-66): Kartenelemente und Flugspuren. Rein: kein
//! Speicher, keine Laufzeit.

use crate::contract::{
    FlightSummary, FmFeature, FmFeatureCollection, FmFeatureCollectionType, FmFeatureProperties,
    FmFeatureType, FmGeometry, FmPosition, MAP_COLORS, MapElement, MapElementKind, MapPoint,
    TrackPoint,
};

/// Flugspuren violett: neben blauen Suchgebieten und roten Punkten gut zu unterscheiden.
pub const TRACK_COLOR: &str = MAP_COLORS[4];

/// Ein Flug mit seiner Spur und dem Abschnitt der Flugmappe, der beim Start aktiv war.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
    pub flight: FlightSummary,
    pub flugmappe_section_id: Option<String>,
    pub segments: Vec<Vec<TrackPoint>>,
}

fn position(lat: f64, lon: f64) -> FmPosition {
    [lon, lat]
}

fn element_geometry(kind: MapElementKind, points: &[MapPoint]) -> Option<FmGeometry> {
    let mut coords: Vec<FmPosition> = points.iter().map(|p| position(p.lat, p.lon)).collect();
    match kind {
        MapElementKind::Point => coords
            .first()
            .map(|&c| FmGeometry::Point { coordinates: c }),
        MapElementKind::Line if coords.len() >= 2 => Some(FmGeometry::LineString {
            coordinates: coords,
        }),
        // Gespeichert ohne Schlusspunkt; GeoJSON verlangt den geschlossenen Ring.
        MapElementKind::Polygon if coords.len() >= 3 => {
            coords.push(coords[0]);
            Some(FmGeometry::Polygon {
                coordinates: vec![coords],
            })
        }
        _ => None,
    }
}

/// Ein Segment mit nur einem Punkt ist keine Linie und fällt weg; ohne Linie keine Spur.
fn track_geometry(segments: &[Vec<TrackPoint>]) -> Option<FmGeometry> {
    let lines: Vec<Vec<FmPosition>> = segments
        .iter()
        .filter(|s| s.len() >= 2)
        .map(|s| s.iter().map(|p| position(p.lat, p.lon)).collect())
        .collect();
    (!lines.is_empty()).then_some(FmGeometry::MultiLineString { coordinates: lines })
}

fn feature(geometry: FmGeometry, properties: FmFeatureProperties) -> FmFeature {
    FmFeature {
        kind: FmFeatureType::Feature,
        geometry,
        properties,
    }
}

/// Erst die Kartenelemente, dann die Spuren in der übergebenen Reihenfolge.
pub fn feature_collection(elements: &[MapElement], tracks: &[Track]) -> FmFeatureCollection {
    let elements = elements.iter().filter_map(|e| {
        let geometry = element_geometry(e.kind, &e.points)?;
        Some(feature(
            geometry,
            FmFeatureProperties::MapElement {
                id: e.id.clone(),
                name: e.name.clone(),
                color: e.color.clone(),
            },
        ))
    });
    let tracks = tracks.iter().filter_map(|t| {
        let geometry = track_geometry(&t.segments)?;
        Some(feature(
            geometry,
            FmFeatureProperties::FlightTrack {
                flight_id: t.flight.id,
                name: format!("Flug {}", t.flight.id),
                color: TRACK_COLOR.into(),
                flugmappe_section_id: t.flugmappe_section_id.clone(),
                started_at_ms: t.flight.started_at_ms,
                ended_at_ms: t.flight.ended_at_ms,
            },
        ))
    });
    FmFeatureCollection {
        kind: FmFeatureCollectionType::FeatureCollection,
        features: elements.chain(tracks).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::examples;

    #[test]
    fn elements_and_tracks_become_the_example_map() {
        let area = examples::map_elements().remove(0);
        let track = Track {
            flight: examples::flight_running(),
            flugmappe_section_id: Some(examples::FM_SECTION_IDS[1].into()),
            segments: examples::track_segments(),
        };
        assert_eq!(feature_collection(&[area], &[track]), examples::fm_map());
    }

    #[test]
    fn json_is_rfc_7946_with_longitude_first() {
        let mut pin = examples::map_elements().remove(1);
        pin.points[0].alt_m = Some(80.0);
        let json = serde_json::to_value(feature_collection(&[pin], &[])).unwrap();
        assert_eq!(json["type"], "FeatureCollection");
        let f = &json["features"][0];
        assert_eq!(f["type"], "Feature");
        assert_eq!(
            f["geometry"],
            serde_json::json!({"type": "Point", "coordinates": [10.0124, 52.4318]}),
            "Höhe aus Pilot 2 bleibt draußen"
        );
        assert_eq!(f["properties"]["kind"], "map_element");
        assert_eq!(f["properties"]["color"], "#E23C39");
    }

    #[test]
    fn polygons_are_closed_and_short_segments_dropped() {
        let area = examples::map_elements().remove(0);
        let collection = feature_collection(std::slice::from_ref(&area), &[]);
        let FmGeometry::Polygon { coordinates } = &collection.features[0].geometry else {
            panic!("Fläche erwartet");
        };
        assert_eq!(coordinates[0].len(), area.points.len() + 1);
        assert_eq!(coordinates[0].first(), coordinates[0].last());

        let p = |t_ms| TrackPoint {
            t_ms,
            lat: 52.0,
            lon: 10.0,
            elevation_m: None,
        };
        let only_points = Track {
            flight: examples::flight_running(),
            flugmappe_section_id: None,
            segments: vec![vec![p(1)], vec![]],
        };
        assert!(
            feature_collection(&[], std::slice::from_ref(&only_points))
                .features
                .is_empty()
        );
        let mixed = Track {
            segments: vec![vec![p(1)], vec![p(2), p(3)]],
            ..only_points
        };
        let FmGeometry::MultiLineString { coordinates } =
            &feature_collection(&[], &[mixed]).features[0].geometry
        else {
            panic!("Spur erwartet");
        };
        assert_eq!(coordinates.len(), 1);
    }
}

//! Flugspur in Segmenten. Eine Lücke über `SEGMENT_GAP_MS` beginnt ein neues Segment (Spec §5);
//! die Strecke über eine Lücke hinweg zählt nicht zur Flugstrecke.

use crate::contract::{SEGMENT_GAP_MS, TrackPoint};

#[derive(Debug, Clone, PartialEq)]
pub struct Appended {
    pub segment: u32,
    pub point: TrackPoint,
}

#[derive(Debug, Clone, Default)]
pub struct TrackBuilder {
    segments: Vec<Vec<TrackPoint>>,
    distance_m: f64,
    max_elevation_m: Option<f64>,
}

pub fn haversine_m(a: &TrackPoint, b: &TrackPoint) -> f64 {
    const R: f64 = 6_371_008.8;
    let (la1, la2) = (a.lat.to_radians(), b.lat.to_radians());
    let dla = la2 - la1;
    let dlo = (b.lon - a.lon).to_radians();
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    2.0 * R * h.sqrt().asin()
}

impl TrackBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Setzt eine gespeicherte Spur fort (Neustart mitten im Flug). Leere Segmente fallen weg.
    pub fn from_segments(segments: Vec<Vec<TrackPoint>>) -> Self {
        let mut t = Self::new();
        for segment in segments.into_iter().filter(|s| !s.is_empty()) {
            t.segments.push(Vec::new());
            for point in segment {
                t.extend_last_segment(point);
            }
        }
        t
    }

    /// `None`, wenn der Punkt nicht neuer ist als der letzte (Duplikat oder Rückläufer).
    pub fn push(&mut self, point: TrackPoint) -> Option<Appended> {
        match self.last_point().map(|p| p.t_ms) {
            Some(last) if point.t_ms <= last => return None,
            Some(last) if point.t_ms - last <= SEGMENT_GAP_MS => {}
            _ => self.segments.push(Vec::new()),
        }
        self.extend_last_segment(point.clone());
        let segment = u32::try_from(self.segments.len() - 1).expect("Segmentzahl passt in u32");
        Some(Appended { segment, point })
    }

    /// Hängt an das letzte Segment an; die Strecke zählt nur innerhalb dieses Segments.
    fn extend_last_segment(&mut self, point: TrackPoint) {
        if let Some(prev) = self.last_point() {
            self.distance_m += haversine_m(prev, &point);
        }
        if let Some(e) = point.elevation_m {
            self.max_elevation_m = Some(self.max_elevation_m.map_or(e, |m| m.max(e)));
        }
        self.segments
            .last_mut()
            .expect("ein Segment ist angelegt")
            .push(point);
    }

    fn last_point(&self) -> Option<&TrackPoint> {
        self.segments.last().and_then(|s| s.last())
    }

    pub fn segments(&self) -> &[Vec<TrackPoint>] {
        &self.segments
    }

    pub fn distance_m(&self) -> f64 {
        self.distance_m
    }

    pub fn max_elevation_m(&self) -> Option<f64> {
        self.max_elevation_m
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(t_ms: i64, lat: f64, elevation: f64) -> TrackPoint {
        TrackPoint {
            t_ms,
            lat,
            lon: 10.0,
            elevation_m: Some(elevation),
        }
    }

    #[test]
    fn gap_starts_new_segment_and_is_not_counted_as_distance() {
        let mut t = TrackBuilder::new();
        assert_eq!(t.push(p(0, 52.0, 5.0)).unwrap().segment, 0);
        assert_eq!(t.push(p(2_000, 52.001, 30.0)).unwrap().segment, 0);
        let after_gap = t.push(p(2_000 + SEGMENT_GAP_MS + 1, 52.010, 60.0)).unwrap();
        assert_eq!(after_gap.segment, 1);
        assert_eq!(t.segments().len(), 2);
        assert!(
            (t.distance_m() - 111.2).abs() < 1.0,
            "nur die 0,001° im ersten Segment: {}",
            t.distance_m()
        );
        assert_eq!(t.max_elevation_m(), Some(60.0));
    }

    #[test]
    fn exactly_the_gap_threshold_stays_in_the_segment() {
        let mut t = TrackBuilder::new();
        t.push(p(0, 52.0, 0.0));
        assert_eq!(t.push(p(SEGMENT_GAP_MS, 52.0, 0.0)).unwrap().segment, 0);
    }

    #[test]
    fn duplicates_and_out_of_order_points_are_dropped() {
        let mut t = TrackBuilder::new();
        t.push(p(2_000, 52.0, 0.0));
        assert_eq!(t.push(p(2_000, 52.0, 0.0)), None);
        assert_eq!(t.push(p(1_000, 52.0, 0.0)), None);
    }

    #[test]
    fn resumed_builder_recomputes_stats_and_continues_last_segment() {
        let mut t =
            TrackBuilder::from_segments(vec![vec![p(0, 52.0, 10.0), p(2_000, 52.001, 20.0)]]);
        assert!((t.distance_m() - 111.2).abs() < 1.0);
        assert_eq!(t.push(p(4_000, 52.002, 25.0)).unwrap().segment, 0);
    }

    #[test]
    fn resumed_builder_skips_empty_segments_and_ignores_distance_across_segments() {
        let first = vec![p(0, 52.0, 10.0), p(2_000, 52.001, 40.0)];
        let second = vec![p(20_000, 52.010, 15.0), p(22_000, 52.011, 15.0)];
        let t = TrackBuilder::from_segments(vec![Vec::new(), first, Vec::new(), second]);
        assert_eq!(t.segments().len(), 2);
        assert!(
            (t.distance_m() - 222.4).abs() < 2.0,
            "zweimal 0,001°, ohne die Lücke: {}",
            t.distance_m()
        );
        assert_eq!(t.max_elevation_m(), Some(40.0));
    }
}

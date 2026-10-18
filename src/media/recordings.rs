//! Videoablage von MediaMTX: `live/<RC-SN>/<YYYY-MM-DD_HH-MM-SS-ffffff>.mp4` unter
//! `SUCHLAGE_DATA_DIR/video` (`recordPath` in `deploy/mediamtx.m1.yml`, 10-Minuten-Segmente).

use std::path::{Path, PathBuf};

/// `recordSegmentDuration` in `deploy/mediamtx.m1.yml`.
const MAX_SEGMENT_MS: i64 = 10 * 60_000;

/// Ein aufgezeichnetes Segment einer Fernsteuerung.
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    pub rc: String,
    pub start_ms: i64,
    pub path: PathBuf,
}

/// Segmente der Fernsteuerung `rc`, nach Startzeit sortiert. Leer, wenn `live/<rc>` fehlt;
/// `NotFound`, wenn die Ablage selbst fehlt (nicht eingehängt).
pub fn scan(video_dir: &Path, rc: &str) -> std::io::Result<Vec<Segment>> {
    video_dir.metadata()?;
    let entries = match std::fs::read_dir(video_dir.join("live").join(rc)) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
        r => r?,
    };
    let mut segments = Vec::new();
    for entry in entries {
        let path = entry?.path();
        if path.extension().is_some_and(|e| e == "mp4")
            && let Some(start_ms) = path
                .file_stem()
                .and_then(|s| s.to_str())
                .and_then(parse_start)
        {
            segments.push(Segment {
                rc: rc.to_string(),
                start_ms,
                path,
            });
        }
    }
    segments.sort_by_key(|s| s.start_ms);
    Ok(segments)
}

/// Startzeit aus dem Dateinamen. MediaMTX schreibt Ortszeit des Containers; die ist UTC, solange
/// niemand `TZ` setzt – davon gehen wir aus.
fn parse_start(stem: &str) -> Option<i64> {
    chrono::NaiveDateTime::parse_from_str(stem, "%Y-%m-%d_%H-%M-%S-%6f")
        .ok()
        .map(|t| t.and_utc().timestamp_millis())
}

/// Segmente, die zu löschen sind. Rein; `delete` und `keep` sind Flugfenster `(start, ende)` in ms
/// derselben Fernsteuerung wie die Segmente – also je Fernsteuerung einmal aufrufen.
pub fn to_delete(segments: &[Segment], delete: &[(i64, i64)], keep: &[(i64, i64)]) -> Vec<PathBuf> {
    let mut sorted: Vec<&Segment> = segments.iter().collect();
    sorted.sort_by(|a, b| (&a.rc, a.start_ms).cmp(&(&b.rc, b.start_ms)));
    let overlaps = |windows: &[(i64, i64)], (start, end): (i64, i64)| {
        windows.iter().any(|&(from, to)| start < to && from < end)
    };
    sorted
        .iter()
        .enumerate()
        .filter_map(|(i, seg)| {
            let next = sorted
                .get(i + 1)
                .filter(|n| n.rc == seg.rc)
                .map_or(i64::MAX, |n| n.start_ms);
            let span = (seg.start_ms, next.min(seg.start_ms + MAX_SEGMENT_MS));
            (overlaps(delete, span) && !overlaps(keep, span)).then(|| seg.path.clone())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: i64 = 60_000;

    fn seg(start_ms: i64) -> Segment {
        Segment {
            rc: "RC1".into(),
            start_ms,
            path: PathBuf::from(format!("{start_ms}.mp4")),
        }
    }

    fn paths(starts: &[i64]) -> Vec<PathBuf> {
        starts.iter().map(|s| seg(*s).path).collect()
    }

    #[test]
    fn parses_mediamtx_names_as_utc() {
        let dir = tempfile::tempdir().unwrap();
        let rc_dir = dir.path().join("live/RC1");
        std::fs::create_dir_all(&rc_dir).unwrap();
        std::fs::write(rc_dir.join("2026-10-18_12-30-00-123456.mp4"), b"").unwrap();
        std::fs::write(rc_dir.join("2026-10-18_12-20-00-000000.mp4"), b"").unwrap();
        std::fs::write(rc_dir.join("notiz.txt"), b"").unwrap();
        std::fs::write(rc_dir.join("kaputt.mp4"), b"").unwrap();
        let found = scan(dir.path(), "RC1").unwrap();
        assert_eq!(
            found,
            vec![
                Segment {
                    rc: "RC1".into(),
                    start_ms: 1_792_326_000_000,
                    path: rc_dir.join("2026-10-18_12-20-00-000000.mp4"),
                },
                Segment {
                    rc: "RC1".into(),
                    start_ms: 1_792_326_600_123,
                    path: rc_dir.join("2026-10-18_12-30-00-123456.mp4"),
                },
            ]
        );
    }

    #[test]
    fn missing_rc_dir_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(scan(dir.path(), "RC1").unwrap(), vec![]);
        let gone = dir.path().join("fehlt");
        assert_eq!(
            scan(&gone, "RC1").unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
    }

    #[test]
    fn deletes_overlapping_segments() {
        let segs = [seg(0), seg(10 * MIN), seg(20 * MIN), seg(30 * MIN)];
        // Flug 12–25 min: Segmente ab 10 und 20; das ab 30 beginnt erst danach, das ab 0 endet bei 10.
        assert_eq!(
            to_delete(&segs, &[(12 * MIN, 25 * MIN)], &[]),
            paths(&[10 * MIN, 20 * MIN])
        );
        // Halboffen: Ein Flug, der genau beim Segmentstart endet, berührt es nicht.
        assert_eq!(to_delete(&segs, &[(5 * MIN, 10 * MIN)], &[]), paths(&[0]));
    }

    #[test]
    fn keeps_segment_shared_with_remaining_flight() {
        let segs = [seg(0), seg(10 * MIN), seg(20 * MIN)];
        // Gelöschter Flug 2–15 min, verbleibender Flug 14–25 min teilt das Segment ab 10.
        assert_eq!(
            to_delete(&segs, &[(2 * MIN, 15 * MIN)], &[(14 * MIN, 25 * MIN)]),
            paths(&[0])
        );
    }

    #[test]
    fn segment_end_is_next_start_or_ten_minutes() {
        // Nächstes Segment nach 4 min: das erste endet dort, ein Flug ab 5 min berührt es nicht.
        let segs = [seg(0), seg(4 * MIN)];
        assert_eq!(
            to_delete(&segs, &[(5 * MIN, 6 * MIN)], &[]),
            paths(&[4 * MIN])
        );
        // Letztes Segment endet nach höchstens 10 min.
        assert_eq!(
            to_delete(&segs, &[(14 * MIN, 20 * MIN)], &[]),
            Vec::<PathBuf>::new()
        );
        assert_eq!(
            to_delete(&segs, &[(13 * MIN, 20 * MIN)], &[]),
            paths(&[4 * MIN])
        );
        // Lücke in der Aufzeichnung: Ende nach 10 min, nicht erst beim nächsten Start.
        let gap = [seg(0), seg(60 * MIN)];
        assert_eq!(
            to_delete(&gap, &[(30 * MIN, 40 * MIN)], &[]),
            Vec::<PathBuf>::new()
        );
    }
}

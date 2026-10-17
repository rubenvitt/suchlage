use sqlx::{Row, SqlitePool};

use crate::contract::{FlightEndReason, FlightSummary, TrackPoint};

fn reason_str(r: Option<FlightEndReason>) -> Option<&'static str> {
    r.map(|r| match r {
        FlightEndReason::Landed => "landed",
        FlightEndReason::ConnectionLost => "connection_lost",
    })
}

fn summary(row: &sqlx::sqlite::SqliteRow) -> FlightSummary {
    FlightSummary {
        id: row.get("id"),
        aircraft_sn: row.get("aircraft_sn"),
        gateway_sn: row.get("gateway_sn"),
        started_at_ms: row.get("started_at_ms"),
        ended_at_ms: row.get("ended_at_ms"),
        end_reason: row
            .get::<Option<String>, _>("end_reason")
            .map(|r| match r.as_str() {
                "landed" => FlightEndReason::Landed,
                _ => FlightEndReason::ConnectionLost,
            }),
        max_elevation_m: row.get("max_elevation_m"),
        distance_m: row.get("distance_m"),
    }
}

/// `SELECT <Spalten> FROM flights <Rest>` als `&'static str`: sqlx 0.9 nimmt nur Literale ohne Prüfung an.
macro_rules! select_flights {
    ($tail:literal) => {
        concat!(
            "SELECT id, aircraft_sn, gateway_sn, started_at_ms, ended_at_ms, end_reason, max_elevation_m, distance_m FROM flights ",
            $tail
        )
    };
}

pub async fn insert(pool: &SqlitePool, f: &FlightSummary) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO flights (id, aircraft_sn, gateway_sn, started_at_ms, ended_at_ms, end_reason, max_elevation_m, distance_m) VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(f.id)
    .bind(&f.aircraft_sn)
    .bind(&f.gateway_sn)
    .bind(f.started_at_ms)
    .bind(f.ended_at_ms)
    .bind(reason_str(f.end_reason))
    .bind(f.max_elevation_m)
    .bind(f.distance_m)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update(pool: &SqlitePool, f: &FlightSummary) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE flights SET ended_at_ms = ?, end_reason = ?, max_elevation_m = ?, distance_m = ? WHERE id = ?",
    )
    .bind(f.ended_at_ms)
    .bind(reason_str(f.end_reason))
    .bind(f.max_elevation_m)
    .bind(f.distance_m)
    .bind(f.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Mehrere Punkte in einer Transaktion. Bereits vorhandene (gleiche Zeit) werden übersprungen.
pub async fn append_points(
    pool: &SqlitePool,
    flight_id: i64,
    points: &[(u32, TrackPoint)],
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    for (segment, p) in points {
        sqlx::query(
            "INSERT OR IGNORE INTO track_points (flight_id, t_ms, segment, lat, lon, elevation_m) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(flight_id)
        .bind(p.t_ms)
        .bind(i64::from(*segment))
        .bind(p.lat)
        .bind(p.lon)
        .bind(p.elevation_m)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

pub async fn list(pool: &SqlitePool, limit: i64) -> Result<Vec<FlightSummary>, sqlx::Error> {
    let rows = sqlx::query(select_flights!(
        "ORDER BY started_at_ms DESC, id DESC LIMIT ?"
    ))
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(summary).collect())
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<FlightSummary>, sqlx::Error> {
    let row = sqlx::query(select_flights!("WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(summary))
}

/// Spurpunkte in Zeitfolge, an jedem Segmentwechsel getrennt.
pub async fn segments(pool: &SqlitePool, id: i64) -> Result<Vec<Vec<TrackPoint>>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT segment, t_ms, lat, lon, elevation_m FROM track_points WHERE flight_id = ? ORDER BY t_ms",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    let mut out: Vec<Vec<TrackPoint>> = Vec::new();
    let mut current: Option<i64> = None;
    for row in rows {
        let segment: i64 = row.get("segment");
        if current != Some(segment) {
            out.push(Vec::new());
            current = Some(segment);
        }
        let point = TrackPoint {
            t_ms: row.get("t_ms"),
            lat: row.get("lat"),
            lon: row.get("lon"),
            elevation_m: row.get("elevation_m"),
        };
        out.last_mut()
            .expect("Segment wurde eben angelegt")
            .push(point);
    }
    Ok(out)
}

/// Laufende Flüge samt Spur, für das Resume nach einem Neustart (Spec §5).
pub async fn open(
    pool: &SqlitePool,
) -> Result<Vec<(FlightSummary, Vec<Vec<TrackPoint>>)>, sqlx::Error> {
    let rows = sqlx::query(select_flights!("WHERE ended_at_ms IS NULL"))
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let f = summary(&row);
        let segs = segments(pool, f.id).await?;
        out.push((f, segs));
    }
    Ok(out)
}

pub async fn next_id(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) + 1 FROM flights")
        .fetch_one(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::error::AppError;

    fn flight(id: i64, aircraft: &str) -> FlightSummary {
        FlightSummary {
            id,
            aircraft_sn: aircraft.into(),
            gateway_sn: "RC1".into(),
            started_at_ms: id * 1_000,
            ended_at_ms: None,
            end_reason: None,
            max_elevation_m: None,
            distance_m: 0.0,
        }
    }

    fn pt(t: i64) -> TrackPoint {
        TrackPoint {
            t_ms: t,
            lat: 52.0,
            lon: 10.0,
            elevation_m: Some(1.0),
        }
    }

    #[tokio::test]
    async fn flight_lifecycle_with_segments() {
        let pool = test_pool().await;
        assert_eq!(next_id(&pool).await.unwrap(), 1);
        insert(&pool, &flight(1, "AC1")).await.unwrap();
        append_points(&pool, 1, &[(0, pt(1)), (0, pt(2)), (1, pt(20))])
            .await
            .unwrap();
        append_points(&pool, 1, &[(1, pt(20))]).await.unwrap();
        assert_eq!(
            segments(&pool, 1)
                .await
                .unwrap()
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            vec![2, 1]
        );
        assert_eq!(open(&pool).await.unwrap().len(), 1);

        let done = FlightSummary {
            ended_at_ms: Some(30),
            end_reason: Some(FlightEndReason::Landed),
            distance_m: 12.5,
            ..flight(1, "AC1")
        };
        update(&pool, &done).await.unwrap();
        assert_eq!(get(&pool, 1).await.unwrap(), Some(done));
        assert!(open(&pool).await.unwrap().is_empty());
        assert_eq!(next_id(&pool).await.unwrap(), 2);
    }

    #[tokio::test]
    async fn second_open_flight_for_the_same_aircraft_is_a_conflict() {
        let pool = test_pool().await;
        insert(&pool, &flight(1, "AC1")).await.unwrap();
        let err: AppError = insert(&pool, &flight(2, "AC1")).await.unwrap_err().into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        insert(&pool, &flight(3, "AC2")).await.unwrap();

        // Der Index gilt nur für offene Flüge: nach der Landung darf AC1 wieder starten.
        let landed = FlightSummary {
            ended_at_ms: Some(1_500),
            end_reason: Some(FlightEndReason::Landed),
            ..flight(1, "AC1")
        };
        update(&pool, &landed).await.unwrap();
        insert(&pool, &flight(4, "AC1")).await.unwrap();
        let open_ac1: Vec<i64> = open(&pool)
            .await
            .unwrap()
            .iter()
            .filter(|(f, _)| f.aircraft_sn == "AC1")
            .map(|(f, _)| f.id)
            .collect();
        assert_eq!(open_ac1, vec![4]);
    }

    #[tokio::test]
    async fn list_is_newest_first() {
        let pool = test_pool().await;
        for id in 1..=3 {
            let mut f = flight(id, &format!("AC{id}"));
            f.ended_at_ms = Some(id * 1_000 + 1);
            f.end_reason = Some(FlightEndReason::Landed);
            insert(&pool, &f).await.unwrap();
        }
        let ids: Vec<i64> = list(&pool, 2).await.unwrap().iter().map(|f| f.id).collect();
        assert_eq!(ids, vec![3, 2]);
    }
}

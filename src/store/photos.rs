use sqlx::{Row, SqlitePool};

use crate::contract::PhotoLens;

/// Gespeichertes Foto mit allem, was eine Auslösung braucht.
#[derive(Debug, Clone, PartialEq)]
pub struct PhotoRow {
    pub id: i64,
    pub name: String,
    pub shot_key: String,
    pub lens: PhotoLens,
    pub gateway_sn: String,
    pub aircraft_sn: Option<String>,
    pub flight_id: Option<i64>,
    pub mission_id: Option<i64>,
    pub taken_at_ms: Option<i64>,
    pub uploaded_at_ms: i64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub absolute_altitude_m: Option<f64>,
    pub relative_altitude_m: Option<f64>,
    pub gimbal_yaw_deg: Option<f64>,
    pub thumb_ready: bool,
}

/// Ein Foto, wie es der `upload-callback` meldet, nach den Regeln aufbereitet.
#[derive(Debug, Clone, PartialEq)]
pub struct NewPhoto {
    pub object_key: String,
    pub name: String,
    pub fingerprint: Option<String>,
    pub tiny_fingerprint: Option<String>,
    pub shot_key: String,
    pub lens: PhotoLens,
    pub gateway_sn: String,
    pub aircraft_sn: Option<String>,
    pub flight_id: Option<i64>,
    pub taken_at_ms: Option<i64>,
    pub uploaded_at_ms: i64,
    pub lat: Option<f64>,
    pub lon: Option<f64>,
    pub absolute_altitude_m: Option<f64>,
    pub relative_altitude_m: Option<f64>,
    pub gimbal_yaw_deg: Option<f64>,
    pub payload_model_key: Option<String>,
}

macro_rules! columns {
    () => {
        "id, name, shot_key, lens, gateway_sn, aircraft_sn, flight_id, taken_at_ms, uploaded_at_ms, \
         lat, lon, absolute_altitude_m, relative_altitude_m, gimbal_yaw_deg, thumb_ready, mission_id"
    };
}

fn lens_str(lens: PhotoLens) -> &'static str {
    match lens {
        PhotoLens::Thermal => "thermal",
        PhotoLens::Wide => "wide",
        PhotoLens::Zoom => "zoom",
        PhotoLens::Visible => "visible",
        PhotoLens::Unknown => "unknown",
    }
}

fn row(r: &sqlx::sqlite::SqliteRow) -> PhotoRow {
    let lens = match r.get::<&str, _>("lens") {
        "thermal" => PhotoLens::Thermal,
        "wide" => PhotoLens::Wide,
        "zoom" => PhotoLens::Zoom,
        "visible" => PhotoLens::Visible,
        _ => PhotoLens::Unknown,
    };
    PhotoRow {
        id: r.get("id"),
        name: r.get("name"),
        shot_key: r.get("shot_key"),
        lens,
        gateway_sn: r.get("gateway_sn"),
        aircraft_sn: r.get("aircraft_sn"),
        flight_id: r.get("flight_id"),
        mission_id: r.get("mission_id"),
        taken_at_ms: r.get("taken_at_ms"),
        uploaded_at_ms: r.get("uploaded_at_ms"),
        lat: r.get("lat"),
        lon: r.get("lon"),
        absolute_altitude_m: r.get("absolute_altitude_m"),
        relative_altitude_m: r.get("relative_altitude_m"),
        gimbal_yaw_deg: r.get("gimbal_yaw_deg"),
        thumb_ready: r.get::<i64, _>("thumb_ready") != 0,
    }
}

/// Legt an oder überschreibt die Angaben zum selben Objekt (Pilot 2 wiederholt den Callback nach
/// einem Fehler). Das Vorschaubild bleibt, das Objekt ist dasselbe. Ohne Flug bleibt ein schon
/// zugeordnetes Foto in seinem Einsatz, auch wenn inzwischen ein anderer läuft. Liefert die ID.
pub async fn upsert(pool: &SqlitePool, p: &NewPhoto) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO photos (object_key, name, fingerprint, tiny_fingerprint, shot_key, lens, \
         gateway_sn, aircraft_sn, flight_id, taken_at_ms, uploaded_at_ms, lat, lon, \
         absolute_altitude_m, relative_altitude_m, gimbal_yaw_deg, payload_model_key, mission_id) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, \
         (SELECT CASE WHEN ?18 IS NULL THEN (SELECT id FROM missions WHERE state = 'running') ELSE (SELECT mission_id FROM flights WHERE id = ?18) END)) \
         ON CONFLICT (object_key) DO UPDATE SET name = excluded.name, \
         fingerprint = excluded.fingerprint, tiny_fingerprint = excluded.tiny_fingerprint, \
         shot_key = excluded.shot_key, lens = excluded.lens, aircraft_sn = excluded.aircraft_sn, \
         flight_id = excluded.flight_id, taken_at_ms = excluded.taken_at_ms, lat = excluded.lat, \
         lon = excluded.lon, absolute_altitude_m = excluded.absolute_altitude_m, \
         relative_altitude_m = excluded.relative_altitude_m, \
         gimbal_yaw_deg = excluded.gimbal_yaw_deg, payload_model_key = excluded.payload_model_key, \
         mission_id = CASE WHEN excluded.flight_id IS NOT NULL THEN excluded.mission_id \
         ELSE COALESCE(photos.mission_id, excluded.mission_id) END \
         RETURNING id",
    )
    .bind(&p.object_key)
    .bind(&p.name)
    .bind(&p.fingerprint)
    .bind(&p.tiny_fingerprint)
    .bind(&p.shot_key)
    .bind(lens_str(p.lens))
    .bind(&p.gateway_sn)
    .bind(&p.aircraft_sn)
    .bind(p.flight_id)
    .bind(p.taken_at_ms)
    .bind(p.uploaded_at_ms)
    .bind(p.lat)
    .bind(p.lon)
    .bind(p.absolute_altitude_m)
    .bind(p.relative_altitude_m)
    .bind(p.gimbal_yaw_deg)
    .bind(&p.payload_model_key)
    .bind(p.flight_id)
    .fetch_one(pool)
    .await
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<PhotoRow>, sqlx::Error> {
    let r = sqlx::query(concat!("SELECT ", columns!(), " FROM photos WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(row))
}

/// Objektschlüssel eines Fotos (für Vorschaubild und Download).
pub async fn object_key(pool: &SqlitePool, id: i64) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT object_key FROM photos WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
}

/// Alle Fotos der Auslösung.
pub async fn of_shot(pool: &SqlitePool, shot_key: &str) -> Result<Vec<PhotoRow>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM photos WHERE shot_key = ? ORDER BY id"
    ))
    .bind(shot_key)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

/// Fotos mit Aufnahme- oder Uploadzeit ab `since_ms`, höchstens `limit`, neueste zuerst gelesen.
pub async fn since(
    pool: &SqlitePool,
    since_ms: i64,
    limit: i64,
) -> Result<Vec<PhotoRow>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM photos WHERE COALESCE(taken_at_ms, uploaded_at_ms) >= ? \
         ORDER BY COALESCE(taken_at_ms, uploaded_at_ms) DESC, id DESC LIMIT ?"
    ))
    .bind(since_ms)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

/// Alle Fotos eines Einsatzes, neueste zuerst.
pub async fn in_mission(pool: &SqlitePool, mission: i64) -> Result<Vec<PhotoRow>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM photos WHERE mission_id IS ? \
         ORDER BY COALESCE(taken_at_ms, uploaded_at_ms) DESC, id DESC"
    ))
    .bind(mission)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

/// Fotos ohne Einsatz mit Aufnahme- oder Uploadzeit ab `since_ms`, neueste zuerst.
pub async fn since_unassigned(
    pool: &SqlitePool,
    since_ms: i64,
) -> Result<Vec<PhotoRow>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM photos WHERE mission_id IS NULL AND COALESCE(taken_at_ms, uploaded_at_ms) >= ? \
         ORDER BY COALESCE(taken_at_ms, uploaded_at_ms) DESC, id DESC"
    ))
    .bind(since_ms)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

/// Auslösungen je Flug der Flüge eines Einsatzes (mehrere Linsen zählen einmal).
pub async fn shots_per_flight(
    pool: &SqlitePool,
    mission: i64,
) -> Result<Vec<(i64, u32)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT flight_id, COUNT(DISTINCT shot_key) FROM photos \
         WHERE flight_id IN (SELECT id FROM flights WHERE mission_id = ?) GROUP BY flight_id",
    )
    .bind(mission)
    .fetch_all(pool)
    .await
}

/// Alle Fotodateien eines Einsatzes, mit und ohne Flug.
pub async fn of_mission_all(pool: &SqlitePool, mission: i64) -> Result<Vec<PhotoRow>, sqlx::Error> {
    in_mission(pool, mission).await
}

/// Löscht genau diese Fotos (deren Dateien schon weg sind); Anzahl gelöschter Zeilen. Eine Linse,
/// die danach ankam, behält ihre Zeile.
pub async fn delete_ids(pool: &SqlitePool, ids: &[i64]) -> Result<u64, sqlx::Error> {
    let done = sqlx::query("DELETE FROM photos WHERE id IN (SELECT value FROM json_each(?))")
        .bind(ids_json(ids))
        .execute(pool)
        .await?;
    Ok(done.rows_affected())
}

/// IDs als JSON-Liste für `json_each(?)`: sqlx bindet keine Listen.
pub(crate) fn ids_json(ids: &[i64]) -> String {
    serde_json::to_string(ids).expect("Zahlen sind JSON")
}

pub async fn of_flight(pool: &SqlitePool, flight_id: i64) -> Result<Vec<PhotoRow>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM photos WHERE flight_id = ? ORDER BY id"
    ))
    .bind(flight_id)
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

pub async fn set_thumb_ready(pool: &SqlitePool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE photos SET thumb_ready = 1 WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Welche der Tiny-Fingerprints sind schon bekannt? Reihenfolge wie in der Anfrage.
pub async fn known_tiny_fingerprints(
    pool: &SqlitePool,
    candidates: &[String],
) -> Result<Vec<String>, sqlx::Error> {
    let mut known = Vec::new();
    for c in candidates {
        let hit: Option<i64> =
            sqlx::query_scalar("SELECT 1 FROM photos WHERE tiny_fingerprint = ? LIMIT 1")
                .bind(c)
                .fetch_optional(pool)
                .await?;
        if hit.is_some() {
            known.push(c.clone());
        }
    }
    Ok(known)
}

pub async fn fingerprint_known(pool: &SqlitePool, fingerprint: &str) -> Result<bool, sqlx::Error> {
    let hit: Option<i64> = sqlx::query_scalar("SELECT 1 FROM photos WHERE fingerprint = ? LIMIT 1")
        .bind(fingerprint)
        .fetch_optional(pool)
        .await?;
    Ok(hit.is_some())
}

/// Flug, in dessen Zeitfenster (± `slack_ms`) die Aufnahme fällt: von dieser Fernsteuerung oder
/// diesem Fluggerät, bei mehreren der zuletzt gestartete.
pub async fn flight_at(
    pool: &SqlitePool,
    gateway_sn: &str,
    aircraft_sn: Option<&str>,
    t_ms: i64,
    slack_ms: i64,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM flights WHERE (gateway_sn = ? OR aircraft_sn = ?) \
         AND started_at_ms - ? <= ? AND (ended_at_ms IS NULL OR ended_at_ms + ? >= ?) \
         ORDER BY started_at_ms DESC LIMIT 1",
    )
    .bind(gateway_sn)
    .bind(aircraft_sn)
    .bind(slack_ms)
    .bind(t_ms)
    .bind(slack_ms)
    .bind(t_ms)
    .fetch_optional(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FlightSummary;

    fn new(object_key: &str, name: &str, lens: PhotoLens) -> NewPhoto {
        NewPhoto {
            object_key: object_key.into(),
            name: name.into(),
            fingerprint: Some(format!("fp-{name}")),
            tiny_fingerprint: Some(format!("tiny-{name}")),
            shot_key: "RC1/DJI_1".into(),
            lens,
            gateway_sn: "RC1".into(),
            aircraft_sn: Some("AC1".into()),
            flight_id: None,
            taken_at_ms: Some(10_000),
            uploaded_at_ms: 20_000,
            lat: Some(52.96),
            lon: Some(10.56),
            absolute_altitude_m: Some(120.5),
            relative_altitude_m: Some(80.0),
            gimbal_yaw_deg: Some(-12.5),
            payload_model_key: Some("1-61-0".into()),
        }
    }

    #[tokio::test]
    async fn upsert_keeps_one_row_per_object_and_reads_back() {
        let pool = crate::db::test_pool().await;
        let a = upsert(
            &pool,
            &new("fotos/RC1/DJI_1_T.JPG", "DJI_1_T.JPG", PhotoLens::Thermal),
        )
        .await
        .unwrap();
        let mut again = new("fotos/RC1/DJI_1_T.JPG", "DJI_1_T.JPG", PhotoLens::Thermal);
        again.relative_altitude_m = Some(81.0);
        assert_eq!(upsert(&pool, &again).await.unwrap(), a, "dieselbe Zeile");
        set_thumb_ready(&pool, a).await.unwrap();
        let p = get(&pool, a).await.unwrap().unwrap();
        assert_eq!(p.relative_altitude_m, Some(81.0));
        assert_eq!(p.lens, PhotoLens::Thermal);
        assert!(p.thumb_ready);
        assert_eq!(
            object_key(&pool, a).await.unwrap().as_deref(),
            Some("fotos/RC1/DJI_1_T.JPG")
        );
        upsert(
            &pool,
            &new("fotos/RC1/DJI_1_W.JPG", "DJI_1_W.JPG", PhotoLens::Wide),
        )
        .await
        .unwrap();
        assert_eq!(of_shot(&pool, "RC1/DJI_1").await.unwrap().len(), 2);
        assert_eq!(since(&pool, 10_000, 10).await.unwrap().len(), 2);
        assert_eq!(since(&pool, 10_001, 10).await.unwrap().len(), 0);
        assert!(fingerprint_known(&pool, "fp-DJI_1_W.JPG").await.unwrap());
        assert!(!fingerprint_known(&pool, "fp-x").await.unwrap());
        assert_eq!(
            known_tiny_fingerprints(&pool, &["x".into(), "tiny-DJI_1_T.JPG".into()])
                .await
                .unwrap(),
            ["tiny-DJI_1_T.JPG"]
        );
    }

    #[tokio::test]
    async fn the_flight_is_found_by_controller_or_aircraft_and_time() {
        let pool = crate::db::test_pool().await;
        let flight = |id, started, ended: Option<i64>| FlightSummary {
            id,
            aircraft_sn: "AC1".into(),
            gateway_sn: "RC1".into(),
            started_at_ms: started,
            ended_at_ms: ended,
            end_reason: ended.map(|_| crate::contract::FlightEndReason::Landed),
            max_elevation_m: None,
            distance_m: 0.0,
        };
        crate::store::flights::insert(&pool, &flight(1, 100_000, Some(200_000)))
            .await
            .unwrap();
        crate::store::flights::insert(&pool, &flight(2, 300_000, None))
            .await
            .unwrap();
        let cases = [
            ("RC1", None, 150_000, Some(1), "im Flug"),
            ("RC1", None, 205_000, Some(1), "Spielraum nach der Landung"),
            ("RC1", None, 250_000, None, "zwischen zwei Flügen"),
            ("RC9", Some("AC1"), 150_000, Some(1), "über das Fluggerät"),
            ("RC1", None, 900_000, Some(2), "offener Flug"),
            ("RC9", Some("AC9"), 150_000, None, "fremde Geräte"),
        ];
        for (gw, ac, t, expected, why) in cases {
            assert_eq!(
                flight_at(&pool, gw, ac, t, 10_000).await.unwrap(),
                expected,
                "{why}"
            );
        }
    }
}

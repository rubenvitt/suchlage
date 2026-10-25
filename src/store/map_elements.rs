use sqlx::{Row, SqlitePool};

use crate::contract::{MapElement, MapElementKind, MapElementOrigin, MapPoint};

macro_rules! columns {
    () => {
        "id, name, kind, points, color, clamp_to_ground, author, origin, created_at_ms, updated_at_ms, mission_id"
    };
}

fn kind_str(kind: MapElementKind) -> &'static str {
    match kind {
        MapElementKind::Point => "point",
        MapElementKind::Line => "line",
        MapElementKind::Polygon => "polygon",
    }
}

fn origin_str(origin: MapElementOrigin) -> &'static str {
    match origin {
        MapElementOrigin::Pilot => "pilot",
        MapElementOrigin::Web => "web",
    }
}

fn row(r: &sqlx::sqlite::SqliteRow) -> Result<MapElement, sqlx::Error> {
    let kind = match r.get::<&str, _>("kind") {
        "point" => MapElementKind::Point,
        "line" => MapElementKind::Line,
        _ => MapElementKind::Polygon,
    };
    let origin = if r.get::<&str, _>("origin") == "pilot" {
        MapElementOrigin::Pilot
    } else {
        MapElementOrigin::Web
    };
    let points: Vec<MapPoint> =
        serde_json::from_str(r.get("points")).map_err(|e| sqlx::Error::Decode(Box::new(e)))?;
    Ok(MapElement {
        id: r.get("id"),
        name: r.get("name"),
        kind,
        points,
        color: r.get("color"),
        clamp_to_ground: r.get::<i64, _>("clamp_to_ground") != 0,
        author: r.get("author"),
        origin,
        created_at_ms: r.get("created_at_ms"),
        updated_at_ms: r.get("updated_at_ms"),
        mission_id: r.get("mission_id"),
    })
}

fn points_json(points: &[MapPoint]) -> String {
    serde_json::to_string(points).expect("Punkte sind serialisierbar")
}

/// Älteste zuerst: In dieser Reihenfolge zeichnet die Karte, Neues liegt oben.
pub async fn list(pool: &SqlitePool) -> Result<Vec<MapElement>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM map_elements ORDER BY created_at_ms, id"
    ))
    .fetch_all(pool)
    .await?;
    rows.iter().map(row).collect()
}

/// Elemente eines Einsatzes, mit `None` die ohne Einsatz.
pub async fn list_in(
    pool: &SqlitePool,
    mission: Option<i64>,
) -> Result<Vec<MapElement>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM map_elements WHERE mission_id IS ? ORDER BY created_at_ms, id"
    ))
    .bind(mission)
    .fetch_all(pool)
    .await?;
    rows.iter().map(row).collect()
}

pub async fn get(pool: &SqlitePool, id: &str) -> Result<Option<MapElement>, sqlx::Error> {
    let r = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM map_elements WHERE id = ?"
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    r.as_ref().map(row).transpose()
}

/// `false`, wenn es die ID schon gibt; dann bleibt der gespeicherte Stand unverändert.
pub async fn insert<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
    e: &MapElement,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(concat!(
        "INSERT INTO map_elements (",
        columns!(),
        ") VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT (id) DO NOTHING"
    ))
    .bind(&e.id)
    .bind(&e.name)
    .bind(kind_str(e.kind))
    .bind(points_json(&e.points))
    .bind(&e.color)
    .bind(i64::from(e.clamp_to_ground))
    .bind(&e.author)
    .bind(origin_str(e.origin))
    .bind(e.created_at_ms)
    .bind(e.updated_at_ms)
    .bind(e.mission_id)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Schreibt alles außer ID, Urheber, Herkunft und Anlagezeit. `false`, wenn es die ID nicht gibt.
pub async fn update(pool: &SqlitePool, e: &MapElement) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE map_elements SET name = ?, kind = ?, points = ?, color = ?, clamp_to_ground = ?, updated_at_ms = ? WHERE id = ?",
    )
    .bind(&e.name)
    .bind(kind_str(e.kind))
    .bind(points_json(&e.points))
    .bind(&e.color)
    .bind(i64::from(e.clamp_to_ground))
    .bind(e.updated_at_ms)
    .bind(&e.id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn delete(pool: &SqlitePool, id: &str) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("DELETE FROM map_elements WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// IDs der Elemente ohne Einsatz, zuletzt geändert vor `cutoff_ms`.
pub async fn unassigned_older_than(
    pool: &SqlitePool,
    cutoff_ms: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM map_elements WHERE mission_id IS NULL AND updated_at_ms < ?")
        .bind(cutoff_ms)
        .fetch_all(pool)
        .await
}

/// IDs der Elemente außerhalb eines Einsatzes (mit `None`: außerhalb „ohne Einsatz“), zuletzt
/// geänderte zuerst, höchstens `limit`.
pub async fn ids_outside(
    pool: &SqlitePool,
    mission: Option<i64>,
    limit: i64,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT id FROM map_elements WHERE mission_id IS NOT ? ORDER BY updated_at_ms DESC, id LIMIT ?",
    )
    .bind(mission)
    .bind(limit)
    .fetch_all(pool)
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::examples;

    #[tokio::test]
    async fn elements_roundtrip_with_height_and_origin() {
        let pool = crate::db::test_pool().await;
        let [area, pin] = <[MapElement; 2]>::try_from(examples::map_elements()).unwrap();
        // Absichtlich in umgekehrter Reihenfolge: Die Liste sortiert nach Anlagezeit.
        assert!(insert(&pool, &pin).await.unwrap());
        assert!(insert(&pool, &area).await.unwrap());
        assert_eq!(list(&pool).await.unwrap(), vec![area.clone(), pin.clone()]);
        assert_eq!(get(&pool, &pin.id).await.unwrap(), Some(pin.clone()));
        assert_eq!(get(&pool, "fehlt").await.unwrap(), None);
    }

    #[tokio::test]
    async fn insert_keeps_an_existing_element() {
        let pool = crate::db::test_pool().await;
        let pin = examples::map_elements().remove(1);
        assert!(insert(&pool, &pin).await.unwrap());
        let other = MapElement {
            name: "anders".into(),
            ..pin.clone()
        };
        assert!(!insert(&pool, &other).await.unwrap());
        assert_eq!(get(&pool, &pin.id).await.unwrap(), Some(pin));
    }

    #[tokio::test]
    async fn update_keeps_author_origin_and_creation_time() {
        let pool = crate::db::test_pool().await;
        let pin = examples::map_elements().remove(1);
        insert(&pool, &pin).await.unwrap();
        let changed = MapElement {
            name: "Fundstelle".into(),
            color: "#19BE6B".into(),
            author: "jemand anderes".into(),
            origin: MapElementOrigin::Web,
            created_at_ms: 0,
            updated_at_ms: pin.updated_at_ms + 1,
            ..pin.clone()
        };
        assert!(update(&pool, &changed).await.unwrap());
        let stored = get(&pool, &pin.id).await.unwrap().unwrap();
        assert_eq!(
            (stored.name.as_str(), stored.color.as_str()),
            ("Fundstelle", "#19BE6B")
        );
        assert_eq!(
            (stored.author, stored.origin, stored.created_at_ms),
            (pin.author, pin.origin, pin.created_at_ms)
        );
        assert!(
            !update(
                &pool,
                &MapElement {
                    id: "fehlt".into(),
                    ..changed
                }
            )
            .await
            .unwrap()
        );
    }

    #[tokio::test]
    async fn delete_reports_whether_something_was_there() {
        let pool = crate::db::test_pool().await;
        let pin = examples::map_elements().remove(1);
        insert(&pool, &pin).await.unwrap();
        assert!(delete(&pool, &pin.id).await.unwrap());
        assert!(!delete(&pool, &pin.id).await.unwrap());
    }
}

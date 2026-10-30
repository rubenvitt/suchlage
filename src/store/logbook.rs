//! Flugbucheinträge der Flugmappen (SUL-113): bestätigte Telemetrie-Flüge und Handeinträge, je
//! Verknüpfung, mit Revision und Löschmarke.

use sqlx::SqlitePool;

/// Ein gespeicherter Eintrag; bei einer Löschmarke (`deleted`) sind nur Revision und IDs gesetzt.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct StoredEntry {
    pub id: i64,
    pub link_id: i64,
    pub entry_id: String,
    pub revision: i64,
    pub deleted: bool,
    pub flight_id: Option<i64>,
    pub section_id: Option<i64>,
    pub started_at_ms: Option<i64>,
    pub landed_at_ms: Option<i64>,
    pub aircraft_label: Option<String>,
    pub pilot: Option<String>,
    pub landing_status: Option<String>,
    pub changed_at_ms: Option<i64>,
}

/// Was die Flugmappe meldet, mit dem Abschnitt in Suchlage.
#[derive(Debug, Clone, PartialEq)]
pub struct NewEntry {
    pub revision: i64,
    pub flight_id: Option<i64>,
    pub section_id: Option<i64>,
    pub started_at_ms: i64,
    pub landed_at_ms: Option<i64>,
    pub aircraft_label: String,
    pub pilot: Option<String>,
    pub landing_status: Option<String>,
    pub changed_at_ms: i64,
}

macro_rules! select_entries {
    ($tail:literal) => {
        concat!(
            "SELECT e.id, e.link_id, e.entry_id, e.revision, e.deleted, e.flight_id, e.section_id, \
             e.started_at_ms, e.landed_at_ms, e.aircraft_label, e.pilot, e.landing_status, \
             e.changed_at_ms FROM flugmappe_logbook_entries e ",
            $tail
        )
    };
}

pub async fn get(
    pool: &SqlitePool,
    link_id: i64,
    entry_id: &str,
) -> Result<Option<StoredEntry>, sqlx::Error> {
    sqlx::query_as(select_entries!("WHERE e.link_id = ? AND e.entry_id = ?"))
        .bind(link_id)
        .bind(entry_id)
        .fetch_optional(pool)
        .await
}

/// Wer den Flug im Flugbuch hat: (Verknüpfung, deren Fluggerät).
pub async fn holder(
    pool: &SqlitePool,
    flight_id: i64,
) -> Result<Option<(i64, String)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT l.id, l.aircraft_label FROM flugmappe_logbook_entries e \
         JOIN mission_flugmappe_links l ON l.id = e.link_id WHERE e.flight_id = ?",
    )
    .bind(flight_id)
    .fetch_optional(pool)
    .await
}

/// Schreibt den Eintrag (neu oder ersetzt, auch eine Löschmarke). Ein bestätigter Flug bekommt
/// den Abschnitt des Eintrags. Ein Flug, den schon ein anderer Eintrag hat, scheitert am Index (409).
pub async fn put(
    pool: &SqlitePool,
    link_id: i64,
    entry_id: &str,
    e: &NewEntry,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "INSERT INTO flugmappe_logbook_entries (link_id, entry_id, revision, deleted, flight_id, \
         section_id, started_at_ms, landed_at_ms, aircraft_label, pilot, landing_status, \
         changed_at_ms, received_at_ms) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12) \
         ON CONFLICT (link_id, entry_id) DO UPDATE SET revision = ?3, deleted = 0, flight_id = ?4, \
         section_id = ?5, started_at_ms = ?6, landed_at_ms = ?7, aircraft_label = ?8, pilot = ?9, \
         landing_status = ?10, changed_at_ms = ?11, received_at_ms = ?12",
    )
    .bind(link_id)
    .bind(entry_id)
    .bind(e.revision)
    .bind(e.flight_id)
    .bind(e.section_id)
    .bind(e.started_at_ms)
    .bind(e.landed_at_ms)
    .bind(&e.aircraft_label)
    .bind(&e.pilot)
    .bind(&e.landing_status)
    .bind(e.changed_at_ms)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;
    if let (Some(flight), Some(section)) = (e.flight_id, e.section_id) {
        sqlx::query("UPDATE flights SET section_id = ? WHERE id = ?")
            .bind(section)
            .bind(flight)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

/// Ersetzt den Eintrag durch eine Löschmarke mit dieser Revision (auch für einen unbekannten).
/// Die Bestätigung entfällt, der Telemetrie-Flug bleibt.
pub async fn tombstone(
    pool: &SqlitePool,
    link_id: i64,
    entry_id: &str,
    revision: i64,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO flugmappe_logbook_entries (link_id, entry_id, revision, deleted, received_at_ms) \
         VALUES (?1, ?2, ?3, 1, ?4) ON CONFLICT (link_id, entry_id) DO UPDATE SET revision = ?3, \
         deleted = 1, flight_id = NULL, section_id = NULL, started_at_ms = NULL, landed_at_ms = NULL, \
         aircraft_label = NULL, pilot = NULL, landing_status = NULL, changed_at_ms = NULL, \
         received_at_ms = ?4",
    )
    .bind(link_id)
    .bind(entry_id)
    .bind(revision)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

/// Ein lebender Eintrag eines Einsatzes mit dem Fluggerät seiner Flugmappe.
#[derive(Debug, Clone, PartialEq)]
pub struct MissionEntry {
    pub entry: StoredEntry,
    pub link_aircraft_label: String,
}

/// Alle lebenden Einträge der Flugmappen eines Einsatzes, nach Start.
pub async fn of_mission(
    pool: &SqlitePool,
    mission_id: i64,
) -> Result<Vec<MissionEntry>, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        #[sqlx(flatten)]
        entry: StoredEntry,
        link_aircraft_label: String,
    }
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT e.id, e.link_id, e.entry_id, e.revision, e.deleted, e.flight_id, e.section_id, \
         e.started_at_ms, e.landed_at_ms, e.aircraft_label, e.pilot, e.landing_status, \
         e.changed_at_ms, l.aircraft_label AS link_aircraft_label FROM flugmappe_logbook_entries e \
         JOIN mission_flugmappe_links l ON l.id = e.link_id \
         WHERE l.mission_id = ? AND e.deleted = 0 ORDER BY e.started_at_ms, e.id",
    )
    .bind(mission_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| MissionEntry {
            entry: r.entry,
            link_aircraft_label: r.link_aircraft_label,
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::FlightSummary;
    use crate::db::test_pool;
    use crate::error::AppError;
    use crate::store::{flights, missions};

    const FM_A: &str = "0f6b2a1e-5d43-4c8e-9b7a-2e1f3c4d5a6b";
    const FM_B: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

    fn start(fm: &str) -> missions::FlugmappeStart {
        missions::FlugmappeStart {
            name: "Vermisste Person".into(),
            location: None,
            created_by: "Testpilot".into(),
            flugmappe_mission_id: fm.into(),
            flugmappe_section_id: "s1".into(),
            section_label: "Abschnitt 1".into(),
            section_location: None,
            aircraft_label: "Matrice 350 RTK".into(),
        }
    }

    fn entry(revision: i64, flight_id: Option<i64>) -> NewEntry {
        NewEntry {
            revision,
            flight_id,
            section_id: None,
            started_at_ms: 1_000,
            landed_at_ms: Some(2_000),
            aircraft_label: "Matrice 200".into(),
            pilot: Some("Erika".into()),
            landing_status: None,
            changed_at_ms: 3_000,
        }
    }

    #[tokio::test]
    async fn a_flight_is_in_one_logbook_and_a_tombstone_frees_it() {
        let pool = test_pool().await;
        let m = missions::create_from_flugmappe(&pool, &start(FM_A), 1)
            .await
            .unwrap();
        let ids = missions::flugmappe_ids(&pool, m).await.unwrap();
        let a = ids.link(FM_A).unwrap().id;
        let b = sqlx::query_scalar::<_, i64>(
            "INSERT INTO mission_flugmappe_links (mission_id, flugmappe_mission_id, linked_by, \
             linked_at_ms, aircraft_label) VALUES (?, ?, 'Zweite', 2, 'Matrice 200') RETURNING id",
        )
        .bind(m)
        .bind(FM_B)
        .fetch_one(&pool)
        .await
        .unwrap();
        flights::insert(
            &pool,
            &FlightSummary {
                id: 1,
                aircraft_sn: "AC1".into(),
                gateway_sn: "RC1".into(),
                started_at_ms: 1_000,
                ended_at_ms: None,
                end_reason: None,
                max_elevation_m: None,
                distance_m: 0.0,
            },
        )
        .await
        .unwrap();

        put(&pool, a, "e1", &entry(1, Some(1)), 10).await.unwrap();
        assert_eq!(
            holder(&pool, 1).await.unwrap(),
            Some((a, "Matrice 350 RTK".into()))
        );
        let err: AppError = put(&pool, b, "e1", &entry(1, Some(1)), 11)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");

        tombstone(&pool, a, "e1", 2, 12).await.unwrap();
        let t = get(&pool, a, "e1").await.unwrap().unwrap();
        assert_eq!((t.deleted, t.revision, t.flight_id), (true, 2, None));
        assert_eq!(holder(&pool, 1).await.unwrap(), None);
        put(&pool, b, "e1", &entry(1, Some(1)), 13).await.unwrap();
        let live = of_mission(&pool, m).await.unwrap();
        assert_eq!(live.len(), 1, "Löschmarken zählen nicht");
        assert_eq!(live[0].link_aircraft_label, "Matrice 200");

        // Ein gelöschter Flug macht aus der Bestätigung einen Handeintrag.
        sqlx::query("DELETE FROM flights WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert_eq!(get(&pool, b, "e1").await.unwrap().unwrap().flight_id, None);
    }
}

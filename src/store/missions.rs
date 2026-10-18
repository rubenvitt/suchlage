//! Einsätze und ihre Abschnitte. Zuordnung von Flügen, Fotos und Kartenelementen.

use sqlx::{Row, SqlitePool};

use crate::contract::{Mission, MissionCreate, MissionSection, MissionState, SectionCreate};

fn state_str(s: MissionState) -> &'static str {
    match s {
        MissionState::Prepared => "prepared",
        MissionState::Running => "running",
        MissionState::Closed => "closed",
    }
}

async fn sections(pool: &SqlitePool, mission_id: i64) -> Result<Vec<MissionSection>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, label, location, created_at_ms FROM mission_sections WHERE mission_id = ? ORDER BY id",
    )
    .bind(mission_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| MissionSection {
            id: r.get("id"),
            label: r.get("label"),
            location: r.get("location"),
            created_at_ms: r.get("created_at_ms"),
        })
        .collect())
}

async fn mission(pool: &SqlitePool, r: &sqlx::sqlite::SqliteRow) -> Result<Mission, sqlx::Error> {
    let id: i64 = r.get("id");
    Ok(Mission {
        id,
        name: r.get("name"),
        location: r.get("location"),
        state: match r.get::<&str, _>("state") {
            "running" => MissionState::Running,
            "closed" => MissionState::Closed,
            _ => MissionState::Prepared,
        },
        created_by: r.get("created_by"),
        created_at_ms: r.get("created_at_ms"),
        started_at_ms: r.get("started_at_ms"),
        closed_at_ms: r.get("closed_at_ms"),
        sections: sections(pool, id).await?,
    })
}

/// `SELECT <Spalten> FROM missions <Rest>` als Literal (sqlx 0.9 nimmt nur solche an).
macro_rules! select_missions {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, location, state, created_by, created_at_ms, started_at_ms, closed_at_ms FROM missions ",
            $tail
        )
    };
}

pub async fn create(
    pool: &SqlitePool,
    input: &MissionCreate,
    created_by: &str,
    now_ms: i64,
) -> Result<Mission, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO missions (name, location, state, created_by, created_at_ms) VALUES (?, ?, 'prepared', ?, ?) RETURNING id",
    )
    .bind(&input.name)
    .bind(&input.location)
    .bind(created_by)
    .bind(now_ms)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO mission_sections (mission_id, label, location, created_at_ms) VALUES (?, 'Abschnitt 1', ?, ?)",
    )
    .bind(id)
    .bind(&input.location)
    .bind(now_ms)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(get(pool, id).await?.expect("Einsatz wurde eben angelegt"))
}

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<Mission>, sqlx::Error> {
    let r = sqlx::query(select_missions!("WHERE id = ?"))
        .bind(id)
        .fetch_optional(pool)
        .await?;
    match r {
        Some(r) => Ok(Some(mission(pool, &r).await?)),
        None => Ok(None),
    }
}

/// Laufend, dann vorbereitet nach Anlage, dann abgeschlossen neueste zuerst.
pub async fn list(pool: &SqlitePool) -> Result<Vec<Mission>, sqlx::Error> {
    let rows = sqlx::query(select_missions!(
        "ORDER BY CASE state WHEN 'running' THEN 0 WHEN 'prepared' THEN 1 ELSE 2 END, \
         CASE WHEN state = 'closed' THEN -id ELSE id END"
    ))
    .fetch_all(pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(mission(pool, r).await?);
    }
    Ok(out)
}

pub async fn running_id(pool: &SqlitePool) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM missions WHERE state = 'running'")
        .fetch_optional(pool)
        .await
}

/// Setzt den Zustand und die passende Zeit. Ein zweiter laufender Einsatz scheitert am Teilindex
/// (Unique-Fehler, im `AppError` ein Konflikt).
pub async fn set_state(
    pool: &SqlitePool,
    id: i64,
    state: MissionState,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE missions SET state = ?1, \
         started_at_ms = CASE WHEN ?1 = 'running' THEN ?2 ELSE started_at_ms END, \
         closed_at_ms = CASE WHEN ?1 = 'closed' THEN ?2 ELSE closed_at_ms END WHERE id = ?3",
    )
    .bind(state_str(state))
    .bind(now_ms)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_section(
    pool: &SqlitePool,
    mission_id: i64,
    input: &SectionCreate,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO mission_sections (mission_id, label, location, created_at_ms) VALUES (?, ?, ?, ?)",
    )
    .bind(mission_id)
    .bind(&input.label)
    .bind(&input.location)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn has_open_flight(pool: &SqlitePool, mission_id: i64) -> Result<bool, sqlx::Error> {
    let hit: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM flights WHERE mission_id = ? AND ended_at_ms IS NULL LIMIT 1",
    )
    .bind(mission_id)
    .fetch_optional(pool)
    .await?;
    Ok(hit.is_some())
}

/// Ordnet einen Flug ohne Einsatz dem letzten Abschnitt des Ziels zu, samt seinen Fotos.
/// `false`, wenn der Flug fehlt oder schon zugeordnet ist.
pub async fn assign_flight(
    pool: &SqlitePool,
    flight_id: i64,
    mission_id: i64,
) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let done = sqlx::query(
        "UPDATE flights SET mission_id = ?1, \
         section_id = (SELECT MAX(id) FROM mission_sections WHERE mission_id = ?1) \
         WHERE id = ?2 AND mission_id IS NULL",
    )
    .bind(mission_id)
    .bind(flight_id)
    .execute(&mut *tx)
    .await?;
    if done.rows_affected() == 0 {
        return Ok(false);
    }
    sqlx::query("UPDATE photos SET mission_id = ? WHERE flight_id = ?")
        .bind(mission_id)
        .bind(flight_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// `false`, wenn das Element fehlt oder schon zugeordnet ist.
pub async fn assign_element(
    pool: &SqlitePool,
    element_id: &str,
    mission_id: i64,
) -> Result<bool, sqlx::Error> {
    let done =
        sqlx::query("UPDATE map_elements SET mission_id = ? WHERE id = ? AND mission_id IS NULL")
            .bind(mission_id)
            .bind(element_id)
            .execute(pool)
            .await?;
    Ok(done.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{FlightSummary, examples};
    use crate::db::test_pool;
    use crate::error::AppError;
    use crate::store::photos::{NewPhoto, in_mission, of_flight, since_unassigned, upsert};
    use crate::store::{flights, map_elements};

    fn mc(name: &str) -> MissionCreate {
        MissionCreate {
            name: name.into(),
            location: Some("Wald".into()),
        }
    }

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

    fn photo(key: &str, flight_id: Option<i64>) -> NewPhoto {
        NewPhoto {
            object_key: key.into(),
            name: format!("{key}.JPG"),
            fingerprint: None,
            tiny_fingerprint: None,
            shot_key: format!("RC1/{key}"),
            lens: crate::contract::PhotoLens::Wide,
            gateway_sn: "RC1".into(),
            aircraft_sn: None,
            flight_id,
            taken_at_ms: Some(10_000),
            uploaded_at_ms: 20_000,
            lat: None,
            lon: None,
            absolute_altitude_m: None,
            relative_altitude_m: None,
            gimbal_yaw_deg: None,
            payload_model_key: None,
        }
    }

    #[tokio::test]
    async fn create_starts_prepared_with_first_section() {
        let pool = test_pool().await;
        let m = create(&pool, &mc("Vermisste Person"), "ruben", 5)
            .await
            .unwrap();
        assert_eq!(m.state, MissionState::Prepared);
        assert_eq!(m.sections.len(), 1);
        assert_eq!(m.sections[0].label, "Abschnitt 1");
        assert_eq!(m.sections[0].location.as_deref(), Some("Wald"));
        assert_eq!(get(&pool, m.id).await.unwrap(), Some(m));
    }

    #[tokio::test]
    async fn second_running_mission_is_a_conflict() {
        let pool = test_pool().await;
        let a = create(&pool, &mc("A"), "u", 1).await.unwrap();
        let b = create(&pool, &mc("B"), "u", 2).await.unwrap();
        set_state(&pool, a.id, MissionState::Running, 10)
            .await
            .unwrap();
        let err: AppError = set_state(&pool, b.id, MissionState::Running, 11)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        assert_eq!(running_id(&pool).await.unwrap(), Some(a.id));
        // Reihenfolge: laufend, vorbereitet, abgeschlossen.
        set_state(&pool, a.id, MissionState::Closed, 12)
            .await
            .unwrap();
        let c = create(&pool, &mc("C"), "u", 3).await.unwrap();
        set_state(&pool, c.id, MissionState::Running, 13)
            .await
            .unwrap();
        let order: Vec<i64> = list(&pool).await.unwrap().iter().map(|m| m.id).collect();
        assert_eq!(order, vec![c.id, b.id, a.id]);
        let a = get(&pool, a.id).await.unwrap().unwrap();
        assert_eq!((a.started_at_ms, a.closed_at_ms), (Some(10), Some(12)));
    }

    #[tokio::test]
    async fn flight_takes_running_mission_and_latest_section_at_insert() {
        let pool = test_pool().await;
        let m = create(&pool, &mc("A"), "u", 1).await.unwrap();
        set_state(&pool, m.id, MissionState::Running, 2)
            .await
            .unwrap();
        add_section(
            &pool,
            m.id,
            &SectionCreate {
                label: "Nord".into(),
                location: None,
            },
            3,
        )
        .await
        .unwrap();
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        let ids = |v: Vec<FlightSummary>| v.iter().map(|f| f.id).collect::<Vec<_>>();
        assert_eq!(
            ids(flights::list_in(&pool, Some(m.id), 10).await.unwrap()),
            vec![1]
        );
        let section: i64 = sqlx::query_scalar("SELECT section_id FROM flights WHERE id = 1")
            .fetch_one(&pool)
            .await
            .unwrap();
        let latest = get(&pool, m.id)
            .await
            .unwrap()
            .unwrap()
            .sections
            .pop()
            .unwrap();
        assert_eq!(section, latest.id);
        assert_eq!(latest.label, "Nord");

        set_state(&pool, m.id, MissionState::Closed, 4)
            .await
            .unwrap();
        flights::insert(&pool, &flight(2, "AC2")).await.unwrap();
        assert_eq!(
            ids(flights::list_in(&pool, None, 10).await.unwrap()),
            vec![2]
        );
        assert_eq!(
            ids(flights::list_in(&pool, Some(m.id), 10).await.unwrap()),
            vec![1]
        );
    }

    #[tokio::test]
    async fn photo_takes_mission_of_its_flight_else_running() {
        let pool = test_pool().await;
        let a = create(&pool, &mc("A"), "u", 1).await.unwrap();
        set_state(&pool, a.id, MissionState::Running, 2)
            .await
            .unwrap();
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        set_state(&pool, a.id, MissionState::Closed, 3)
            .await
            .unwrap();
        let b = create(&pool, &mc("B"), "u", 4).await.unwrap();
        set_state(&pool, b.id, MissionState::Running, 5)
            .await
            .unwrap();

        upsert(&pool, &photo("with-flight", Some(1))).await.unwrap();
        upsert(&pool, &photo("no-flight", None)).await.unwrap();
        let in_a = in_mission(&pool, a.id).await.unwrap();
        let in_b = in_mission(&pool, b.id).await.unwrap();
        assert_eq!(in_a.len(), 1);
        assert_eq!(in_a[0].name, "with-flight.JPG");
        assert_eq!(in_b.len(), 1);
        assert_eq!(in_b[0].name, "no-flight.JPG");
        // Wiederholter Callback bleibt im selben Einsatz.
        upsert(&pool, &photo("with-flight", Some(1))).await.unwrap();
        assert_eq!(in_mission(&pool, a.id).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn existing_rows_are_unassigned_after_migration() {
        let pool = test_pool().await;
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        let pin = examples::map_elements().remove(1);
        map_elements::insert(&pool, &pin).await.unwrap();
        upsert(&pool, &photo("p", None)).await.unwrap();
        create(&pool, &mc("A"), "u", 1).await.unwrap();
        assert_eq!(flights::list_in(&pool, None, 10).await.unwrap().len(), 1);
        assert_eq!(map_elements::list_in(&pool, None).await.unwrap(), vec![pin]);
        assert_eq!(since_unassigned(&pool, 0).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn assign_flight_moves_its_photos_and_only_from_unassigned() {
        let pool = test_pool().await;
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        upsert(&pool, &photo("p", Some(1))).await.unwrap();
        let m = create(&pool, &mc("A"), "u", 1).await.unwrap();
        assert!(!has_open_flight(&pool, m.id).await.unwrap());
        assert!(assign_flight(&pool, 1, m.id).await.unwrap());
        assert!(!assign_flight(&pool, 1, m.id).await.unwrap());
        assert_eq!(
            flights::list_in(&pool, Some(m.id), 10).await.unwrap().len(),
            1
        );
        assert_eq!(of_flight(&pool, 1).await.unwrap().len(), 1);
        assert_eq!(in_mission(&pool, m.id).await.unwrap().len(), 1);
        assert!(since_unassigned(&pool, 0).await.unwrap().is_empty());
        assert!(has_open_flight(&pool, m.id).await.unwrap());

        let pin = examples::map_elements().remove(1);
        map_elements::insert(&pool, &pin).await.unwrap();
        assert!(assign_element(&pool, &pin.id, m.id).await.unwrap());
        assert!(!assign_element(&pool, &pin.id, m.id).await.unwrap());
    }
}

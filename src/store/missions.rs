//! Einsätze und ihre Abschnitte. Zuordnung von Flügen, Fotos und Kartenelementen.

use sqlx::{Row, SqlitePool};

use crate::contract::{
    Mission, MissionCreate, MissionSection, MissionState, MissionUpdate, SectionCreate,
};

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

/// Schließt einen offenen Einsatz ohne offenen Flug. Prüfen und Schreiben in einer Anweisung,
/// damit kein Flugstart dazwischenfällt. `false`, wenn nichts geändert wurde.
pub async fn close(pool: &SqlitePool, id: i64, now_ms: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE missions SET state = 'closed', closed_at_ms = ?1 \
         WHERE id = ?2 AND state IN ('prepared', 'running') AND NOT EXISTS \
         (SELECT 1 FROM flights WHERE mission_id = ?2 AND ended_at_ms IS NULL)",
    )
    .bind(now_ms)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() > 0)
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

/// Name und Ort ändern. `false`, wenn es den Einsatz nicht gibt.
pub async fn update(pool: &SqlitePool, id: i64, u: &MissionUpdate) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("UPDATE missions SET name = ?, location = ? WHERE id = ?")
        .bind(&u.name)
        .bind(&u.location)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Anzahl Flüge, Fotodateien (Zeilen, nicht Auslösungen) und Kartenelemente.
pub async fn counts(pool: &SqlitePool, id: i64) -> Result<(u32, u32, u32), sqlx::Error> {
    let (f, p, e): (u32, u32, u32) = sqlx::query_as(
        "SELECT (SELECT COUNT(*) FROM flights WHERE mission_id = ?1), \
         (SELECT COUNT(*) FROM photos WHERE mission_id = ?1), \
         (SELECT COUNT(*) FROM map_elements WHERE mission_id = ?1)",
    )
    .bind(id)
    .fetch_one(pool)
    .await?;
    Ok((f, p, e))
}

/// Löscht den Einsatz mit allem, was an ihm hängt, in einer Transaktion; von den Fotos genau
/// `photo_ids` (deren Dateien schon weg sind). Später angekommene Fotos bleiben ohne Einsatz und
/// ohne Flug. Liefert die IDs der gelöschten Kartenelemente, `None`, wenn der Einsatz fehlt.
pub async fn delete_rows(
    pool: &SqlitePool,
    id: i64,
    photo_ids: &[i64],
) -> Result<Option<Vec<String>>, sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query("DELETE FROM photos WHERE id IN (SELECT value FROM json_each(?))")
        .bind(crate::store::photos::ids_json(photo_ids))
        .execute(&mut *tx)
        .await?;
    // mission_id hat keine Kaskade; flight_id wird beim Löschen der Flüge NULL.
    for sql in [
        "UPDATE photos SET mission_id = NULL WHERE mission_id = ?",
        "DELETE FROM flights WHERE mission_id = ?",
    ] {
        sqlx::query(sql).bind(id).execute(&mut *tx).await?;
    }
    let elements: Vec<String> =
        sqlx::query_scalar("DELETE FROM map_elements WHERE mission_id = ? RETURNING id")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    sqlx::query("DELETE FROM mission_sections WHERE mission_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let done = sqlx::query("DELETE FROM missions WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok((done.rows_affected() == 1).then_some(elements))
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
    async fn retried_photo_without_flight_stays_in_its_mission() {
        let pool = test_pool().await;
        let a = create(&pool, &mc("A"), "u", 1).await.unwrap();
        set_state(&pool, a.id, MissionState::Running, 2)
            .await
            .unwrap();
        upsert(&pool, &photo("no-flight", None)).await.unwrap();
        set_state(&pool, a.id, MissionState::Closed, 3)
            .await
            .unwrap();
        let b = create(&pool, &mc("B"), "u", 4).await.unwrap();
        set_state(&pool, b.id, MissionState::Running, 5)
            .await
            .unwrap();
        upsert(&pool, &photo("no-flight", None)).await.unwrap();
        assert_eq!(in_mission(&pool, a.id).await.unwrap().len(), 1);
        assert!(in_mission(&pool, b.id).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn closing_waits_for_the_open_flight_to_end() {
        let pool = test_pool().await;
        let m = create(&pool, &mc("A"), "u", 1).await.unwrap();
        set_state(&pool, m.id, MissionState::Running, 2)
            .await
            .unwrap();
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        assert!(!close(&pool, m.id, 3).await.unwrap());
        assert_eq!(running_id(&pool).await.unwrap(), Some(m.id));
        sqlx::query("UPDATE flights SET ended_at_ms = 4, end_reason = 'landed' WHERE id = 1")
            .execute(&pool)
            .await
            .unwrap();
        assert!(close(&pool, m.id, 5).await.unwrap());
        assert_eq!(running_id(&pool).await.unwrap(), None);
    }

    #[tokio::test]
    async fn photo_of_an_unassigned_flight_stays_unassigned() {
        let pool = test_pool().await;
        flights::insert(&pool, &flight(1, "AC1")).await.unwrap();
        let b = create(&pool, &mc("B"), "u", 1).await.unwrap();
        set_state(&pool, b.id, MissionState::Running, 2)
            .await
            .unwrap();
        upsert(&pool, &photo("p", Some(1))).await.unwrap();
        assert_eq!(since_unassigned(&pool, 0).await.unwrap().len(), 1);
        assert!(in_mission(&pool, b.id).await.unwrap().is_empty());
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
        assert!(assign_flight(&pool, 1, m.id).await.unwrap());
        assert!(!assign_flight(&pool, 1, m.id).await.unwrap());
        assert_eq!(
            flights::list_in(&pool, Some(m.id), 10).await.unwrap().len(),
            1
        );
        assert_eq!(of_flight(&pool, 1).await.unwrap().len(), 1);
        assert_eq!(in_mission(&pool, m.id).await.unwrap().len(), 1);
        assert!(since_unassigned(&pool, 0).await.unwrap().is_empty());
        // Der zugeordnete Flug ist offen: Abschließen ändert nichts.
        assert!(!close(&pool, m.id, 9).await.unwrap());

        let pin = examples::map_elements().remove(1);
        map_elements::insert(&pool, &pin).await.unwrap();
        assert!(assign_element(&pool, &pin.id, m.id).await.unwrap());
        assert!(!assign_element(&pool, &pin.id, m.id).await.unwrap());
    }

    /// Einsatz mit Flug `fid`, Spur, Foto am Flug, Foto ohne Flug, Element und zweitem Abschnitt.
    async fn filled(pool: &SqlitePool, name: &str, fid: i64) -> i64 {
        let m = create(pool, &mc(name), "u", 1).await.unwrap();
        set_state(pool, m.id, MissionState::Running, 2)
            .await
            .unwrap();
        flights::insert(pool, &flight(fid, &format!("AC{fid}")))
            .await
            .unwrap();
        let pt = crate::contract::TrackPoint {
            t_ms: 1,
            lat: 52.0,
            lon: 10.0,
            elevation_m: None,
        };
        flights::append_points(pool, fid, &[(0, pt)]).await.unwrap();
        upsert(pool, &photo(&format!("{name}-f"), Some(fid)))
            .await
            .unwrap();
        upsert(pool, &photo(&format!("{name}-n"), None))
            .await
            .unwrap();
        let mut e = examples::map_elements().remove(1);
        e.id = format!("el-{name}");
        e.mission_id = Some(m.id);
        map_elements::insert(pool, &e).await.unwrap();
        add_section(
            pool,
            m.id,
            &SectionCreate {
                label: "Nord".into(),
                location: None,
            },
            3,
        )
        .await
        .unwrap();
        set_state(pool, m.id, MissionState::Closed, 4)
            .await
            .unwrap();
        m.id
    }

    async fn rows(pool: &SqlitePool, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(pool)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn counts_match_contents() {
        let pool = test_pool().await;
        let a = filled(&pool, "A", 1).await;
        assert_eq!(counts(&pool, a).await.unwrap(), (1, 2, 1));
        assert_eq!(counts(&pool, 999).await.unwrap(), (0, 0, 0));
    }

    #[tokio::test]
    async fn delete_rows_removes_everything_of_the_mission_and_nothing_else() {
        let pool = test_pool().await;
        let a = filled(&pool, "A", 1).await;
        let b = filled(&pool, "B", 2).await;
        let ids = photo_ids(&pool, a).await;
        assert_eq!(
            delete_rows(&pool, a, &ids).await.unwrap(),
            Some(vec!["el-A".to_string()])
        );
        assert_eq!(delete_rows(&pool, a, &ids).await.unwrap(), None);
        assert_eq!(get(&pool, a).await.unwrap(), None);
        assert_eq!(counts(&pool, b).await.unwrap(), (1, 2, 1));
        assert_eq!(get(&pool, b).await.unwrap().unwrap().sections.len(), 2);
        assert_eq!(flights::segments(&pool, 2).await.unwrap().len(), 1);
        for (t, n) in [
            ("missions", 1),
            ("flights", 1),
            ("photos", 2),
            ("map_elements", 1),
            ("mission_sections", 2),
            ("track_points", 1),
        ] {
            assert_eq!(rows(&pool, t).await, n, "{t}");
        }
    }

    async fn photo_ids(pool: &SqlitePool, mission: i64) -> Vec<i64> {
        crate::store::photos::of_mission_all(pool, mission)
            .await
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect()
    }

    #[tokio::test]
    async fn late_photos_stay_without_mission_or_flight() {
        let pool = test_pool().await;
        let a = filled(&pool, "A", 1).await;
        let ids = photo_ids(&pool, a).await;
        // Eine Linse kommt nach dem Sammeln an: Ihre Zeile bleibt, ihre Datei ist nicht gelöscht.
        let late = upsert(&pool, &photo("A-spaet", Some(1))).await.unwrap();
        assert!(delete_rows(&pool, a, &ids).await.unwrap().is_some());
        let p = crate::store::photos::get(&pool, late)
            .await
            .unwrap()
            .unwrap();
        assert_eq!((p.mission_id, p.flight_id), (None, None));
        assert_eq!(rows(&pool, "photos").await, 1);
        assert_eq!(rows(&pool, "flights").await, 0);
    }

    #[tokio::test]
    async fn flight_delete_takes_its_photos_only() {
        let pool = test_pool().await;
        let a = filled(&pool, "A", 1).await;
        assert_eq!(flights::of_mission(&pool, a).await.unwrap().len(), 1);
        let of_flight: Vec<i64> = crate::store::photos::of_flight(&pool, 1)
            .await
            .unwrap()
            .iter()
            .map(|p| p.id)
            .collect();
        let late = upsert(&pool, &photo("A-spaet", Some(1))).await.unwrap();
        assert!(flights::delete_rows(&pool, 1, &of_flight).await.unwrap());
        assert!(!flights::delete_rows(&pool, 1, &of_flight).await.unwrap());
        let late = crate::store::photos::get(&pool, late)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(late.flight_id, None, "später angekommen: bleibt, ohne Flug");
        let left = crate::store::photos::of_mission_all(&pool, a)
            .await
            .unwrap();
        assert_eq!(left.len(), 2);
        assert!(left.iter().any(|p| p.name == "A-n.JPG"));
        assert_eq!(rows(&pool, "track_points").await, 0);
        let n = left.iter().find(|p| p.name == "A-n.JPG").unwrap().id;
        assert_eq!(
            crate::store::photos::delete_ids(&pool, &[n]).await.unwrap(),
            1
        );
        assert_eq!(rows(&pool, "photos").await, 1);
    }

    #[tokio::test]
    async fn unassigned_older_than_ignores_assigned_and_fresh() {
        let pool = test_pool().await;
        let mut old = examples::map_elements().remove(1);
        old.id = "old".into();
        old.updated_at_ms = 100;
        let mut fresh = old.clone();
        fresh.id = "fresh".into();
        fresh.updated_at_ms = 900;
        let m = create(&pool, &mc("A"), "u", 1).await.unwrap();
        let mut assigned = old.clone();
        assigned.id = "assigned".into();
        assigned.mission_id = Some(m.id);
        for e in [&old, &fresh, &assigned] {
            map_elements::insert(&pool, e).await.unwrap();
        }
        assert_eq!(
            map_elements::unassigned_older_than(&pool, 500)
                .await
                .unwrap(),
            vec!["old"]
        );
    }

    #[tokio::test]
    async fn update_changes_name_and_location() {
        let pool = test_pool().await;
        let m = create(&pool, &mc("A"), "u", 1).await.unwrap();
        let u = MissionUpdate {
            name: "Neu".into(),
            location: None,
        };
        assert!(update(&pool, m.id, &u).await.unwrap());
        let g = get(&pool, m.id).await.unwrap().unwrap();
        assert_eq!((g.name.as_str(), g.location), ("Neu", None));
        assert!(!update(&pool, 999, &u).await.unwrap());
    }
}

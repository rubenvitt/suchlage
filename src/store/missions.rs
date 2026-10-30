//! Einsätze und ihre Abschnitte. Zuordnung von Flügen, Fotos und Kartenelementen, Verknüpfung
//! mit der Flugmappe.

use sqlx::{Row, SqlitePool};

use crate::contract::{
    FlugmappeLink, Mission, MissionCreate, MissionOrigin, MissionSection, MissionState,
    MissionUpdate, SectionCreate,
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
        "SELECT id, label, location, created_at_ms, map_photo_at_ms FROM mission_sections \
         WHERE mission_id = ? ORDER BY id",
    )
    .bind(mission_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| {
            let map_photo_at_ms: Option<i64> = r.get("map_photo_at_ms");
            MissionSection {
                id: r.get("id"),
                label: r.get("label"),
                location: r.get("location"),
                created_at_ms: r.get("created_at_ms"),
                has_map_photo: map_photo_at_ms.is_some(),
                map_photo_at_ms,
            }
        })
        .collect())
}

/// Verknüpfungen mit Flugmappen, älteste zuerst, auch gelöste.
pub async fn links(pool: &SqlitePool, mission_id: i64) -> Result<Vec<LinkRow>, sqlx::Error> {
    sqlx::query_as(
        "SELECT id, flugmappe_mission_id, aircraft_label, linked_by, linked_at_ms, done_at_ms, \
         released_at_ms FROM mission_flugmappe_links WHERE mission_id = ? ORDER BY id",
    )
    .bind(mission_id)
    .fetch_all(pool)
    .await
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
        origin: match r.get::<&str, _>("origin") {
            "flugmappe" => MissionOrigin::Flugmappe,
            _ => MissionOrigin::Web,
        },
        flugmappe_links: links(pool, id)
            .await?
            .into_iter()
            .map(LinkRow::view)
            .collect(),
    })
}

/// `SELECT <Spalten> FROM missions <Rest>` als Literal (sqlx 0.9 nimmt nur solche an).
macro_rules! select_missions {
    ($tail:literal) => {
        concat!(
            "SELECT id, name, location, state, created_by, created_at_ms, started_at_ms, closed_at_ms, \
             origin FROM missions ",
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
pub async fn set_state<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
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
    .execute(db)
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

/// Eine Verknüpfung mit Flugmappe, samt UUID (die nur die Schnittstelle der Flugmappe sieht).
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct LinkRow {
    pub id: i64,
    pub flugmappe_mission_id: String,
    pub aircraft_label: String,
    pub linked_by: String,
    pub linked_at_ms: i64,
    pub done_at_ms: Option<i64>,
    pub released_at_ms: Option<i64>,
}

impl LinkRow {
    /// Sicht des Browsers: ohne UUID.
    pub fn view(self) -> FlugmappeLink {
        FlugmappeLink {
            id: self.id,
            aircraft_label: self.aircraft_label,
            linked_by: self.linked_by,
            linked_at_ms: self.linked_at_ms,
            done_at_ms: self.done_at_ms,
            released_at_ms: self.released_at_ms,
        }
    }
}

/// Ein Abschnitt und wem er gehört.
#[derive(Debug, Clone, PartialEq, sqlx::FromRow)]
pub struct SectionOwner {
    pub id: i64,
    /// Verknüpfung, die ihn angelegt oder beim Verknüpfen übernommen hat.
    pub link_id: Option<i64>,
    /// Abschnitts-ID der Flugmappe, eindeutig innerhalb ihrer Verknüpfung.
    pub flugmappe_section_id: Option<String>,
}

/// Was nur die Schnittstelle der Flugmappe braucht und der Browser nie sieht.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FlugmappeIds {
    pub links: Vec<LinkRow>,
    pub map_imported_at_ms: Option<i64>,
    /// Alle Abschnitte in Anlegereihenfolge.
    pub sections: Vec<SectionOwner>,
}

impl FlugmappeIds {
    /// Die Verknüpfung dieser Flugmappe mit dem Einsatz, auch gelöst.
    pub fn link(&self, flugmappe_mission_id: &str) -> Option<&LinkRow> {
        self.links
            .iter()
            .find(|l| l.flugmappe_mission_id == flugmappe_mission_id)
    }

    /// Abschnitt in Suchlage zur Abschnitts-ID dieser Verknüpfung.
    pub fn section(&self, link_id: i64, flugmappe_section_id: &str) -> Option<i64> {
        self.sections
            .iter()
            .find(|s| {
                s.link_id == Some(link_id)
                    && s.flugmappe_section_id.as_deref() == Some(flugmappe_section_id)
            })
            .map(|s| s.id)
    }

    /// Abschnitts-ID der Flugmappe, wenn der Abschnitt dieser Verknüpfung gehört.
    pub fn of_section(&self, link_id: i64, section_id: i64) -> Option<&str> {
        self.sections
            .iter()
            .find(|s| s.id == section_id && s.link_id == Some(link_id))
            .and_then(|s| s.flugmappe_section_id.as_deref())
    }

    /// Der aktive Abschnitt der Flugmappe: ihr zuletzt angelegter.
    pub fn active_section(&self, link_id: i64) -> Option<i64> {
        self.sections
            .iter()
            .rev()
            .find(|s| s.link_id == Some(link_id))
            .map(|s| s.id)
    }
}

/// Ein Einsatz, den die Flugmappe anlegt: läuft sofort, erster Abschnitt mit ihrer ID.
#[derive(Debug, Clone, PartialEq)]
pub struct FlugmappeStart {
    pub name: String,
    pub location: Option<String>,
    pub created_by: String,
    pub flugmappe_mission_id: String,
    pub flugmappe_section_id: String,
    pub section_label: String,
    pub section_location: Option<String>,
    pub aircraft_label: String,
}

pub async fn flugmappe_ids(pool: &SqlitePool, id: i64) -> Result<FlugmappeIds, sqlx::Error> {
    let imported: Option<Option<i64>> =
        sqlx::query_scalar("SELECT flugmappe_map_imported_at_ms FROM missions WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    let Some(map_imported_at_ms) = imported else {
        return Ok(FlugmappeIds::default());
    };
    let sections = sqlx::query_as(
        "SELECT id, link_id, flugmappe_section_id FROM mission_sections \
         WHERE mission_id = ? ORDER BY id",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(FlugmappeIds {
        links: links(pool, id).await?,
        map_imported_at_ms,
        sections,
    })
}

/// Der Einsatz mit der nicht gelösten Verknüpfung dieser Flugmappe, egal in welchem Zustand.
pub async fn find_by_flugmappe(
    pool: &SqlitePool,
    flugmappe_mission_id: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar(
        "SELECT mission_id FROM mission_flugmappe_links \
         WHERE flugmappe_mission_id = ? AND released_at_ms IS NULL",
    )
    .bind(flugmappe_mission_id)
    .fetch_optional(pool)
    .await
}

async fn insert_link<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
    mission_id: i64,
    l: &NewLink<'_>,
    now_ms: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO mission_flugmappe_links \
         (mission_id, flugmappe_mission_id, linked_by, linked_at_ms, aircraft_label) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(mission_id)
    .bind(l.flugmappe_mission_id)
    .bind(l.linked_by)
    .bind(now_ms)
    .bind(l.aircraft_label)
    .fetch_one(db)
    .await
}

async fn insert_section<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
    mission_id: i64,
    link_id: i64,
    flugmappe_section_id: &str,
    input: &SectionCreate,
    now_ms: i64,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO mission_sections \
         (mission_id, label, location, created_at_ms, flugmappe_section_id, link_id) \
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(mission_id)
    .bind(&input.label)
    .bind(&input.location)
    .bind(now_ms)
    .bind(flugmappe_section_id)
    .bind(link_id)
    .fetch_one(db)
    .await
}

/// Legt einen laufenden, verknüpften Einsatz mit seinem ersten Abschnitt an. Läuft schon einer
/// oder ist die Flugmappe schon verknüpft, scheitert es an einem Teilindex (409).
pub async fn create_from_flugmappe(
    pool: &SqlitePool,
    start: &FlugmappeStart,
    now_ms: i64,
) -> Result<i64, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO missions (name, location, state, created_by, created_at_ms, started_at_ms, \
         origin) VALUES (?1, ?2, 'running', ?3, ?4, ?4, 'flugmappe') RETURNING id",
    )
    .bind(&start.name)
    .bind(&start.location)
    .bind(&start.created_by)
    .bind(now_ms)
    .fetch_one(&mut *tx)
    .await?;
    let link = NewLink {
        flugmappe_mission_id: &start.flugmappe_mission_id,
        linked_by: &start.created_by,
        aircraft_label: &start.aircraft_label,
    };
    let link_id = insert_link(&mut *tx, id, &link, now_ms).await?;
    let section = SectionCreate {
        label: start.section_label.clone(),
        location: start.section_location.clone(),
    };
    insert_section(
        &mut *tx,
        id,
        link_id,
        &start.flugmappe_section_id,
        &section,
        now_ms,
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Eine neue Verknüpfung.
#[derive(Debug, Clone, Copy)]
pub struct NewLink<'a> {
    pub flugmappe_mission_id: &'a str,
    pub linked_by: &'a str,
    pub aircraft_label: &'a str,
}

/// Was mit der Verknüpfung geschieht.
#[derive(Debug, Clone, Copy)]
pub enum LinkOp<'a> {
    /// Besteht schon (ID der Verknüpfung).
    Keep(i64),
    Insert(NewLink<'a>),
    /// Gelöste Verknüpfung wieder aufleben lassen, mit dem jetzigen Fluggerät.
    Revive {
        id: i64,
        aircraft_label: &'a str,
    },
}

/// Was mit dem Abschnitt der Flugmappe geschieht.
#[derive(Debug, Clone, Copy)]
pub enum SectionOp<'a> {
    /// Gibt es schon.
    Keep,
    /// Ein Abschnitt ohne Flugmappe bekommt ihre ID, Bezeichnung und (falls genannt) Ort.
    Bind {
        section_id: i64,
        flugmappe_section_id: &'a str,
        input: &'a SectionCreate,
    },
    /// Neuer Abschnitt der Flugmappe (der aktive gehört einer anderen).
    Create {
        flugmappe_section_id: &'a str,
        input: &'a SectionCreate,
    },
}

/// Was beim Verknüpfen geschrieben wird.
#[derive(Debug, Clone, Copy)]
pub struct LinkWrite<'a> {
    /// vorbereitet → laufend
    pub start: bool,
    pub link: LinkOp<'a>,
    pub section: SectionOp<'a>,
}

/// Verknüpfen in einer Transaktion: starten, Verknüpfung schreiben, Abschnitt binden oder anlegen
/// – ganz oder gar nicht. So bleibt kein laufender Einsatz ohne Verknüpfung zurück. Ein zweiter
/// laufender Einsatz oder eine zweite nicht gelöste Verknüpfung derselben Flugmappe scheitert am
/// Teilindex (409).
pub async fn link_flugmappe(
    pool: &SqlitePool,
    id: i64,
    w: &LinkWrite<'_>,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    if w.start {
        set_state(&mut *tx, id, MissionState::Running, now_ms).await?;
    }
    let link_id = match w.link {
        LinkOp::Keep(link_id) => link_id,
        LinkOp::Insert(l) => insert_link(&mut *tx, id, &l, now_ms).await?,
        LinkOp::Revive {
            id: link_id,
            aircraft_label,
        } => {
            sqlx::query(
                "UPDATE mission_flugmappe_links SET released_at_ms = NULL, done_at_ms = NULL, \
                 aircraft_label = ? WHERE id = ?",
            )
            .bind(aircraft_label)
            .bind(link_id)
            .execute(&mut *tx)
            .await?;
            link_id
        }
    };
    match w.section {
        SectionOp::Keep => {}
        SectionOp::Bind {
            section_id,
            flugmappe_section_id,
            input,
        } => {
            sqlx::query(
                "UPDATE mission_sections SET flugmappe_section_id = ?, link_id = ?, label = ?, \
                 location = COALESCE(?, location) WHERE id = ?",
            )
            .bind(flugmappe_section_id)
            .bind(link_id)
            .bind(&input.label)
            .bind(&input.location)
            .bind(section_id)
            .execute(&mut *tx)
            .await?;
        }
        SectionOp::Create {
            flugmappe_section_id,
            input,
        } => {
            insert_section(&mut *tx, id, link_id, flugmappe_section_id, input, now_ms).await?;
        }
    }
    tx.commit().await
}

/// Neuer (und damit aktiver) Abschnitt aus der Flugmappe; liefert seine ID.
pub async fn add_section_from_flugmappe(
    pool: &SqlitePool,
    mission_id: i64,
    link_id: i64,
    flugmappe_section_id: &str,
    input: &SectionCreate,
    now_ms: i64,
) -> Result<i64, sqlx::Error> {
    insert_section(
        pool,
        mission_id,
        link_id,
        flugmappe_section_id,
        input,
        now_ms,
    )
    .await
}

/// Die Flugmappe meldet ihren Teil als fertig; wiederholt bleibt die erste Zeit.
pub async fn mark_link_done(
    pool: &SqlitePool,
    link_id: i64,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "UPDATE mission_flugmappe_links SET done_at_ms = COALESCE(done_at_ms, ?) WHERE id = ?",
    )
    .bind(now_ms)
    .bind(link_id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Löst eine Verknüpfung des Einsatzes (in Suchlage). `false`, wenn sie fehlt oder schon gelöst ist.
pub async fn release_link(
    pool: &SqlitePool,
    mission_id: i64,
    link_id: i64,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE mission_flugmappe_links SET released_at_ms = ? \
         WHERE id = ? AND mission_id = ? AND released_at_ms IS NULL",
    )
    .bind(now_ms)
    .bind(link_id)
    .bind(mission_id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// Bezeichnung und Ort eines Abschnitts aus der Flugmappe.
pub async fn update_section(
    pool: &SqlitePool,
    section_id: i64,
    input: &SectionCreate,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mission_sections SET label = ?, location = ? WHERE id = ?")
        .bind(&input.label)
        .bind(&input.location)
        .bind(section_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Setzt den Ort eines Abschnitts, falls er noch keinen hat.
pub async fn set_section_location_if_empty<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
    section_id: i64,
    location: &str,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE mission_sections SET location = ? WHERE id = ? AND (location IS NULL OR location = '')",
    )
    .bind(location)
    .bind(section_id)
    .execute(db)
    .await?;
    Ok(done.rows_affected() == 1)
}

pub async fn set_map_photo(
    pool: &SqlitePool,
    section_id: i64,
    at_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE mission_sections SET map_photo_at_ms = ? WHERE id = ?")
        .bind(at_ms)
        .bind(section_id)
        .execute(pool)
        .await?;
    Ok(())
}

/// Merkt die Übernahme der Kartenelemente. `false`, wenn sie schon erledigt war.
pub async fn mark_map_imported<'e>(
    db: impl sqlx::SqliteExecutor<'e>,
    id: i64,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE missions SET flugmappe_map_imported_at_ms = ? \
         WHERE id = ? AND flugmappe_map_imported_at_ms IS NULL",
    )
    .bind(now_ms)
    .bind(id)
    .execute(db)
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

    fn fm_start(fm: &str) -> FlugmappeStart {
        FlugmappeStart {
            name: "Vermisste Person".into(),
            location: Some("Wald".into()),
            created_by: "Testpilot".into(),
            flugmappe_mission_id: fm.into(),
            flugmappe_section_id: format!("{fm}-s1"),
            section_label: "Abschnitt 1".into(),
            section_location: None,
            aircraft_label: "Matrice 350 RTK".into(),
        }
    }

    async fn migrated_until(last: usize, setup: &str) -> SqlitePool {
        let options = sqlx::sqlite::SqliteConnectOptions::new()
            .in_memory(true)
            .foreign_keys(true);
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await
            .unwrap();
        let all = [
            include_str!("../../migrations/0001_init.sql"),
            include_str!("../../migrations/0002_map_elements.sql"),
            include_str!("../../migrations/0003_photos.sql"),
            include_str!("../../migrations/0004_missions.sql"),
            include_str!("../../migrations/0005_flugmappe.sql"),
            include_str!("../../migrations/0006_account_block.sql"),
            include_str!("../../migrations/0007_flugmappe_links.sql"),
        ];
        for sql in &all[..last] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(*sql))
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::raw_sql(sqlx::AssertSqlSafe(setup))
            .execute(&pool)
            .await
            .unwrap();
        for sql in &all[last..] {
            sqlx::raw_sql(sqlx::AssertSqlSafe(*sql))
                .execute(&pool)
                .await
                .unwrap();
        }
        pool
    }

    #[tokio::test]
    async fn migration_keeps_existing_missions_from_the_browser_and_unlinked() {
        let pool = migrated_until(
            4,
            "INSERT INTO missions (id, name, state, created_by, created_at_ms) VALUES (1, 'Alt', 'running', 'u', 1); \
             INSERT INTO mission_sections (mission_id, label, created_at_ms) VALUES (1, 'Abschnitt 1', 1);",
        )
        .await;
        let m = get(&pool, 1).await.unwrap().unwrap();
        assert_eq!((m.origin, m.flugmappe_links), (MissionOrigin::Web, vec![]));
        assert_eq!(m.state, MissionState::Running);
        assert!(!m.sections[0].has_map_photo);
        let ids = flugmappe_ids(&pool, 1).await.unwrap();
        assert!(ids.links.is_empty());
        assert_eq!(ids.sections[0].link_id, None);
    }

    #[tokio::test]
    async fn migration_moves_the_existing_link_into_its_own_table() {
        let fm = "0f6b2a1e-5d43-4c8e-9b7a-2e1f3c4d5a6b";
        let pool = migrated_until(
            6,
            &format!(
                "INSERT INTO missions (id, name, state, created_by, created_at_ms, origin, \
                 flugmappe_mission_id, flugmappe_linked_by, flugmappe_linked_at_ms) \
                 VALUES (1, 'Laufend', 'running', 'u', 1, 'flugmappe', '{fm}', 'Testpilot', 1), \
                        (2, 'Alt', 'closed', 'u', 1, 'web', 'aaaaaaaa-5d43-4c8e-9b7a-2e1f3c4d5a6b', 'Zweite', 2); \
                 UPDATE missions SET closed_at_ms = 9 WHERE id = 2; \
                 INSERT INTO mission_sections (mission_id, label, created_at_ms, flugmappe_section_id) \
                 VALUES (1, 'Abschnitt 1', 1, 's1'), (1, 'Nord', 2, NULL), (2, 'Abschnitt 1', 1, 's1');"
            ),
        )
        .await;
        let m = get(&pool, 1).await.unwrap().unwrap();
        assert_eq!(
            m.flugmappe_links
                .iter()
                .map(|l| (
                    l.linked_by.as_str(),
                    l.linked_at_ms,
                    l.aircraft_label.as_str(),
                    l.done_at_ms
                ))
                .collect::<Vec<_>>(),
            vec![("Testpilot", 1, "Fluggerät unbekannt", None)]
        );
        let ids = flugmappe_ids(&pool, 1).await.unwrap();
        let link = ids.link(fm).unwrap().id;
        assert_eq!(ids.section(link, "s1"), Some(m.sections[0].id));
        assert_eq!(ids.sections[1].link_id, None, "in Suchlage angelegt");
        assert_eq!(find_by_flugmappe(&pool, fm).await.unwrap(), Some(1));
        let closed = get(&pool, 2).await.unwrap().unwrap();
        assert_eq!(
            closed.flugmappe_links[0].done_at_ms,
            Some(9),
            "abgeschlossen heißt fertig"
        );
        // Beide Abschnitte heißen „s1“, gehören aber verschiedenen Verknüpfungen.
        let other = flugmappe_ids(&pool, 2).await.unwrap();
        assert!(other.section(other.links[0].id, "s1").is_some());
    }

    #[tokio::test]
    async fn flugmappe_start_runs_at_once_and_is_linked() {
        let pool = test_pool().await;
        let id = create_from_flugmappe(&pool, &fm_start("fm-a"), 7)
            .await
            .unwrap();
        let m = get(&pool, id).await.unwrap().unwrap();
        assert_eq!(m.state, MissionState::Running);
        assert_eq!(
            (m.started_at_ms, m.origin),
            (Some(7), MissionOrigin::Flugmappe)
        );
        let link = &m.flugmappe_links[0];
        assert_eq!(
            (
                link.linked_by.as_str(),
                link.linked_at_ms,
                link.aircraft_label.as_str()
            ),
            ("Testpilot", 7, "Matrice 350 RTK")
        );
        assert!(m.has_active_flugmappe());
        assert_eq!(find_by_flugmappe(&pool, "fm-a").await.unwrap(), Some(id));
        let ids = flugmappe_ids(&pool, id).await.unwrap();
        let l = ids.link("fm-a").unwrap().id;
        assert_eq!(ids.section(l, "fm-a-s1"), Some(m.sections[0].id));
        assert_eq!(ids.of_section(l, m.sections[0].id), Some("fm-a-s1"));
        assert_eq!(ids.active_section(l), Some(m.sections[0].id));

        // Ein zweiter laufender Einsatz scheitert am Teilindex (409).
        let err: AppError = create_from_flugmappe(&pool, &fm_start("fm-b"), 8)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        assert!(close(&pool, id, 9).await.unwrap());
        // Dieselbe Flugmappe hat höchstens eine nicht gelöste Verknüpfung.
        let err: AppError = create_from_flugmappe(&pool, &fm_start("fm-a"), 10)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        let other = create(&pool, &mc("B"), "u", 11).await.unwrap();
        let insert = |kind| LinkWrite {
            start: false,
            link: kind,
            section: SectionOp::Keep,
        };
        let new = NewLink {
            flugmappe_mission_id: "fm-a",
            linked_by: "x",
            aircraft_label: "M200",
        };
        let err: AppError = link_flugmappe(&pool, other.id, &insert(LinkOp::Insert(new)), 12)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");

        // Gelöst blockiert sie nichts mehr.
        assert!(release_link(&pool, id, l, 13).await.unwrap());
        assert!(
            !release_link(&pool, id, l, 14).await.unwrap(),
            "schon gelöst"
        );
        assert!(
            !release_link(&pool, other.id, l, 14).await.unwrap(),
            "fremder Einsatz"
        );
        assert_eq!(find_by_flugmappe(&pool, "fm-a").await.unwrap(), None);
        link_flugmappe(&pool, other.id, &insert(LinkOp::Insert(new)), 15)
            .await
            .unwrap();
        assert_eq!(
            find_by_flugmappe(&pool, "fm-a").await.unwrap(),
            Some(other.id)
        );
    }

    #[tokio::test]
    async fn sections_of_the_flugmappe_are_unique_per_link() {
        let pool = test_pool().await;
        let a = create_from_flugmappe(&pool, &fm_start("fm-a"), 1)
            .await
            .unwrap();
        let ids = flugmappe_ids(&pool, a).await.unwrap();
        let la = ids.link("fm-a").unwrap().id;
        let input = SectionCreate {
            label: "Nord".into(),
            location: Some("Wald".into()),
        };
        let new = NewLink {
            flugmappe_mission_id: "fm-b",
            linked_by: "Zweite",
            aircraft_label: "Matrice 200",
        };
        // Die zweite Flugmappe: der aktive Abschnitt gehört der ersten, also ein neuer.
        let write = LinkWrite {
            start: false,
            link: LinkOp::Insert(new),
            section: SectionOp::Create {
                flugmappe_section_id: "fm-a-s1",
                input: &input,
            },
        };
        link_flugmappe(&pool, a, &write, 2).await.unwrap();
        let ids = flugmappe_ids(&pool, a).await.unwrap();
        let lb = ids.link("fm-b").unwrap().id;
        let s_b = ids.section(lb, "fm-a-s1").unwrap();
        assert_ne!(
            ids.section(la, "fm-a-s1"),
            Some(s_b),
            "gleiche lokale ID, verschiedene Abschnitte"
        );
        let s2 = add_section_from_flugmappe(&pool, a, la, "s2", &input, 3)
            .await
            .unwrap();
        let err: AppError = add_section_from_flugmappe(&pool, a, la, "s2", &input, 4)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        let ids = flugmappe_ids(&pool, a).await.unwrap();
        assert_eq!(ids.active_section(la), Some(s2));
        assert_eq!(ids.active_section(lb), Some(s_b));
        assert_eq!(ids.of_section(lb, s2), None, "fremder Abschnitt");

        // Ein Browser-Abschnitt wird beim Verknüpfen übernommen; ohne Ort bleibt der vorhandene.
        let b = create(&pool, &mc("B"), "u", 5).await.unwrap();
        let bind_input = SectionCreate {
            label: "Erster".into(),
            location: None,
        };
        let write = LinkWrite {
            start: true,
            link: LinkOp::Insert(NewLink {
                flugmappe_mission_id: "fm-c",
                ..new
            }),
            section: SectionOp::Bind {
                section_id: b.sections[0].id,
                flugmappe_section_id: "c1",
                input: &bind_input,
            },
        };
        assert!(close(&pool, a, 6).await.unwrap());
        link_flugmappe(&pool, b.id, &write, 7).await.unwrap();
        let m = get(&pool, b.id).await.unwrap().unwrap();
        assert_eq!(
            (
                m.state,
                m.sections[0].label.as_str(),
                m.sections[0].location.as_deref()
            ),
            (MissionState::Running, "Erster", Some("Wald"))
        );
        assert!(
            !set_section_location_if_empty(&pool, m.sections[0].id, "52.4, 10.0")
                .await
                .unwrap()
        );
        set_map_photo(&pool, s2, 42).await.unwrap();
        let m = get(&pool, a).await.unwrap().unwrap();
        let s = m.sections.iter().find(|s| s.id == s2).unwrap();
        assert_eq!((s.has_map_photo, s.map_photo_at_ms), (true, Some(42)));
    }

    #[tokio::test]
    async fn done_reviving_and_import_once() {
        let pool = test_pool().await;
        let a = create_from_flugmappe(&pool, &fm_start("fm-a"), 1)
            .await
            .unwrap();
        let l = flugmappe_ids(&pool, a).await.unwrap().links[0].id;
        mark_link_done(&pool, l, 5).await.unwrap();
        mark_link_done(&pool, l, 6).await.unwrap();
        assert_eq!(links(&pool, a).await.unwrap()[0].done_at_ms, Some(5));
        assert!(release_link(&pool, a, l, 7).await.unwrap());
        let write = LinkWrite {
            start: false,
            link: LinkOp::Revive {
                id: l,
                aircraft_label: "Matrice 200",
            },
            section: SectionOp::Keep,
        };
        link_flugmappe(&pool, a, &write, 8).await.unwrap();
        let row = &links(&pool, a).await.unwrap()[0];
        assert_eq!(
            (
                row.released_at_ms,
                row.done_at_ms,
                row.aircraft_label.as_str()
            ),
            (None, None, "Matrice 200")
        );
        assert_eq!(links(&pool, a).await.unwrap().len(), 1, "keine zweite");

        assert!(mark_map_imported(&pool, a, 5).await.unwrap());
        assert!(!mark_map_imported(&pool, a, 6).await.unwrap());
        assert_eq!(
            flugmappe_ids(&pool, a).await.unwrap().map_imported_at_ms,
            Some(5)
        );
        assert_eq!(
            flugmappe_ids(&pool, 999).await.unwrap(),
            FlugmappeIds::default()
        );
    }

    #[tokio::test]
    async fn deleting_a_mission_takes_its_links_and_logbook() {
        let pool = test_pool().await;
        let a = create_from_flugmappe(&pool, &fm_start("fm-a"), 1)
            .await
            .unwrap();
        let ids = flugmappe_ids(&pool, a).await.unwrap();
        let l = ids.links[0].id;
        let entry = crate::store::logbook::NewEntry {
            revision: 1,
            flight_id: None,
            section_id: Some(ids.sections[0].id),
            started_at_ms: 1,
            landed_at_ms: None,
            aircraft_label: "M200".into(),
            pilot: None,
            landing_status: None,
            changed_at_ms: 1,
        };
        crate::store::logbook::put(&pool, l, "e1", &entry, 2)
            .await
            .unwrap();
        assert!(delete_rows(&pool, a, &[]).await.unwrap().is_some());
        for t in ["mission_flugmappe_links", "flugmappe_logbook_entries"] {
            assert_eq!(rows(&pool, t).await, 0, "{t}");
        }
    }
}

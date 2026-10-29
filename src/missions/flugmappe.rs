//! Einsätze für die Flugmappe (SUL-66): anlegen, verknüpfen und übernehmen, Abschnitte, Flüge,
//! Einsatzkarte, Kartenelemente übernehmen, Kartenfoto, Abschluss. Schreibendes läuft unter der
//! Schreibsperre der Einsätze; jede Änderung meldet `missions_changed` an die Browser.

use std::collections::HashMap;

use crate::contract::{
    FmFeatureCollection, FmFlight, FmLinkInfo, FmMapImport, FmMapImportResult, FmMission,
    FmMissionCreate, FmMissionLink, FmSection, FmSectionCreate, FmSectionUpdate, MapElementOrigin,
    Mission, MissionOrigin, MissionState, SectionCreate,
};
use crate::error::AppError;
use crate::map::geojson::{self, Track};
use crate::map::{self, MapChange, NewElement};
use crate::store::missions::{FlugmappeIds, FlugmappeStart, LinkWrite};
use crate::store::{flights, map_elements, missions as store, photos, remote_controllers};

use super::{MissionHub, rules};

/// Sicht der Flugmappe auf einen Einsatz: mit ihren IDs und der Verknüpfung samt UUID.
pub fn fm_mission(m: Mission, ids: &FlugmappeIds) -> FmMission {
    let link = match (&ids.mission, &m.flugmappe) {
        (Some(id), Some(l)) => Some(FmLinkInfo {
            flugmappe_mission_id: id.clone(),
            linked_by: l.linked_by.clone(),
            linked_at_ms: l.linked_at_ms,
        }),
        _ => None,
    };
    FmMission {
        id: m.id,
        name: m.name,
        location: m.location,
        state: m.state,
        started_at_ms: m.started_at_ms,
        created_at_ms: m.created_at_ms,
        created_by: m.created_by,
        sections: m
            .sections
            .into_iter()
            .map(|s| FmSection {
                flugmappe_section_id: ids.of_section(s.id).map(Into::into),
                id: s.id,
                label: s.label,
                location: s.location,
                created_at_ms: s.created_at_ms,
                has_map_photo: s.has_map_photo,
            })
            .collect(),
        link,
    }
}

fn validation(e: String) -> AppError {
    AppError::Validation(e)
}

impl MissionHub {
    async fn fm_view(&self, mission: Mission) -> Result<FmMission, AppError> {
        let ids = store::flugmappe_ids(&self.pool, mission.id).await?;
        Ok(fm_mission(mission, &ids))
    }

    /// Einsatz, an dem die aufrufende Flugmappe arbeiten darf (404, sonst 409 bei fremder Bindung).
    async fn bound(&self, id: i64, caller: &str) -> Result<(Mission, FlugmappeIds), AppError> {
        let mission = self.get(id).await?;
        let ids = store::flugmappe_ids(&self.pool, id).await?;
        rules::binding(ids.mission.as_deref(), caller)?;
        Ok((mission, ids))
    }

    /// Wie `bound`, aber nur für einen offenen Einsatz (422, wenn abgeschlossen).
    async fn bound_open(&self, id: i64, caller: &str) -> Result<(Mission, FlugmappeIds), AppError> {
        let (mission, ids) = self.bound(id, caller).await?;
        if mission.state == MissionState::Closed {
            return Err(AppError::Unprocessable(rules::CLOSED.into()));
        }
        Ok((mission, ids))
    }

    /// Vorbereitete und laufende Einsätze zur Auswahl in der Flugmappe.
    pub async fn flugmappe_missions(&self) -> Result<Vec<FmMission>, AppError> {
        let mut out = Vec::new();
        for m in self.list().await? {
            if m.state != MissionState::Closed {
                out.push(self.fm_view(m).await?);
            }
        }
        Ok(out)
    }

    /// Neuer laufender Einsatz aus der Flugmappe mit ihrem ersten Abschnitt. Wiederholt (Antwort
    /// verloren) liefert es den schon angelegten, solange er offen ist (`rules::create_again`).
    pub async fn create_from_flugmappe(
        &self,
        input: FmMissionCreate,
        created_by: &str,
        now_ms: i64,
    ) -> Result<FmMission, AppError> {
        let start = FlugmappeStart {
            name: rules::name(&input.name, "Name").map_err(validation)?,
            location: rules::location(input.location),
            created_by: created_by.into(),
            flugmappe_mission_id: rules::flugmappe_mission_id(&input.flugmappe_mission_id)
                .map_err(validation)?,
            flugmappe_section_id: rules::flugmappe_section_id(&input.flugmappe_section_id)
                .map_err(validation)?,
            section_label: rules::name(&input.section_label, "Bezeichnung").map_err(validation)?,
            section_location: rules::location(input.section_location),
        };
        let _serial = self.writes.lock().await;
        if let Some(existing) =
            store::find_by_flugmappe(&self.pool, &start.flugmappe_mission_id).await?
        {
            let mission = self.get(existing).await?;
            rules::create_again(rules::Existing {
                state: mission.state,
                created_by_caller: mission.origin == MissionOrigin::Flugmappe
                    && mission
                        .flugmappe
                        .as_ref()
                        .is_some_and(|l| l.linked_at_ms == mission.created_at_ms),
            })?;
            return self.fm_view(mission).await;
        }
        if store::running_id(&self.pool).await?.is_some() {
            return Err(AppError::Conflict(rules::OTHER_RUNNING.into()));
        }
        let id = store::create_from_flugmappe(&self.pool, &start, now_ms).await?;
        self.map.set_pilot_scope(Some(id));
        self.changed();
        self.fm_view(self.get(id).await?).await
    }

    /// Verknüpft einen vorbereiteten (dann läuft er) oder laufenden Einsatz; eine fremde
    /// Verknüpfung nur mit `take_over`. Der aktive Abschnitt wird der der Flugmappe.
    pub async fn link(
        &self,
        id: i64,
        input: FmMissionLink,
        linked_by: &str,
        now_ms: i64,
    ) -> Result<FmMission, AppError> {
        let caller =
            rules::flugmappe_mission_id(&input.flugmappe_mission_id).map_err(validation)?;
        let section =
            rules::flugmappe_section_id(&input.flugmappe_section_id).map_err(validation)?;
        let label = rules::name(&input.section_label, "Bezeichnung").map_err(validation)?;
        let location = rules::location(input.section_location);
        let _serial = self.writes.lock().await;
        let mission = self.get(id).await?;
        let ids = store::flugmappe_ids(&self.pool, id).await?;
        let running = store::running_id(&self.pool).await?;
        let plan = rules::link(rules::LinkFacts {
            state: mission.state,
            linked: ids.mission.as_deref(),
            caller: &caller,
            take_over: input.take_over,
            other_running: running.is_some_and(|r| r != id),
            caller_linked_elsewhere: store::find_by_flugmappe(&self.pool, &caller)
                .await?
                .is_some_and(|other| other != id),
        })?;
        let sections: Vec<(i64, Option<&str>)> = mission
            .sections
            .iter()
            .map(|s| (s.id, ids.of_section(s.id)))
            .collect();
        let bind_section = match rules::section_for_link(&sections, &section) {
            rules::SectionStep::Bind(section_id) => Some((
                section_id,
                section.as_str(),
                label.as_str(),
                location.as_deref(),
            )),
            rules::SectionStep::Known(_) => None,
        };
        // Starten, Verknüpfen und Abschnitt binden in einer Transaktion. Ein zweiter laufender
        // Einsatz scheitert am Teilindex (409), dann bleibt auch die Verknüpfung ungeschrieben.
        let write = LinkWrite {
            start: plan.start,
            link: plan.write_link.then_some((caller.as_str(), linked_by)),
            bind_section,
        };
        store::link_flugmappe(&self.pool, id, &write, now_ms).await?;
        if plan.start {
            self.map
                .set_pilot_scope(store::running_id(&self.pool).await?);
        }
        self.changed();
        self.fm_view(self.get(id).await?).await
    }

    /// Verlegung in der Flugmappe: neuer aktiver Abschnitt. Eine bekannte ID liefert den Einsatz
    /// unverändert (Wiederholung).
    pub async fn add_flugmappe_section(
        &self,
        id: i64,
        caller: &str,
        input: FmSectionCreate,
        now_ms: i64,
    ) -> Result<FmMission, AppError> {
        let section =
            rules::flugmappe_section_id(&input.flugmappe_section_id).map_err(validation)?;
        let input = SectionCreate {
            label: rules::name(&input.label, "Bezeichnung").map_err(validation)?,
            location: rules::location(input.location),
        };
        let _serial = self.writes.lock().await;
        let (mission, ids) = self.bound_open(id, caller).await?;
        if ids.section(&section).is_some() {
            return self.fm_view(mission).await;
        }
        store::add_section_from_flugmappe(&self.pool, id, &section, &input, now_ms).await?;
        self.changed();
        self.fm_view(self.get(id).await?).await
    }

    /// Bezeichnung und Ort eines Abschnitts der Flugmappe nachreichen (SUL-81). Ohne Änderung
    /// bleibt alles still (die Ausgangsliste wiederholt nach einer verlorenen Antwort).
    pub async fn update_flugmappe_section(
        &self,
        id: i64,
        caller: &str,
        flugmappe_section_id: &str,
        input: FmSectionUpdate,
    ) -> Result<FmMission, AppError> {
        let input = SectionCreate {
            label: rules::name(&input.label, "Bezeichnung").map_err(validation)?,
            location: rules::location(input.location),
        };
        let _serial = self.writes.lock().await;
        let (mission, ids) = self.bound_open(id, caller).await?;
        let section = ids
            .section(flugmappe_section_id)
            .ok_or(AppError::NotFound)?;
        let unchanged = mission
            .sections
            .iter()
            .any(|s| s.id == section && s.label == input.label && s.location == input.location);
        if unchanged {
            return self.fm_view(mission).await;
        }
        store::update_section(&self.pool, section, &input).await?;
        self.changed();
        self.fm_view(self.get(id).await?).await
    }

    /// Flüge des Einsatzes als Vorschläge für das Flugbuch, älteste zuerst.
    pub async fn flugmappe_flights(
        &self,
        id: i64,
        caller: &str,
    ) -> Result<Vec<FmFlight>, AppError> {
        let (_, ids) = self.bound(id, caller).await?;
        let shots: HashMap<i64, u32> = photos::shots_per_flight(&self.pool, id)
            .await?
            .into_iter()
            .collect();
        let labels: HashMap<String, String> = remote_controllers::labels(&self.pool)
            .await?
            .into_iter()
            .collect();
        Ok(flights::of_mission_with_section(&self.pool, id)
            .await?
            .into_iter()
            .map(|(f, section)| FmFlight {
                flugmappe_section_id: section.and_then(|s| ids.of_section(s)).map(Into::into),
                aircraft_label: labels.get(&f.gateway_sn).cloned(),
                photo_count: shots.get(&f.id).copied().unwrap_or(0),
                id: f.id,
                started_at_ms: f.started_at_ms,
                ended_at_ms: f.ended_at_ms,
                end_reason: f.end_reason,
                aircraft_sn: f.aircraft_sn,
                max_elevation_m: f.max_elevation_m,
                distance_m: f.distance_m,
            })
            .collect())
    }

    /// Einsatzkarte: alle Kartenelemente, Flugspuren mit `section` nur aus diesem Abschnitt der
    /// Flugmappe (unbekannt → 404).
    pub async fn flugmappe_map(
        &self,
        id: i64,
        caller: &str,
        section: Option<&str>,
    ) -> Result<FmFeatureCollection, AppError> {
        let (_, ids) = self.bound(id, caller).await?;
        let only = match section {
            Some(fm) => Some(ids.section(fm).ok_or(AppError::NotFound)?),
            None => None,
        };
        let elements = self.map.list_in(Some(id)).await?;
        let mut tracks = Vec::new();
        for (flight, section) in flights::of_mission_with_section(&self.pool, id).await? {
            if only.is_some_and(|o| section != Some(o)) {
                continue;
            }
            tracks.push(Track {
                flugmappe_section_id: section.and_then(|s| ids.of_section(s)).map(Into::into),
                segments: flights::segments(&self.pool, flight.id).await?,
                flight,
            });
        }
        Ok(geojson::feature_collection(&elements, &tracks))
    }

    /// Übernimmt Zeichnungen und Standort der Flugmappe einmal je Einsatz; danach ein No-op.
    pub async fn import_map(
        &self,
        id: i64,
        caller: &str,
        input: FmMapImport,
        author: &str,
        now_ms: i64,
    ) -> Result<FmMapImportResult, AppError> {
        let drawings = rules::import(&input).map_err(validation)?;
        let _serial = self.writes.lock().await;
        let (mission, ids) = self.bound_open(id, caller).await?;
        if ids.map_imported_at_ms.is_some() {
            return Ok(FmMapImportResult { imported: 0 });
        }
        let author = format!("{author} (Flugmappe)");
        let elements = drawings
            .into_iter()
            .map(|d| {
                let new = NewElement {
                    id: None,
                    name: d.name,
                    kind: d.kind,
                    points: d.points,
                    color: d.color.into(),
                    // Ohne Höhe: Pilot 2 legt das Element auf das Gelände.
                    clamp_to_ground: true,
                    author: author.clone(),
                    origin: MapElementOrigin::Web,
                    mission_id: Some(id),
                };
                map::element(new, now_ms)
            })
            .collect::<Result<Vec<_>, _>>()?;
        // Markieren, anlegen und den Ort setzen in einer Transaktion: Scheitert etwas, bleibt
        // nichts, und eine Wiederholung legt nichts doppelt an. Verteilt wird erst danach.
        let map_serial = self.map.serial().await;
        let mut tx = self.pool.begin().await?;
        if !store::mark_map_imported(&mut *tx, id, now_ms).await? {
            return Ok(FmMapImportResult { imported: 0 });
        }
        for e in &elements {
            map_elements::insert(&mut *tx, e).await?;
        }
        if let (Some(p), Some(active)) = (input.standort, mission.sections.last()) {
            store::set_section_location_if_empty(&mut *tx, active.id, &rules::standort_location(p))
                .await?;
        }
        tx.commit().await?;
        let imported = elements.len() as u32;
        for e in elements {
            self.map.publish(MapChange::Created(e));
        }
        drop(map_serial);
        self.changed();
        Ok(FmMapImportResult { imported })
    }

    /// Foto der Papierkarte für einen Abschnitt der Flugmappe; ersetzt ein vorhandenes. Erst die
    /// Datei (temporär, dann umbenannt), dann die Zeile.
    pub async fn set_map_photo(
        &self,
        id: i64,
        caller: &str,
        flugmappe_section_id: &str,
        jpeg: &[u8],
        now_ms: i64,
    ) -> Result<(), AppError> {
        if !rules::is_jpeg(jpeg) {
            return Err(AppError::Validation("Kartenfoto muss ein JPEG sein".into()));
        }
        let _serial = self.writes.lock().await;
        let (_, ids) = self.bound_open(id, caller).await?;
        let section = ids
            .section(flugmappe_section_id)
            .ok_or(AppError::NotFound)?;
        let path = self.map_photo_path(section);
        let tmp = path.with_extension(format!("jpg.{}.tmp", uuid::Uuid::new_v4()));
        let io = |e: std::io::Error| AppError::Internal(format!("Kartenfoto {section}: {e}"));
        tokio::fs::create_dir_all(&self.map_photos)
            .await
            .map_err(io)?;
        tokio::fs::write(&tmp, jpeg).await.map_err(io)?;
        if let Err(e) = tokio::fs::rename(&tmp, &path).await {
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(io(e));
        }
        store::set_map_photo(&self.pool, section, now_ms).await?;
        self.changed();
        Ok(())
    }

    /// Abschluss aus der Flugmappe. Schon abgeschlossen ist kein Fehler (Abschluss nachholen).
    pub async fn close_from_flugmappe(
        &self,
        id: i64,
        caller: &str,
        now_ms: i64,
    ) -> Result<FmMission, AppError> {
        let _serial = self.writes.lock().await;
        let (mission, _) = self.bound(id, caller).await?;
        let mission = if mission.state == MissionState::Closed {
            mission
        } else {
            self.change_state_locked(id, MissionState::Closed, now_ms)
                .await?
        };
        self.fm_view(mission).await
    }

    /// Ein verknüpfter Einsatz für die Flugmappe (nur mit passender Bindung).
    pub async fn flugmappe_mission(&self, id: i64, caller: &str) -> Result<FmMission, AppError> {
        let (mission, ids) = self.bound(id, caller).await?;
        Ok(fm_mission(mission, &ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{
        FlightSummary, FmImportFeature, GeoPoint, MapElementKind, MissionCreate, MissionOrigin,
        examples,
    };

    const FM_A: &str = "0f6b2a1e-5d43-4c8e-9b7a-2e1f3c4d5a6b";
    const FM_B: &str = "1a2b3c4d-5e6f-4a7b-8c9d-0e1f2a3b4c5d";

    struct Fixture {
        hub: MissionHub,
        _dir: tempfile::TempDir,
    }

    async fn hub() -> Fixture {
        let pool = crate::db::test_pool().await;
        let (_stop, rx) = tokio::sync::watch::channel(false);
        let map = crate::map::MapHub::new(pool.clone(), rx);
        let dir = tempfile::tempdir().unwrap();
        let hub = MissionHub::start_up(pool, map, dir.path().join("kartenfotos"))
            .await
            .unwrap();
        Fixture { hub, _dir: dir }
    }

    fn create_input(fm: &str) -> FmMissionCreate {
        FmMissionCreate {
            name: " Vermisste Person ".into(),
            location: Some("Wald".into()),
            section_label: "Abschnitt 1".into(),
            section_location: Some(" ".into()),
            flugmappe_mission_id: fm.to_uppercase(),
            flugmappe_section_id: "s1".into(),
        }
    }

    fn link_input(fm: &str, section: &str, take_over: bool) -> FmMissionLink {
        FmMissionLink {
            flugmappe_mission_id: fm.into(),
            flugmappe_section_id: section.into(),
            section_label: "Abschnitt A".into(),
            section_location: None,
            take_over,
        }
    }

    async fn prepared(hub: &MissionHub, name: &str) -> i64 {
        hub.create(
            MissionCreate {
                name: name.into(),
                location: None,
            },
            "Browser",
            1,
        )
        .await
        .unwrap()
        .id
    }

    #[tokio::test]
    async fn the_flugmappe_creates_a_running_linked_mission_once() {
        let f = hub().await;
        let mut changes = f.hub.subscribe();
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        assert!(changes.try_recv().is_ok(), "Browser laden neu");
        assert_eq!(
            (m.name.as_str(), m.state),
            ("Vermisste Person", MissionState::Running)
        );
        assert_eq!(m.sections[0].flugmappe_section_id.as_deref(), Some("s1"));
        assert_eq!(m.sections[0].location, None, "leerer Ort heißt keiner");
        let link = m.link.unwrap();
        assert_eq!(
            (link.flugmappe_mission_id.as_str(), link.linked_by.as_str()),
            (FM_A, "Testpilot")
        );
        assert_eq!(
            f.hub.get(m.id).await.unwrap().origin,
            MissionOrigin::Flugmappe
        );
        assert_eq!(f.hub.map.pilot_scope(), Some(m.id));

        let err = f
            .hub
            .create_from_flugmappe(create_input(FM_B), "Testpilot", 11)
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), rules::OTHER_RUNNING);
        f.hub.close(m.id, 12).await.unwrap();
        let err = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 13)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AppError::Unprocessable(m) if m == rules::CLOSED),
            "abgeschlossen: 422, {err:?}"
        );
        let mut bad = create_input(FM_B);
        bad.flugmappe_mission_id = "keine-uuid".into();
        assert!(matches!(
            f.hub.create_from_flugmappe(bad, "x", 14).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn creating_again_returns_the_open_mission_of_this_flugmappe() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let mut changes = f.hub.subscribe();
        let again = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 11)
            .await
            .unwrap();
        assert_eq!(again, m, "Antwort verloren, Wiederholung");
        assert!(changes.try_recv().is_err(), "nichts geändert");
        assert_eq!(f.hub.list().await.unwrap().len(), 1);

        // Übernommen: Die UUID der neuen Flugmappe gehört zu einem Einsatz, den sie nicht angelegt hat.
        f.hub
            .link(m.id, link_input(FM_B, "t1", true), "Zweite", 20)
            .await
            .unwrap();
        let mut by_b = create_input(FM_B);
        by_b.flugmappe_section_id = "t1".into();
        let err = f
            .hub
            .create_from_flugmappe(by_b, "Zweite", 21)
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), rules::FLUGMAPPE_TAKEN);
    }

    #[tokio::test]
    async fn creating_with_the_uuid_of_a_linked_browser_mission_is_409() {
        let f = hub().await;
        let id = prepared(&f.hub, "Aus dem Browser").await;
        f.hub
            .link(id, link_input(FM_A, "s1", false), "Testpilot", 5)
            .await
            .unwrap();
        let err = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 6)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
    }

    #[tokio::test]
    async fn a_failed_link_leaves_no_running_mission_without_its_link() {
        let f = hub().await;
        let id = prepared(&f.hub, "Vorbereitet").await;
        sqlx::query(
            "CREATE TRIGGER fail_bind BEFORE UPDATE ON mission_sections \
             BEGIN SELECT RAISE(ABORT, 'Testfehler'); END",
        )
        .execute(&f.hub.pool)
        .await
        .unwrap();
        assert!(
            f.hub
                .link(id, link_input(FM_A, "s1", false), "Testpilot", 20)
                .await
                .is_err()
        );
        let m = f.hub.get(id).await.unwrap();
        assert_eq!(
            (m.state, m.flugmappe),
            (MissionState::Prepared, None),
            "ganz oder gar nicht"
        );
        assert_eq!(f.hub.map.pilot_scope(), None);

        sqlx::query("DROP TRIGGER fail_bind")
            .execute(&f.hub.pool)
            .await
            .unwrap();
        let linked = f
            .hub
            .link(id, link_input(FM_A, "s1", false), "Testpilot", 21)
            .await
            .unwrap();
        assert_eq!(linked.state, MissionState::Running);
        assert_eq!(f.hub.map.pilot_scope(), Some(id));
    }

    #[tokio::test]
    async fn a_second_running_mission_fails_the_whole_link_on_the_partial_index() {
        let f = hub().await;
        let a = prepared(&f.hub, "A").await;
        let b = prepared(&f.hub, "B").await;
        f.hub.start(a, 2).await.unwrap();
        // Am Hub vorbei, wie ein Start, der die Prüfung überholt hätte.
        let write = LinkWrite {
            start: true,
            link: Some((FM_A, "Testpilot")),
            bind_section: None,
        };
        let err: AppError = store::link_flugmappe(&f.hub.pool, b, &write, 3)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        let m = f.hub.get(b).await.unwrap();
        assert_eq!((m.state, m.flugmappe), (MissionState::Prepared, None));
    }

    #[tokio::test]
    async fn linking_starts_a_prepared_mission_and_binds_its_active_section() {
        let f = hub().await;
        let id = prepared(&f.hub, "Vorbereitet").await;
        let m = f
            .hub
            .link(id, link_input(FM_A, "s1", false), "Testpilot", 20)
            .await
            .unwrap();
        assert_eq!(
            (m.state, m.started_at_ms),
            (MissionState::Running, Some(20))
        );
        assert_eq!(m.sections.len(), 1);
        assert_eq!(
            (
                m.sections[0].flugmappe_section_id.as_deref(),
                m.sections[0].label.as_str()
            ),
            (Some("s1"), "Abschnitt A")
        );
        // Wiederholt: nichts ändert sich, auch nicht wer verknüpft hat.
        let again = f
            .hub
            .link(id, link_input(FM_A, "s1", false), "Jemand", 30)
            .await
            .unwrap();
        assert_eq!(again, m);
        assert_eq!(f.hub.get(id).await.unwrap().origin, MissionOrigin::Web);

        // Eine zweite Flugmappe braucht Übernehmen; danach ist die erste abgelöst.
        let err = f
            .hub
            .link(id, link_input(FM_B, "t1", false), "Zweite", 40)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        let taken = f
            .hub
            .link(id, link_input(FM_B, "t1", true), "Zweite", 50)
            .await
            .unwrap();
        let link = taken.link.unwrap();
        assert_eq!(
            (
                link.flugmappe_mission_id.as_str(),
                link.linked_by.as_str(),
                link.linked_at_ms
            ),
            (FM_B, "Zweite", 50)
        );
        assert_eq!(
            taken.sections[0].flugmappe_section_id.as_deref(),
            Some("t1")
        );
        let err = f.hub.flugmappe_flights(id, FM_A).await.unwrap_err();
        assert_eq!(err.to_string(), rules::OTHER_FLUGMAPPE);
        assert!(f.hub.flugmappe_flights(id, FM_B).await.is_ok());
    }

    #[tokio::test]
    async fn linking_refuses_while_another_runs_or_when_closed() {
        let f = hub().await;
        let a = prepared(&f.hub, "A").await;
        let b = prepared(&f.hub, "B").await;
        f.hub.start(a, 2).await.unwrap();
        let err = f
            .hub
            .link(b, link_input(FM_A, "s1", false), "x", 3)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        assert_eq!(
            f.hub.get(b).await.unwrap().flugmappe,
            None,
            "nichts geschrieben"
        );
        f.hub.close(b, 4).await.unwrap();
        let err = f
            .hub
            .link(b, link_input(FM_A, "s1", true), "x", 5)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unprocessable(_)), "{err:?}");
        // Laufend ohne Flugmappe: darf verknüpft werden.
        assert!(
            f.hub
                .link(a, link_input(FM_A, "s1", false), "x", 6)
                .await
                .is_ok()
        );
        assert!(matches!(
            f.hub.link(999, link_input(FM_A, "s1", false), "x", 7).await,
            Err(AppError::NotFound)
        ));
    }

    #[tokio::test]
    async fn sections_come_from_the_flugmappe_and_repeat_harmlessly() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let input = FmSectionCreate {
            flugmappe_section_id: "s2".into(),
            label: "Verlegung".into(),
            location: Some("Waldrand".into()),
        };
        let after = f
            .hub
            .add_flugmappe_section(m.id, FM_A, input.clone(), 11)
            .await
            .unwrap();
        assert_eq!(after.sections.len(), 2);
        let again = f
            .hub
            .add_flugmappe_section(m.id, FM_A, input.clone(), 12)
            .await
            .unwrap();
        assert_eq!(again, after);
        assert!(matches!(
            f.hub
                .add_flugmappe_section(m.id, FM_B, input.clone(), 13)
                .await,
            Err(AppError::Conflict(_))
        ));
        let browser = f
            .hub
            .add_section(
                m.id,
                SectionCreate {
                    label: "Nord".into(),
                    location: None,
                },
                14,
            )
            .await
            .unwrap_err();
        assert_eq!(browser.to_string(), rules::SECTIONS_FROM_FLUGMAPPE);
        f.hub.close(m.id, 15).await.unwrap();
        let err = f
            .hub
            .add_flugmappe_section(
                m.id,
                FM_A,
                FmSectionCreate {
                    flugmappe_section_id: "s3".into(),
                    ..input
                },
                16,
            )
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unprocessable(_)), "{err:?}");
    }

    #[tokio::test]
    async fn the_location_of_a_section_comes_later_from_the_flugmappe() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        f.hub
            .add_flugmappe_section(
                m.id,
                FM_A,
                FmSectionCreate {
                    flugmappe_section_id: "s2".into(),
                    label: "Standort 2".into(),
                    location: None,
                },
                11,
            )
            .await
            .unwrap();
        let update = |label: &str, location: Option<&str>| FmSectionUpdate {
            label: label.into(),
            location: location.map(Into::into),
        };
        let after = f
            .hub
            .update_flugmappe_section(m.id, FM_A, "s2", update(" Standort 2 ", Some(" Waldrand ")))
            .await
            .unwrap();
        let s2 = &after.sections[1];
        assert_eq!(
            (s2.label.as_str(), s2.location.as_deref()),
            ("Standort 2", Some("Waldrand"))
        );
        assert_eq!(after.sections[0].location, None, "der erste bleibt");
        assert_eq!(
            f.hub.get(m.id).await.unwrap().sections.last().unwrap().id,
            s2.id,
            "der aktive Abschnitt bleibt der aktive"
        );
        let again = f
            .hub
            .update_flugmappe_section(m.id, FM_A, "s2", update("Standort 2", Some("Waldrand")))
            .await
            .unwrap();
        assert_eq!(again, after, "Wiederholung ändert nichts");
        let cleared = f
            .hub
            .update_flugmappe_section(m.id, FM_A, "s1", update("Abschnitt 1", Some(" ")))
            .await
            .unwrap();
        assert_eq!(
            cleared.sections[0].location, None,
            "leerer Ort heißt keiner"
        );

        for (fm, section, label, expected) in [
            (FM_A, "s9", "X", "NotFound"),
            (FM_B, "s2", "X", "Conflict"),
            (FM_A, "s2", " ", "Validation"),
        ] {
            let err = f
                .hub
                .update_flugmappe_section(m.id, fm, section, update(label, None))
                .await
                .unwrap_err();
            assert!(format!("{err:?}").starts_with(expected), "{err:?}");
        }
        f.hub.close(m.id, 15).await.unwrap();
        let err = f
            .hub
            .update_flugmappe_section(m.id, FM_A, "s2", update("Standort 2", None))
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unprocessable(_)), "{err:?}");
    }

    fn flight(id: i64, ended: Option<i64>) -> FlightSummary {
        FlightSummary {
            id,
            aircraft_sn: format!("AC{id}"),
            gateway_sn: "RC1".into(),
            started_at_ms: id * 1_000,
            ended_at_ms: ended,
            end_reason: ended.map(|_| crate::contract::FlightEndReason::Landed),
            max_elevation_m: Some(60.0),
            distance_m: 100.0,
        }
    }

    #[tokio::test]
    async fn flights_and_map_carry_the_sections_of_the_flugmappe() {
        let f = hub().await;
        let pool = &f.hub.pool;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let (rc, _) = remote_controllers::create(pool, "RC Plus 1", 0)
            .await
            .unwrap();
        remote_controllers::bind(pool, rc.id, "RC1", "pw", "k", 1)
            .await
            .unwrap();
        flights::insert(pool, &flight(1, Some(1_500)))
            .await
            .unwrap();
        let pt = |t_ms| crate::contract::TrackPoint {
            t_ms,
            lat: 52.0,
            lon: 10.0,
            elevation_m: None,
        };
        flights::append_points(pool, 1, &[(0, pt(1)), (0, pt(2))])
            .await
            .unwrap();
        for (key, shot) in [("a_T", "a"), ("a_W", "a"), ("b_W", "b")] {
            let mut p = crate::store::photos::NewPhoto {
                object_key: key.into(),
                name: format!("{key}.JPG"),
                fingerprint: None,
                tiny_fingerprint: None,
                shot_key: format!("RC1/{shot}"),
                lens: crate::contract::PhotoLens::Wide,
                gateway_sn: "RC1".into(),
                aircraft_sn: None,
                flight_id: Some(1),
                taken_at_ms: None,
                uploaded_at_ms: 1,
                lat: None,
                lon: None,
                absolute_altitude_m: None,
                relative_altitude_m: None,
                gimbal_yaw_deg: None,
                payload_model_key: None,
            };
            p.object_key = key.into();
            crate::store::photos::upsert(pool, &p).await.unwrap();
        }
        f.hub
            .add_flugmappe_section(
                m.id,
                FM_A,
                FmSectionCreate {
                    flugmappe_section_id: "s2".into(),
                    label: "Verlegung".into(),
                    location: None,
                },
                2_000,
            )
            .await
            .unwrap();
        flights::insert(pool, &flight(3, None)).await.unwrap();
        flights::append_points(pool, 3, &[(0, pt(3)), (0, pt(4))])
            .await
            .unwrap();

        let list = f.hub.flugmappe_flights(m.id, FM_A).await.unwrap();
        let summary: Vec<_> = list
            .iter()
            .map(|f| {
                (
                    f.id,
                    f.flugmappe_section_id.as_deref(),
                    f.photo_count,
                    f.aircraft_label.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (1, Some("s1"), 2, Some("RC Plus 1")),
                (3, Some("s2"), 0, Some("RC Plus 1"))
            ]
        );
        assert!(list[1].ended_at_ms.is_none(), "laufender Flug erkennbar");

        let map = f.hub.flugmappe_map(m.id, FM_A, None).await.unwrap();
        assert_eq!(map.features.len(), 2);
        let only = f.hub.flugmappe_map(m.id, FM_A, Some("s2")).await.unwrap();
        assert_eq!(only.features.len(), 1);
        assert!(matches!(
            &only.features[0].properties,
            crate::contract::FmFeatureProperties::FlightTrack { flight_id: 3, .. }
        ));
        assert!(matches!(
            f.hub.flugmappe_map(m.id, FM_A, Some("gibt-es-nicht")).await,
            Err(AppError::NotFound)
        ));
        assert!(matches!(
            f.hub.flugmappe_map(m.id, FM_B, None).await,
            Err(AppError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn drawings_are_imported_once_into_the_mission() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let area = examples::map_elements().remove(0);
        let input = FmMapImport {
            features: vec![FmImportFeature {
                name: Some("Suchgebiet".into()),
                kind: MapElementKind::Polygon,
                points: area
                    .points
                    .iter()
                    .map(|p| GeoPoint {
                        lat: p.lat,
                        lon: p.lon,
                    })
                    .collect(),
            }],
            standort: Some(GeoPoint {
                lat: 52.43,
                lon: 10.01,
            }),
        };
        let mut map_changes = f.hub.map.subscribe();
        let done = f
            .hub
            .import_map(m.id, FM_A, input.clone(), "Testpilot", 11)
            .await
            .unwrap();
        assert_eq!(done.imported, 2);
        assert!(
            map_changes.try_recv().is_ok(),
            "Browser und Pilots sehen es"
        );
        let elements = f.hub.map.list_in(Some(m.id)).await.unwrap();
        let mut summary: Vec<_> = elements
            .iter()
            .map(|e| {
                (
                    e.name.as_str(),
                    e.color.as_str(),
                    e.author.as_str(),
                    e.origin,
                )
            })
            .collect();
        // Gleiche Anlegezeit: Die Reihenfolge der Liste hängt an der zufälligen ID.
        summary.sort_by(|a, b| b.0.cmp(a.0));
        assert_eq!(
            summary,
            vec![
                (
                    "Suchgebiet",
                    "#2D8CF0",
                    "Testpilot (Flugmappe)",
                    MapElementOrigin::Web
                ),
                (
                    "Standort",
                    "#E23C39",
                    "Testpilot (Flugmappe)",
                    MapElementOrigin::Web
                ),
            ]
        );
        let section = f.hub.get(m.id).await.unwrap().sections.remove(0);
        assert_eq!(section.location.as_deref(), Some("52.43000, 10.01000"));

        let again = f
            .hub
            .import_map(m.id, FM_A, input, "Testpilot", 12)
            .await
            .unwrap();
        assert_eq!(again.imported, 0);
        assert_eq!(f.hub.map.list_in(Some(m.id)).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_failed_import_leaves_nothing_and_can_be_repeated() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let point = |name: &str| FmImportFeature {
            name: Some(name.into()),
            kind: MapElementKind::Point,
            points: vec![GeoPoint {
                lat: 52.43,
                lon: 10.01,
            }],
        };
        let input = FmMapImport {
            features: vec![point("Erster"), point("Kaputt")],
            standort: Some(GeoPoint {
                lat: 52.43,
                lon: 10.01,
            }),
        };
        sqlx::query(
            "CREATE TRIGGER fail_insert BEFORE INSERT ON map_elements \
             WHEN NEW.name = 'Kaputt' BEGIN SELECT RAISE(ABORT, 'Testfehler'); END",
        )
        .execute(&f.hub.pool)
        .await
        .unwrap();
        let mut map_changes = f.hub.map.subscribe();
        assert!(
            f.hub
                .import_map(m.id, FM_A, input.clone(), "Testpilot", 11)
                .await
                .is_err()
        );
        assert!(f.hub.map.list_in(Some(m.id)).await.unwrap().is_empty());
        assert!(map_changes.try_recv().is_err(), "nichts verteilt");
        let ids = store::flugmappe_ids(&f.hub.pool, m.id).await.unwrap();
        assert_eq!(
            ids.map_imported_at_ms, None,
            "nicht als übernommen markiert"
        );
        assert_eq!(f.hub.get(m.id).await.unwrap().sections[0].location, None);

        sqlx::query("DROP TRIGGER fail_insert")
            .execute(&f.hub.pool)
            .await
            .unwrap();
        let done = f
            .hub
            .import_map(m.id, FM_A, input, "Testpilot", 12)
            .await
            .unwrap();
        assert_eq!(done.imported, 3);
        assert_eq!(f.hub.map.list_in(Some(m.id)).await.unwrap().len(), 3);
    }

    #[tokio::test]
    async fn map_photos_are_jpeg_only_and_replace_the_old_one() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        let section = m.sections[0].id;
        let jpeg = |n: u8| vec![0xFF, 0xD8, 0xFF, 0xE0, n];
        assert!(matches!(
            f.hub.set_map_photo(m.id, FM_A, "s1", b"\x89PNG", 11).await,
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            f.hub.set_map_photo(m.id, FM_A, "s9", &jpeg(1), 11).await,
            Err(AppError::NotFound)
        ));
        f.hub
            .set_map_photo(m.id, FM_A, "s1", &jpeg(1), 12)
            .await
            .unwrap();
        f.hub
            .set_map_photo(m.id, FM_A, "s1", &jpeg(2), 13)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read(f.hub.map_photo_path(section)).unwrap(),
            jpeg(2)
        );
        let entries = std::fs::read_dir(&f.hub.map_photos).unwrap().count();
        assert_eq!(entries, 1, "keine temporären Reste");
        let s = f.hub.get(m.id).await.unwrap().sections.remove(0);
        assert_eq!((s.has_map_photo, s.map_photo_at_ms), (true, Some(13)));
        assert!(f.hub.flugmappe_mission(m.id, FM_A).await.unwrap().sections[0].has_map_photo);
    }

    #[tokio::test]
    async fn closing_from_the_flugmappe_waits_for_landing_and_repeats_harmlessly() {
        let f = hub().await;
        let m = f
            .hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        flights::insert(&f.hub.pool, &flight(1, None))
            .await
            .unwrap();
        let err = f
            .hub
            .close_from_flugmappe(m.id, FM_A, 11)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unprocessable(_)), "{err:?}");
        flights::update(&f.hub.pool, &flight(1, Some(2_000)))
            .await
            .unwrap();
        assert!(matches!(
            f.hub.close_from_flugmappe(m.id, FM_B, 12).await,
            Err(AppError::Conflict(_))
        ));
        let closed = f.hub.close_from_flugmappe(m.id, FM_A, 13).await.unwrap();
        assert_eq!(closed.state, MissionState::Closed);
        assert_eq!(f.hub.map.pilot_scope(), None);
        let again = f.hub.close_from_flugmappe(m.id, FM_A, 14).await.unwrap();
        assert_eq!(again, closed);
        assert!(matches!(
            f.hub
                .set_map_photo(m.id, FM_A, "s1", &[0xFF, 0xD8, 0xFF], 15)
                .await,
            Err(AppError::Unprocessable(_))
        ));
        assert!(f.hub.flugmappe_missions().await.unwrap().is_empty());
    }
}

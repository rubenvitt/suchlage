//! Einsätze für die Flugmappe (SUL-66, mehrere Flugmappen SUL-113): anlegen, verknüpfen und
//! dazukommen, Abschnitte, Flüge, Flugbuch, Einsatzkarte, Kartenelemente übernehmen, Kartenfoto,
//! Abschluss. Schreibendes läuft unter der Schreibsperre der Einsätze; jede Änderung meldet
//! `missions_changed` an die Browser.

use std::collections::HashMap;

use crate::contract::{
    FmFeatureCollection, FmFlight, FmLinkInfo, FmLogbookEntry, FmLogbookStored, FmMapImport,
    FmMapImportResult, FmMission, FmMissionCreate, FmMissionLink, FmSection, FmSectionCreate,
    FmSectionUpdate, MapElementOrigin, Mission, MissionFlight, MissionFlightSource, MissionOrigin,
    MissionState, SectionCreate,
};
use crate::error::AppError;
use crate::map::geojson::{self, Track};
use crate::map::{self, MapChange, NewElement};
use crate::store::logbook::{self, MissionEntry, NewEntry, StoredEntry};
use crate::store::missions::{FlugmappeIds, FlugmappeStart, LinkOp, LinkWrite, NewLink, SectionOp};
use crate::store::{flights, map_elements, missions as store, photos, remote_controllers};

use super::{MissionHub, rules};

/// Sicht der Flugmappe `caller` auf einen Einsatz: alle Verknüpfungen samt UUID, Abschnitts-IDs
/// nur ihre eigenen. Ohne `caller` (Auswahlliste ohne Header) keine Abschnitts-IDs.
pub fn fm_mission(m: Mission, ids: &FlugmappeIds, caller: Option<&str>) -> FmMission {
    let own = caller.and_then(|c| ids.link(c)).map(|l| l.id);
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
                flugmappe_section_id: own.and_then(|l| ids.of_section(l, s.id)).map(Into::into),
                id: s.id,
                label: s.label,
                location: s.location,
                created_at_ms: s.created_at_ms,
                has_map_photo: s.has_map_photo,
            })
            .collect(),
        links: ids
            .links
            .iter()
            .map(|l| FmLinkInfo {
                flugmappe_mission_id: l.flugmappe_mission_id.clone(),
                aircraft_label: l.aircraft_label.clone(),
                linked_by: l.linked_by.clone(),
                linked_at_ms: l.linked_at_ms,
                done_at_ms: l.done_at_ms,
                released_at_ms: l.released_at_ms,
            })
            .collect(),
    }
}

/// Zustand der Verknüpfung der Flugmappe `caller` an diesem Einsatz.
fn own(ids: &FlugmappeIds, caller: &str) -> rules::Own {
    match ids.link(caller) {
        None => rules::Own::None,
        Some(l) if l.released_at_ms.is_some() => rules::Own::Released,
        Some(_) => rules::Own::Active,
    }
}

/// Ein gespeicherter Eintrag in der Sicht seiner Flugmappe.
fn stored_view(e: StoredEntry, ids: &FlugmappeIds) -> FmLogbookStored {
    let entry = (!e.deleted).then(|| FmLogbookEntry {
        revision: e.revision,
        flight_id: e.flight_id,
        flugmappe_section_id: e
            .section_id
            .and_then(|s| ids.of_section(e.link_id, s))
            .map(Into::into),
        started_at_ms: e.started_at_ms.unwrap_or_default(),
        landed_at_ms: e.landed_at_ms,
        aircraft_label: e.aircraft_label.clone().unwrap_or_default(),
        pilot: e.pilot.clone(),
        landing_status: e.landing_status.clone(),
        changed_at_ms: e.changed_at_ms.unwrap_or_default(),
    });
    FmLogbookStored {
        entry_id: e.entry_id,
        revision: e.revision,
        entry,
    }
}

/// Flugliste eines Einsatzes: Telemetrie-Flüge (bestätigt mit dem Eintrag der Flugmappe) und
/// Handeinträge, nach Start. `rc_labels`: Bezeichnung je Fernsteuerung.
pub fn flight_log(
    mission: &Mission,
    telemetry: Vec<(crate::contract::FlightSummary, Option<i64>)>,
    entries: Vec<MissionEntry>,
    rc_labels: &HashMap<String, String>,
) -> Vec<MissionFlight> {
    let section = |id: Option<i64>| {
        id.and_then(|id| mission.sections.iter().find(|s| s.id == id))
            .map(|s| s.label.clone())
    };
    let confirmed: HashMap<i64, &MissionEntry> = entries
        .iter()
        .filter_map(|e| e.entry.flight_id.map(|f| (f, e)))
        .collect();
    let mut out: Vec<MissionFlight> = telemetry
        .into_iter()
        .map(|(f, section_id)| {
            let entry = confirmed.get(&f.id);
            MissionFlight {
                key: format!("t{}", f.id),
                flight_id: Some(f.id),
                source: MissionFlightSource::Telemetry,
                started_at_ms: f.started_at_ms,
                ended_at_ms: f.ended_at_ms,
                aircraft: entry
                    .and_then(|e| e.entry.aircraft_label.clone())
                    .or_else(|| rc_labels.get(&f.gateway_sn).cloned())
                    .unwrap_or(f.aircraft_sn),
                section: section(section_id),
                confirmed_in: entry.map(|e| e.link_aircraft_label.clone()),
                pilot: entry.and_then(|e| e.entry.pilot.clone()),
            }
        })
        .collect();
    out.extend(
        entries
            .iter()
            .filter(|e| e.entry.flight_id.is_none())
            .map(|e| MissionFlight {
                key: format!("f{}", e.entry.id),
                flight_id: None,
                source: MissionFlightSource::Flugmappe,
                started_at_ms: e.entry.started_at_ms.unwrap_or_default(),
                ended_at_ms: e.entry.landed_at_ms,
                aircraft: e.entry.aircraft_label.clone().unwrap_or_default(),
                section: section(e.entry.section_id),
                confirmed_in: None,
                pilot: e.entry.pilot.clone(),
            }),
    );
    // Gleiche Startzeit: Telemetrie zuerst.
    out.sort_by(|a, b| {
        (a.started_at_ms, a.flight_id.is_none(), &a.key).cmp(&(
            b.started_at_ms,
            b.flight_id.is_none(),
            &b.key,
        ))
    });
    out
}

fn validation(e: String) -> AppError {
    AppError::Validation(e)
}

impl MissionHub {
    async fn fm_view(&self, mission: Mission, caller: Option<&str>) -> Result<FmMission, AppError> {
        let ids = store::flugmappe_ids(&self.pool, mission.id).await?;
        Ok(fm_mission(mission, &ids, caller))
    }

    /// Einsatz, an dem die aufrufende Flugmappe arbeiten darf, mit der ID ihrer Verknüpfung (404,
    /// sonst 409 ohne oder mit gelöster Verknüpfung).
    async fn bound(&self, id: i64, caller: &str) -> Result<(Mission, FlugmappeIds, i64), AppError> {
        let mission = self.get(id).await?;
        let ids = store::flugmappe_ids(&self.pool, id).await?;
        rules::binding(own(&ids, caller))?;
        let link = ids.link(caller).expect("gebunden").id;
        Ok((mission, ids, link))
    }

    /// Wie `bound`, aber nur für einen offenen Einsatz (422, wenn abgeschlossen).
    async fn bound_open(
        &self,
        id: i64,
        caller: &str,
    ) -> Result<(Mission, FlugmappeIds, i64), AppError> {
        let bound = self.bound(id, caller).await?;
        if bound.0.state == MissionState::Closed {
            return Err(AppError::Unprocessable(rules::CLOSED.into()));
        }
        Ok(bound)
    }

    /// Vorbereitete und laufende Einsätze zur Auswahl in der Flugmappe; mit `caller` samt ihren
    /// Abschnitts-IDs.
    pub async fn flugmappe_missions(
        &self,
        caller: Option<&str>,
    ) -> Result<Vec<FmMission>, AppError> {
        let mut out = Vec::new();
        for m in self.list().await? {
            if m.state != MissionState::Closed {
                out.push(self.fm_view(m, caller).await?);
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
            aircraft_label: rules::aircraft_label(&input.aircraft_label).map_err(validation)?,
        };
        let caller = start.flugmappe_mission_id.as_str();
        let _serial = self.writes.lock().await;
        if let Some(existing) = store::find_by_flugmappe(&self.pool, caller).await? {
            let mission = self.get(existing).await?;
            let ids = store::flugmappe_ids(&self.pool, existing).await?;
            // Angelegt hat sie ihn, wenn ihre Verknüpfung die erste ist und aus der Anlage stammt.
            let first = ids.links.first();
            rules::create_again(rules::Existing {
                state: mission.state,
                created_by_caller: mission.origin == MissionOrigin::Flugmappe
                    && first.is_some_and(|l| {
                        l.flugmappe_mission_id == caller && l.linked_at_ms == mission.created_at_ms
                    }),
            })?;
            return Ok(fm_mission(mission, &ids, Some(caller)));
        }
        if store::running_id(&self.pool).await?.is_some() {
            return Err(AppError::Conflict(rules::OTHER_RUNNING.into()));
        }
        let id = store::create_from_flugmappe(&self.pool, &start, now_ms).await?;
        self.map.set_pilot_scope(Some(id));
        self.changed();
        self.fm_view(self.get(id).await?, Some(caller)).await
    }

    /// Verknüpft einen vorbereiteten (dann läuft er) oder laufenden Einsatz, auch wenn schon
    /// andere Flugmappen daran arbeiten. Der aktive Abschnitt wird der der Flugmappe, wenn er
    /// keiner gehört; sonst bekommt sie einen neuen.
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
        let section_input = SectionCreate {
            label: rules::name(&input.section_label, "Bezeichnung").map_err(validation)?,
            location: rules::location(input.section_location),
        };
        let aircraft_label = rules::aircraft_label(&input.aircraft_label).map_err(validation)?;
        let _serial = self.writes.lock().await;
        let mission = self.get(id).await?;
        let ids = store::flugmappe_ids(&self.pool, id).await?;
        let running = store::running_id(&self.pool).await?;
        let plan = rules::link(rules::LinkFacts {
            state: mission.state,
            own: own(&ids, &caller),
            other_running: running.is_some_and(|r| r != id),
            caller_linked_elsewhere: store::find_by_flugmappe(&self.pool, &caller)
                .await?
                .is_some_and(|other| other != id),
        })?;
        let own_link = ids.link(&caller).map(|l| l.id);
        let facts: Vec<rules::SectionFact> = ids
            .sections
            .iter()
            .map(|s| rules::SectionFact {
                id: s.id,
                owned: s.link_id.is_some(),
                own_id: own_link.and_then(|l| ids.of_section(l, s.id)),
            })
            .collect();
        let section_op = match rules::section_for_link(&facts, &section) {
            rules::SectionStep::Known(_) => SectionOp::Keep,
            rules::SectionStep::Bind(section_id) => SectionOp::Bind {
                section_id,
                flugmappe_section_id: &section,
                input: &section_input,
            },
            rules::SectionStep::Create => SectionOp::Create {
                flugmappe_section_id: &section,
                input: &section_input,
            },
        };
        let link_op = match (plan.write, own_link) {
            (rules::LinkWriteKind::Keep, Some(l)) => LinkOp::Keep(l),
            (rules::LinkWriteKind::Revive, Some(l)) => LinkOp::Revive {
                id: l,
                aircraft_label: &aircraft_label,
            },
            _ => LinkOp::Insert(NewLink {
                flugmappe_mission_id: &caller,
                linked_by,
                aircraft_label: &aircraft_label,
            }),
        };
        let nothing_to_do = !plan.start
            && matches!(link_op, LinkOp::Keep(_))
            && matches!(section_op, SectionOp::Keep);
        if nothing_to_do {
            return Ok(fm_mission(mission, &ids, Some(&caller)));
        }
        // Starten, Verknüpfen und Abschnitt in einer Transaktion. Ein zweiter laufender Einsatz
        // scheitert am Teilindex (409), dann bleibt auch die Verknüpfung ungeschrieben.
        let write = LinkWrite {
            start: plan.start,
            link: link_op,
            section: section_op,
        };
        store::link_flugmappe(&self.pool, id, &write, now_ms).await?;
        if plan.start {
            self.map
                .set_pilot_scope(store::running_id(&self.pool).await?);
        }
        self.changed();
        self.fm_view(self.get(id).await?, Some(&caller)).await
    }

    /// Verlegung in der Flugmappe: neuer aktiver Abschnitt des Einsatzes und der Flugmappe. Eine
    /// bekannte ID liefert den Einsatz unverändert (Wiederholung).
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
        let (mission, ids, link) = self.bound_open(id, caller).await?;
        if ids.section(link, &section).is_some() {
            return Ok(fm_mission(mission, &ids, Some(caller)));
        }
        store::add_section_from_flugmappe(&self.pool, id, link, &section, &input, now_ms).await?;
        self.changed();
        self.fm_view(self.get(id).await?, Some(caller)).await
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
        let (mission, ids, link) = self.bound_open(id, caller).await?;
        let section = ids
            .section(link, flugmappe_section_id)
            .ok_or(AppError::NotFound)?;
        let unchanged = mission
            .sections
            .iter()
            .any(|s| s.id == section && s.label == input.label && s.location == input.location);
        if unchanged {
            return Ok(fm_mission(mission, &ids, Some(caller)));
        }
        store::update_section(&self.pool, section, &input).await?;
        self.changed();
        self.fm_view(self.get(id).await?, Some(caller)).await
    }

    /// Telemetrie-Flüge des Einsatzes als Vorschläge für das Flugbuch, älteste zuerst; mit der
    /// Flugmappe, die einen schon bestätigt hat, wenn es eine andere ist.
    pub async fn flugmappe_flights(
        &self,
        id: i64,
        caller: &str,
    ) -> Result<Vec<FmFlight>, AppError> {
        let (_, ids, link) = self.bound(id, caller).await?;
        let shots: HashMap<i64, u32> = photos::shots_per_flight(&self.pool, id)
            .await?
            .into_iter()
            .collect();
        let labels: HashMap<String, String> = remote_controllers::labels(&self.pool)
            .await?
            .into_iter()
            .collect();
        let confirmed_by: HashMap<i64, String> = logbook::of_mission(&self.pool, id)
            .await?
            .into_iter()
            .filter(|e| e.entry.link_id != link)
            .filter_map(|e| e.entry.flight_id.map(|f| (f, e.link_aircraft_label)))
            .collect();
        Ok(flights::of_mission_with_section(&self.pool, id)
            .await?
            .into_iter()
            .map(|(f, section)| FmFlight {
                flugmappe_section_id: section
                    .and_then(|s| ids.of_section(link, s))
                    .map(Into::into),
                aircraft_label: labels.get(&f.gateway_sn).cloned(),
                photo_count: shots.get(&f.id).copied().unwrap_or(0),
                confirmed_by: confirmed_by.get(&f.id).cloned(),
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

    /// Meldet einen Flugbucheintrag der Flugmappe (bestätigt oder von Hand). Nur eine höhere
    /// Revision ändert etwas; sonst kommt der gespeicherte Stand zurück. Ein Flug, den eine
    /// andere Flugmappe bestätigt hat, ist ein Konflikt (409).
    pub async fn put_logbook_entry(
        &self,
        id: i64,
        caller: &str,
        entry_id: &str,
        input: FmLogbookEntry,
        now_ms: i64,
    ) -> Result<FmLogbookStored, AppError> {
        let entry_id = rules::logbook_entry_id(entry_id).map_err(validation)?;
        let input = rules::logbook_entry(input).map_err(validation)?;
        let _serial = self.writes.lock().await;
        let (_, ids, link) = self.bound_open(id, caller).await?;
        let stored = logbook::get(&self.pool, link, &entry_id).await?;
        if let Some(s) = stored.filter(|s| !rules::newer(Some(s.revision), input.revision)) {
            return Ok(stored_view(s, &ids));
        }
        let section_id = match input.flugmappe_section_id.as_deref() {
            Some(fm) => Some(
                ids.section(link, fm)
                    .ok_or_else(|| AppError::Unprocessable(rules::SECTION_UNKNOWN.into()))?,
            ),
            None => None,
        };
        if let Some(flight) = input.flight_id {
            if flights::mission_of(&self.pool, flight).await? != Some(Some(id)) {
                return Err(AppError::Unprocessable(rules::FLIGHT_NOT_IN_MISSION.into()));
            }
            let holder = logbook::holder(&self.pool, flight).await?;
            rules::confirmation(
                holder.as_ref().map(|(link_id, label)| rules::Holder {
                    link_id: *link_id,
                    aircraft_label: label,
                }),
                link,
            )?;
        }
        let entry = NewEntry {
            revision: input.revision,
            flight_id: input.flight_id,
            section_id,
            started_at_ms: input.started_at_ms,
            landed_at_ms: input.landed_at_ms,
            aircraft_label: input.aircraft_label,
            pilot: input.pilot,
            landing_status: input.landing_status,
            changed_at_ms: input.changed_at_ms,
        };
        logbook::put(&self.pool, link, &entry_id, &entry, now_ms).await?;
        self.changed();
        let stored = logbook::get(&self.pool, link, &entry_id)
            .await?
            .expect("eben geschrieben");
        Ok(stored_view(stored, &ids))
    }

    /// Löscht einen Flugbucheintrag der Flugmappe: Mit höherer Revision bleibt eine Löschmarke
    /// (eine Bestätigung entfällt, der Telemetrie-Flug bleibt); sonst ändert sich nichts.
    pub async fn delete_logbook_entry(
        &self,
        id: i64,
        caller: &str,
        entry_id: &str,
        revision: i64,
        now_ms: i64,
    ) -> Result<(), AppError> {
        let entry_id = rules::logbook_entry_id(entry_id).map_err(validation)?;
        let revision = rules::revision(revision).map_err(validation)?;
        let _serial = self.writes.lock().await;
        let (_, _, link) = self.bound_open(id, caller).await?;
        let stored = logbook::get(&self.pool, link, &entry_id).await?;
        if rules::newer(stored.map(|s| s.revision), revision) {
            logbook::tombstone(&self.pool, link, &entry_id, revision, now_ms).await?;
            self.changed();
        }
        Ok(())
    }

    /// Einsatzkarte: alle Kartenelemente, Flugspuren mit `section` nur aus diesem Abschnitt der
    /// Flugmappe (unbekannt → 404).
    pub async fn flugmappe_map(
        &self,
        id: i64,
        caller: &str,
        section: Option<&str>,
    ) -> Result<FmFeatureCollection, AppError> {
        let (_, ids, link) = self.bound(id, caller).await?;
        let only = match section {
            Some(fm) => Some(ids.section(link, fm).ok_or(AppError::NotFound)?),
            None => None,
        };
        let elements = self.map.list_in(Some(id)).await?;
        let mut tracks = Vec::new();
        for (flight, section) in flights::of_mission_with_section(&self.pool, id).await? {
            if only.is_some_and(|o| section != Some(o)) {
                continue;
            }
            tracks.push(Track {
                flugmappe_section_id: section
                    .and_then(|s| ids.of_section(link, s))
                    .map(Into::into),
                segments: flights::segments(&self.pool, flight.id).await?,
                flight,
            });
        }
        Ok(geojson::feature_collection(&elements, &tracks))
    }

    /// Übernimmt Zeichnungen und Standort der Flugmappe einmal je Einsatz; danach ein No-op,
    /// auch für eine später dazukommende Flugmappe.
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
        let (mission, ids, link) = self.bound_open(id, caller).await?;
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
        // Der Standort setzt den Ort des aktiven Abschnitts dieser Flugmappe.
        let active = ids
            .active_section(link)
            .or_else(|| mission.sections.last().map(|s| s.id));
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
        if let (Some(p), Some(active)) = (input.standort, active) {
            store::set_section_location_if_empty(&mut *tx, active, &rules::standort_location(p))
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
        let (_, ids, link) = self.bound_open(id, caller).await?;
        let section = ids
            .section(link, flugmappe_section_id)
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

    /// Abschluss aus der Flugmappe: meldet ihren Teil als fertig. Der Einsatz schließt erst, wenn
    /// keine nicht gelöste Verknüpfung mehr offen ist (und nichts mehr fliegt, sonst 422 und die
    /// Flugmappe holt den Abschluss nach). Schon abgeschlossen ist kein Fehler.
    pub async fn close_from_flugmappe(
        &self,
        id: i64,
        caller: &str,
        now_ms: i64,
    ) -> Result<FmMission, AppError> {
        let _serial = self.writes.lock().await;
        let (mission, ids, link) = self.bound(id, caller).await?;
        if mission.state == MissionState::Closed {
            return Ok(fm_mission(mission, &ids, Some(caller)));
        }
        let already_done = ids
            .links
            .iter()
            .any(|l| l.id == link && l.done_at_ms.is_some());
        if !already_done {
            store::mark_link_done(&self.pool, link, now_ms).await?;
            self.changed();
        }
        let progress: Vec<rules::LinkProgress> = store::links(&self.pool, id)
            .await?
            .iter()
            .map(|l| rules::LinkProgress {
                done: l.done_at_ms.is_some(),
                released: l.released_at_ms.is_some(),
            })
            .collect();
        let mission = if rules::all_done(&progress) {
            self.change_state_locked(id, MissionState::Closed, now_ms)
                .await?
        } else {
            self.get(id).await?
        };
        self.fm_view(mission, Some(caller)).await
    }

    /// Ein verknüpfter Einsatz für die Flugmappe (nur mit passender Bindung).
    pub async fn flugmappe_mission(&self, id: i64, caller: &str) -> Result<FmMission, AppError> {
        let (mission, ids, _) = self.bound(id, caller).await?;
        Ok(fm_mission(mission, &ids, Some(caller)))
    }

    /// Flugliste eines Einsatzes für den Browser: Telemetrie und Flugbücher der Flugmappen.
    pub async fn flight_log(&self, id: i64) -> Result<Vec<MissionFlight>, AppError> {
        let mission = self.get(id).await?;
        let labels: HashMap<String, String> = remote_controllers::labels(&self.pool)
            .await?
            .into_iter()
            .collect();
        let telemetry = flights::of_mission_with_section(&self.pool, id).await?;
        let entries = logbook::of_mission(&self.pool, id).await?;
        Ok(flight_log(&mission, telemetry, entries, &labels))
    }

    /// Löst eine Verknüpfung in Suchlage (verlorenes Gerät). Gemeldete Einträge bleiben.
    /// Wiederholt ist kein Fehler.
    pub async fn release_flugmappe(
        &self,
        id: i64,
        link_id: i64,
        now_ms: i64,
    ) -> Result<Mission, AppError> {
        let _serial = self.writes.lock().await;
        let mission = self.get(id).await?;
        let link = mission
            .flugmappe_links
            .iter()
            .find(|l| l.id == link_id)
            .ok_or(AppError::NotFound)?;
        if mission.state == MissionState::Closed {
            return Err(AppError::Unprocessable(rules::CLOSED.into()));
        }
        if !link.is_active() {
            return Ok(mission);
        }
        store::release_link(&self.pool, id, link_id, now_ms).await?;
        self.changed();
        self.get(id).await
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
    const E1: &str = "3c4d5e6f-7a8b-4c9d-8e0f-1a2b3c4d5e6f";
    const E2: &str = "4d5e6f7a-8b9c-4d0e-8f1a-2b3c4d5e6f7a";

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
            aircraft_label: " Matrice 350 RTK ".into(),
        }
    }

    fn link_input(fm: &str, section: &str) -> FmMissionLink {
        FmMissionLink {
            flugmappe_mission_id: fm.into(),
            flugmappe_section_id: section.into(),
            section_label: "Abschnitt A".into(),
            section_location: None,
            aircraft_label: "Matrice 200".into(),
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

    /// Laufender Einsatz aus Flugmappe A, dazu verknüpft B (mit eigenem Abschnitt „t1“).
    async fn two_flugmappen(hub: &MissionHub) -> i64 {
        let m = hub
            .create_from_flugmappe(create_input(FM_A), "Testpilot", 10)
            .await
            .unwrap();
        hub.link(m.id, link_input(FM_B, "t1"), "Zweite", 20)
            .await
            .unwrap();
        m.id
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
        let link = &m.links[0];
        assert_eq!(
            (
                link.flugmappe_mission_id.as_str(),
                link.linked_by.as_str(),
                link.aircraft_label.as_str()
            ),
            (FM_A, "Testpilot", "Matrice 350 RTK")
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
        for bad in [
            FmMissionCreate {
                flugmappe_mission_id: "keine-uuid".into(),
                ..create_input(FM_B)
            },
            FmMissionCreate {
                aircraft_label: " ".into(),
                ..create_input(FM_B)
            },
        ] {
            assert!(matches!(
                f.hub.create_from_flugmappe(bad, "x", 14).await,
                Err(AppError::Validation(_))
            ));
        }
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

        // Dazugekommen: Die UUID der zweiten gehört zu einem Einsatz, den sie nicht angelegt hat.
        f.hub
            .link(m.id, link_input(FM_B, "t1"), "Zweite", 20)
            .await
            .unwrap();
        let err = f
            .hub
            .create_from_flugmappe(create_input(FM_B), "Zweite", 21)
            .await
            .unwrap_err();
        assert_eq!(err.to_string(), rules::FLUGMAPPE_TAKEN);
    }

    #[tokio::test]
    async fn creating_with_the_uuid_of_a_linked_browser_mission_is_409() {
        let f = hub().await;
        let id = prepared(&f.hub, "Aus dem Browser").await;
        f.hub
            .link(id, link_input(FM_A, "s1"), "Testpilot", 5)
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
                .link(id, link_input(FM_A, "s1"), "Testpilot", 20)
                .await
                .is_err()
        );
        let m = f.hub.get(id).await.unwrap();
        assert_eq!(
            (m.state, m.flugmappe_links),
            (MissionState::Prepared, vec![]),
            "ganz oder gar nicht"
        );
        assert_eq!(f.hub.map.pilot_scope(), None);

        sqlx::query("DROP TRIGGER fail_bind")
            .execute(&f.hub.pool)
            .await
            .unwrap();
        let linked = f
            .hub
            .link(id, link_input(FM_A, "s1"), "Testpilot", 21)
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
            link: LinkOp::Insert(NewLink {
                flugmappe_mission_id: FM_A,
                linked_by: "Testpilot",
                aircraft_label: "M200",
            }),
            section: SectionOp::Keep,
        };
        let err: AppError = store::link_flugmappe(&f.hub.pool, b, &write, 3)
            .await
            .unwrap_err()
            .into();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        let m = f.hub.get(b).await.unwrap();
        assert_eq!(
            (m.state, m.flugmappe_links),
            (MissionState::Prepared, vec![])
        );
    }

    #[tokio::test]
    async fn a_second_flugmappe_joins_with_its_own_section() {
        let f = hub().await;
        let id = prepared(&f.hub, "Vorbereitet").await;
        let m = f
            .hub
            .link(id, link_input(FM_A, "s1"), "Testpilot", 20)
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
            (Some("s1"), "Abschnitt A"),
            "der Browser-Abschnitt wird der der Flugmappe"
        );
        // Wiederholt: nichts ändert sich, auch nicht wer verknüpft hat.
        let mut changes = f.hub.subscribe();
        let again = f
            .hub
            .link(id, link_input(FM_A, "s1"), "Jemand", 30)
            .await
            .unwrap();
        assert_eq!(again, m);
        assert!(changes.try_recv().is_err());
        assert_eq!(f.hub.get(id).await.unwrap().origin, MissionOrigin::Web);

        // Eine zweite kommt einfach dazu; der aktive Abschnitt gehört der ersten, also ein neuer.
        let joined = f
            .hub
            .link(id, link_input(FM_B, "s1"), "Zweite", 50)
            .await
            .unwrap();
        assert_eq!(joined.links.len(), 2);
        assert_eq!(joined.links[1].linked_by, "Zweite");
        let own: Vec<_> = joined
            .sections
            .iter()
            .map(|s| s.flugmappe_section_id.as_deref())
            .collect();
        assert_eq!(own, vec![None, Some("s1")], "nur ihre eigene ID");
        let for_a = f.hub.flugmappe_mission(id, FM_A).await.unwrap();
        let own_a: Vec<_> = for_a
            .sections
            .iter()
            .map(|s| s.flugmappe_section_id.as_deref())
            .collect();
        assert_eq!(own_a, vec![Some("s1"), None]);
        assert!(f.hub.flugmappe_flights(id, FM_A).await.is_ok());
        assert!(f.hub.flugmappe_flights(id, FM_B).await.is_ok());

        let list = f.hub.flugmappe_missions(None).await.unwrap();
        assert!(
            list[0]
                .sections
                .iter()
                .all(|s| s.flugmappe_section_id.is_none()),
            "ohne Header keine Abschnitts-IDs"
        );
        let list = f.hub.flugmappe_missions(Some(FM_B)).await.unwrap();
        assert_eq!(list[0].sections, joined.sections);
    }

    #[tokio::test]
    async fn linking_refuses_while_another_runs_or_when_closed() {
        let f = hub().await;
        let a = prepared(&f.hub, "A").await;
        let b = prepared(&f.hub, "B").await;
        f.hub.start(a, 2).await.unwrap();
        let err = f
            .hub
            .link(b, link_input(FM_A, "s1"), "x", 3)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)), "{err:?}");
        assert_eq!(
            f.hub.get(b).await.unwrap().flugmappe_links,
            vec![],
            "nichts geschrieben"
        );
        f.hub.close(b, 4).await.unwrap();
        let err = f
            .hub
            .link(b, link_input(FM_A, "s1"), "x", 5)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Unprocessable(_)), "{err:?}");
        // Laufend ohne Flugmappe: darf verknüpft werden.
        assert!(f.hub.link(a, link_input(FM_A, "s1"), "x", 6).await.is_ok());
        assert!(matches!(
            f.hub.link(999, link_input(FM_A, "s1"), "x", 7).await,
            Err(AppError::NotFound)
        ));
        let mut bad = link_input(FM_B, "s1");
        bad.aircraft_label = String::new();
        assert!(matches!(
            f.hub.link(a, bad, "x", 8).await,
            Err(AppError::Validation(_))
        ));
    }

    #[tokio::test]
    async fn a_released_flugmappe_is_409_until_it_links_again() {
        let f = hub().await;
        let id = two_flugmappen(&f.hub).await;
        let link_b = f.hub.get(id).await.unwrap().flugmappe_links[1].id;
        assert!(matches!(
            f.hub.release_flugmappe(id, 999, 30).await,
            Err(AppError::NotFound)
        ));
        let m = f.hub.release_flugmappe(id, link_b, 30).await.unwrap();
        assert_eq!(m.flugmappe_links[1].released_at_ms, Some(30));
        let again = f.hub.release_flugmappe(id, link_b, 31).await.unwrap();
        assert_eq!(again, m, "wiederholt kein Fehler");
        let err = f.hub.flugmappe_flights(id, FM_B).await.unwrap_err();
        assert_eq!(err.to_string(), rules::RELEASED);
        assert!(f.hub.flugmappe_flights(id, FM_A).await.is_ok());

        // Wiederverknüpfen belebt die alte Verknüpfung samt Abschnitt.
        let back = f
            .hub
            .link(id, link_input(FM_B, "t1"), "Zweite", 40)
            .await
            .unwrap();
        assert_eq!(back.links.len(), 2, "keine zweite Verknüpfung");
        assert_eq!(back.links[1].released_at_ms, None);
        assert_eq!(back.sections.len(), 2, "Abschnitt bekannt");
        assert!(f.hub.flugmappe_flights(id, FM_B).await.is_ok());

        f.hub.close(id, 50).await.unwrap();
        assert!(matches!(
            f.hub.release_flugmappe(id, link_b, 51).await,
            Err(AppError::Unprocessable(_))
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
    async fn the_same_local_section_id_of_two_flugmappen_does_not_collide() {
        let f = hub().await;
        let id = two_flugmappen(&f.hub).await;
        let input = |label: &str| FmSectionCreate {
            flugmappe_section_id: "s2".into(),
            label: label.into(),
            location: None,
        };
        f.hub
            .add_flugmappe_section(id, FM_A, input("Verlegung A"), 30)
            .await
            .unwrap();
        let b = f
            .hub
            .add_flugmappe_section(id, FM_B, input("Verlegung B"), 31)
            .await
            .unwrap();
        assert_eq!(b.sections.len(), 4);
        assert_eq!(
            b.sections.last().unwrap().label,
            "Verlegung B",
            "aktiv für Suchlage: der zuletzt angelegte"
        );
        let ids = store::flugmappe_ids(&f.hub.pool, id).await.unwrap();
        let (la, lb) = (ids.link(FM_A).unwrap().id, ids.link(FM_B).unwrap().id);
        assert_ne!(ids.section(la, "s2"), ids.section(lb, "s2"));
        assert_eq!(ids.active_section(la), ids.section(la, "s2"));
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
                    f.confirmed_by.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (1, Some("s1"), 2, Some("RC Plus 1"), None),
                (3, Some("s2"), 0, Some("RC Plus 1"), None)
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

    fn entry(revision: i64, flight_id: Option<i64>, section: Option<&str>) -> FmLogbookEntry {
        FmLogbookEntry {
            revision,
            flight_id,
            flugmappe_section_id: section.map(Into::into),
            started_at_ms: 1_000,
            landed_at_ms: Some(1_500),
            aircraft_label: "Matrice 200".into(),
            pilot: Some("Max Muster".into()),
            landing_status: Some("normal".into()),
            changed_at_ms: 1_600,
        }
    }

    #[tokio::test]
    async fn two_flugmappen_confirm_a_flight_only_once() {
        let f = hub().await;
        let id = two_flugmappen(&f.hub).await;
        flights::insert(&f.hub.pool, &flight(1, Some(1_500)))
            .await
            .unwrap();
        let confirmed = f
            .hub
            .put_logbook_entry(id, FM_B, E1, entry(1, Some(1), Some("t1")), 30)
            .await
            .unwrap();
        assert_eq!(
            (
                confirmed.revision,
                confirmed
                    .entry
                    .as_ref()
                    .unwrap()
                    .flugmappe_section_id
                    .as_deref()
            ),
            (1, Some("t1"))
        );
        // Suchlage übernimmt den Abschnitt der Bestätigung für den Flug.
        let section_b = f.hub.flugmappe_mission(id, FM_B).await.unwrap().sections[1].id;
        let (_, section) = flights::of_mission_with_section(&f.hub.pool, id)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(section, Some(section_b));

        let for_a = f.hub.flugmappe_flights(id, FM_A).await.unwrap();
        assert_eq!(for_a[0].confirmed_by.as_deref(), Some("Matrice 200"));
        assert_eq!(for_a[0].flugmappe_section_id, None, "Abschnitt der anderen");
        let for_b = f.hub.flugmappe_flights(id, FM_B).await.unwrap();
        assert_eq!(for_b[0].confirmed_by, None, "eigene Bestätigung");

        let err = f
            .hub
            .put_logbook_entry(id, FM_A, E2, entry(1, Some(1), None), 31)
            .await
            .unwrap_err();
        assert!(
            matches!(&err, AppError::Conflict(m) if m == "Flug wurde von Matrice 200 bestätigt"),
            "{err:?}"
        );

        // Gelöscht: Der Flug bleibt und ist wieder Vorschlag; A darf ihn jetzt bestätigen.
        f.hub
            .delete_logbook_entry(id, FM_B, E1, 2, 32)
            .await
            .unwrap();
        assert_eq!(flights::of_mission(&f.hub.pool, id).await.unwrap().len(), 1);
        assert_eq!(
            f.hub.flugmappe_flights(id, FM_A).await.unwrap()[0].confirmed_by,
            None
        );
        assert!(
            f.hub
                .put_logbook_entry(id, FM_A, E2, entry(1, Some(1), None), 33)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn logbook_reports_follow_the_revision() {
        let f = hub().await;
        let id = two_flugmappen(&f.hub).await;
        let put = |rev: i64, label: &str| {
            let mut e = entry(rev, None, Some("t1"));
            e.aircraft_label = label.into();
            f.hub.put_logbook_entry(id, FM_B, E1, e, 40)
        };
        assert_eq!(put(2, "Neu").await.unwrap().revision, 2);
        let older = put(1, "Alt").await.unwrap();
        assert_eq!(
            (older.revision, older.entry.unwrap().aircraft_label.as_str()),
            (2, "Neu"),
            "älter: der gespeicherte Stand, kein Fehler"
        );
        // Ein verspätetes DELETE löscht keinen neueren Eintrag.
        f.hub
            .delete_logbook_entry(id, FM_B, E1, 2, 41)
            .await
            .unwrap();
        assert!(put(2, "Neu").await.unwrap().entry.is_some());
        f.hub
            .delete_logbook_entry(id, FM_B, E1, 3, 42)
            .await
            .unwrap();
        f.hub
            .delete_logbook_entry(id, FM_B, E1, 3, 43)
            .await
            .unwrap();
        let gone = put(3, "Wieder").await.unwrap();
        assert_eq!(
            (gone.revision, gone.entry),
            (3, None),
            "nicht wiederhergestellt"
        );
        assert!(put(4, "Wieder").await.unwrap().entry.is_some());
        // Unbekannt gelöscht: eine Löschmarke, die eine ältere Meldung abfängt.
        f.hub
            .delete_logbook_entry(id, FM_B, E2, 5, 44)
            .await
            .unwrap();
        let late = f
            .hub
            .put_logbook_entry(id, FM_B, E2, entry(4, None, None), 45)
            .await
            .unwrap();
        assert_eq!(late.entry, None);

        let log = f.hub.flight_log(id).await.unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(
            (
                log[0].source,
                log[0].aircraft.as_str(),
                log[0].section.as_deref()
            ),
            (
                MissionFlightSource::Flugmappe,
                "Wieder",
                Some("Abschnitt A")
            )
        );
    }

    #[tokio::test]
    async fn logbook_reports_are_checked() {
        let f = hub().await;
        // Flug 9 ohne Einsatz: Er gehört nicht zum Einsatz und lässt sich nicht bestätigen.
        flights::insert(&f.hub.pool, &flight(9, Some(9_500)))
            .await
            .unwrap();
        let id = two_flugmappen(&f.hub).await;
        for (e, expected) in [
            (entry(1, Some(9), None), "Unprocessable"),
            (entry(1, Some(404), None), "Unprocessable"),
            (entry(1, None, Some("s9")), "Unprocessable"),
            (entry(1, None, Some("s1")), "Unprocessable"),
            (entry(0, None, None), "Validation"),
        ] {
            let err = f
                .hub
                .put_logbook_entry(id, FM_B, E1, e, 30)
                .await
                .unwrap_err();
            assert!(format!("{err:?}").starts_with(expected), "{err:?}");
        }
        assert!(matches!(
            f.hub
                .put_logbook_entry(id, FM_B, "e1", entry(1, None, None), 30)
                .await,
            Err(AppError::Validation(_))
        ));
        let stranger = "2b3c4d5e-6f7a-4b8c-9d0e-1f2a3b4c5d6e";
        assert!(matches!(
            f.hub
                .put_logbook_entry(id, stranger, E1, entry(1, None, None), 30)
                .await,
            Err(AppError::Conflict(_))
        ));
        // Nach dem Abschluss: 422, Suchlage zeigt den Eintrag nicht.
        f.hub.close(id, 31).await.unwrap();
        for result in [
            f.hub
                .put_logbook_entry(id, FM_A, E1, entry(1, None, None), 32)
                .await
                .map(|_| ()),
            f.hub.delete_logbook_entry(id, FM_A, E1, 1, 32).await,
        ] {
            assert!(
                matches!(result, Err(AppError::Unprocessable(_))),
                "{result:?}"
            );
        }
        assert!(
            f.hub
                .flight_log(id)
                .await
                .unwrap()
                .iter()
                .all(|e| e.flight_id.is_some())
        );
    }

    #[tokio::test]
    async fn the_flight_log_shows_telemetry_and_hand_entries() {
        let f = hub().await;
        let pool = &f.hub.pool;
        let id = two_flugmappen(&f.hub).await;
        let (rc, _) = remote_controllers::create(pool, "RC Plus 1", 0)
            .await
            .unwrap();
        remote_controllers::bind(pool, rc.id, "RC1", "pw", "k", 1)
            .await
            .unwrap();
        flights::insert(pool, &flight(1, Some(1_500)))
            .await
            .unwrap();
        flights::insert(pool, &flight(3, None)).await.unwrap();
        let mut confirmed = entry(1, Some(1), None);
        confirmed.aircraft_label = "Matrice 350 RTK".into();
        f.hub
            .put_logbook_entry(id, FM_A, E1, confirmed, 30)
            .await
            .unwrap();
        let mut hand = entry(1, None, Some("t1"));
        hand.started_at_ms = 2_000;
        hand.landed_at_ms = Some(2_500);
        f.hub
            .put_logbook_entry(id, FM_B, E2, hand, 31)
            .await
            .unwrap();
        let log = f.hub.flight_log(id).await.unwrap();
        let summary: Vec<_> = log
            .iter()
            .map(|e| {
                (
                    e.flight_id,
                    e.source,
                    e.aircraft.as_str(),
                    e.confirmed_in.as_deref(),
                    e.section.as_deref(),
                )
            })
            .collect();
        assert_eq!(
            summary,
            vec![
                (
                    Some(1),
                    MissionFlightSource::Telemetry,
                    "Matrice 350 RTK",
                    Some("Matrice 350 RTK"),
                    Some("Abschnitt A")
                ),
                (
                    None,
                    MissionFlightSource::Flugmappe,
                    "Matrice 200",
                    None,
                    Some("Abschnitt A")
                ),
                (
                    Some(3),
                    MissionFlightSource::Telemetry,
                    "RC Plus 1",
                    None,
                    Some("Abschnitt A")
                ),
            ]
        );
        assert!(matches!(
            f.hub.flight_log(999).await,
            Err(AppError::NotFound)
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
            .import_map(m.id, FM_A, input.clone(), "Testpilot", 12)
            .await
            .unwrap();
        assert_eq!(again.imported, 0);
        // Eine dazukommende Flugmappe übernimmt nichts mehr.
        f.hub
            .link(m.id, link_input(FM_B, "t1"), "Zweite", 13)
            .await
            .unwrap();
        let joined = f
            .hub
            .import_map(m.id, FM_B, input, "Zweite", 14)
            .await
            .unwrap();
        assert_eq!(joined.imported, 0);
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
        assert!(f.hub.flugmappe_missions(None).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_mission_closes_only_with_the_last_open_flugmappe() {
        let f = hub().await;
        let id = two_flugmappen(&f.hub).await;
        let first = f.hub.close_from_flugmappe(id, FM_A, 30).await.unwrap();
        assert_eq!(first.state, MissionState::Running, "B fliegt noch");
        assert_eq!(first.links[0].done_at_ms, Some(30));
        let again = f.hub.close_from_flugmappe(id, FM_A, 31).await.unwrap();
        assert_eq!(again.links[0].done_at_ms, Some(30), "erste Zeit bleibt");
        // Nach „fertig“ nimmt Suchlage noch Einträge an, solange der Einsatz offen ist.
        assert!(
            f.hub
                .put_logbook_entry(id, FM_A, E1, entry(1, None, None), 32)
                .await
                .is_ok()
        );
        let last = f.hub.close_from_flugmappe(id, FM_B, 33).await.unwrap();
        assert_eq!(last.state, MissionState::Closed);

        // Eine gelöste Flugmappe wartet niemand ab.
        let g = hub().await;
        let id = two_flugmappen(&g.hub).await;
        let link_b = g.hub.get(id).await.unwrap().flugmappe_links[1].id;
        g.hub.release_flugmappe(id, link_b, 40).await.unwrap();
        let closed = g.hub.close_from_flugmappe(id, FM_A, 41).await.unwrap();
        assert_eq!(closed.state, MissionState::Closed);
        assert!(matches!(
            g.hub.close_from_flugmappe(id, FM_B, 42).await,
            Err(AppError::Conflict(_))
        ));
    }
}

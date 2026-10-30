//! Prüfregeln für Einsätze und ihre Verknüpfung mit der Flugmappe. Rein: kein Speicher, keine
//! Laufzeit.

use std::borrow::Cow;

use crate::contract::{
    FmImportFeature, FmLogbookEntry, FmMapImport, GeoPoint, MAP_COLORS, MapElementKind, MapPoint,
    MissionState,
};
use crate::map::rules as map_rules;

pub const MAX_NAME_CHARS: usize = 120;

/// Name eines Einsatzes oder Abschnitts: getrimmt, 1 bis 120 Zeichen (wie bei Kartenelementen).
pub fn name(raw: &str, field: &str) -> Result<String, String> {
    let name = raw.trim();
    let chars = name.chars().count();
    if chars == 0 || chars > MAX_NAME_CHARS {
        return Err(format!("{field}: 1 bis {MAX_NAME_CHARS} Zeichen"));
    }
    Ok(name.to_string())
}

/// Nur vorwärts: vorbereitet → laufend → abgeschlossen; ein vorbereiteter Einsatz darf entfallen.
pub fn transition(from: MissionState, to: MissionState) -> Result<(), String> {
    use MissionState::*;
    match (from, to) {
        (Prepared, Running) | (Prepared, Closed) | (Running, Closed) => Ok(()),
        _ => Err(format!(
            "Einsatz kann nicht von „{}“ nach „{}“ wechseln",
            label(from),
            label(to)
        )),
    }
}

/// Warum die Flugmappe etwas nicht darf. Die Abbildung auf Statuscodes macht der Hub.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// 409
    Conflict(Cow<'static, str>),
    /// 422
    Unprocessable(Cow<'static, str>),
}

impl Refusal {
    fn conflict(m: &'static str) -> Self {
        Refusal::Conflict(Cow::Borrowed(m))
    }
    fn unprocessable(m: &'static str) -> Self {
        Refusal::Unprocessable(Cow::Borrowed(m))
    }
}

pub const CLOSED: &str = "Der Einsatz ist abgeschlossen";
pub const OTHER_RUNNING: &str = "Es läuft bereits ein anderer Einsatz";
pub const NOT_LINKED: &str = "Der Einsatz ist nicht mit dieser Flugmappe verknüpft";
pub const RELEASED: &str = "Verknüpfung in Suchlage gelöst";
pub const FLIGHT_NOT_IN_MISSION: &str = "Der Flug gehört nicht zu diesem Einsatz";
pub const SECTION_UNKNOWN: &str = "Abschnitt der Flugmappe unbekannt";
pub const FLUGMAPPE_TAKEN: &str = "Diese Flugmappe ist schon mit einem anderen Einsatz verknüpft";
pub const SECTIONS_FROM_FLUGMAPPE: &str = "Abschnitte kommen bei diesem Einsatz aus der Flugmappe";

/// So viele Zeichnungen übernimmt Suchlage höchstens aus einer Flugmappe.
pub const MAX_IMPORT_FEATURES: usize = 200;
/// Zeichnungen aus der Flugmappe sind Suchgebiete: blau. Der Standort ist rot.
pub const IMPORT_COLOR: &str = MAP_COLORS[0];
pub const STANDORT_COLOR: &str = MAP_COLORS[3];
pub const STANDORT_NAME: &str = "Standort";

/// Einsatz-ID der Flugmappe: eine UUID, klein geschrieben gespeichert und verglichen.
pub fn flugmappe_mission_id(raw: &str) -> Result<String, String> {
    uuid::Uuid::try_parse(raw.trim())
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| "Einsatz-ID der Flugmappe muss eine UUID sein".into())
}

/// Abschnitts-ID der Flugmappe: steht im Pfad, also nur Zeichen, die dort ohne Kodierung gehen.
pub fn flugmappe_section_id(raw: &str) -> Result<String, String> {
    let id = raw.trim();
    let ok = (1..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then(|| id.to_string())
        .ok_or_else(|| "Abschnitts-ID der Flugmappe: 1 bis 64 Zeichen aus A–Z, 0–9, - und _".into())
}

/// Freitext-Ort: ohne Rand, leer heißt keiner.
pub fn location(raw: Option<String>) -> Option<String> {
    raw.map(|l| l.trim().to_string()).filter(|l| !l.is_empty())
}

/// Zustand der Verknüpfung der aufrufenden Flugmappe mit einem Einsatz.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Own {
    /// Keine Verknüpfung mit dieser UUID an diesem Einsatz.
    None,
    Active,
    Released,
}

/// Jede verknüpfte, nicht gelöste Flugmappe darf am Einsatz lesen und schreiben; eine gelöste
/// bekommt nichts mehr, bis sie neu verknüpft.
///
/// Die Bindung schützt vor veralteten Geräten (eine gelöste Flugmappe schreibt nicht weiter), nicht
/// vor böswilligen Konten: Die UUID ist kein Geheimnis, und jedes Konto ist `operator` oder
/// `admin` und darf dasselbe ohnehin im Browser.
pub fn binding(own: Own) -> Result<(), Refusal> {
    match own {
        Own::Active => Ok(()),
        Own::Released => Err(Refusal::conflict(RELEASED)),
        Own::None => Err(Refusal::conflict(NOT_LINKED)),
    }
}

/// Ein Einsatz, an den die UUID beim Anlegen schon (nicht gelöst) gebunden ist.
#[derive(Debug, Clone, Copy)]
pub struct Existing {
    pub state: MissionState,
    /// Diese Flugmappe hat ihn mit dieser UUID angelegt: Herkunft Flugmappe und ihre Verknüpfung
    /// stammt aus der Anlage (Verknüpfen setzt eine spätere Zeit).
    pub created_by_caller: bool,
}

/// Anlegen mit einer UUID, die schon gebunden ist. Hat diese Flugmappe den Einsatz selbst angelegt
/// und ist er offen, war es eine Wiederholung (Antwort verloren): Ergebnis ist der vorhandene
/// (200). Ist er abgeschlossen, ist das ein Übergang, den es nicht mehr gibt (422) – kein Konflikt
/// mit jemand anderem. Ist die UUID an einen fremden oder verknüpften Einsatz gebunden, 409.
pub fn create_again(e: Existing) -> Result<(), Refusal> {
    match (e.created_by_caller, e.state) {
        (true, MissionState::Closed) => Err(Refusal::unprocessable(CLOSED)),
        (true, _) => Ok(()),
        (false, _) => Err(Refusal::conflict(FLUGMAPPE_TAKEN)),
    }
}

/// Was die Flugmappe über den Einsatz weiß, wenn sie verknüpfen will.
#[derive(Debug, Clone, Copy)]
pub struct LinkFacts {
    pub state: MissionState,
    /// Ihre Verknüpfung mit diesem Einsatz. Andere Flugmappen am Einsatz spielen keine Rolle.
    pub own: Own,
    /// Ein anderer Einsatz läuft.
    pub other_running: bool,
    /// Die Flugmappe ist mit einem anderen Einsatz verknüpft (nicht gelöst).
    pub caller_linked_elsewhere: bool,
}

/// Was mit der Verknüpfung geschieht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkWriteKind {
    /// Besteht schon so (Wiederholung).
    Keep,
    Insert,
    /// Die gelöste Verknüpfung lebt wieder auf; Abschnitte und Einträge bleiben ihr.
    Revive,
}

/// Was beim Verknüpfen zu tun ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkPlan {
    /// vorbereitet → laufend
    pub start: bool,
    pub write: LinkWriteKind,
}

/// Verknüpfen: vorbereitet oder laufend und kein anderer läuft. Andere Flugmappen am Einsatz
/// stören nicht; eine gelöste Verknüpfung blockiert kein neues Verknüpfen. Wiederholen ist harmlos.
pub fn link(f: LinkFacts) -> Result<LinkPlan, Refusal> {
    if f.state == MissionState::Closed {
        return Err(Refusal::unprocessable(CLOSED));
    }
    if f.caller_linked_elsewhere {
        return Err(Refusal::conflict(FLUGMAPPE_TAKEN));
    }
    let start = f.state == MissionState::Prepared;
    if start && f.other_running {
        return Err(Refusal::conflict(OTHER_RUNNING));
    }
    let write = match f.own {
        Own::Active => LinkWriteKind::Keep,
        Own::Released => LinkWriteKind::Revive,
        Own::None => LinkWriteKind::Insert,
    };
    Ok(LinkPlan { start, write })
}

/// Ein Abschnitt des Einsatzes, wie ihn das Verknüpfen sieht.
#[derive(Debug, Clone, Copy)]
pub struct SectionFact<'a> {
    pub id: i64,
    /// Gehört einer Flugmappe (angelegt oder beim Verknüpfen übernommen).
    pub owned: bool,
    /// Abschnitts-ID der aufrufenden Flugmappe.
    pub own_id: Option<&'a str>,
}

/// Welcher Abschnitt beim Verknüpfen der der Flugmappe wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionStep {
    /// Den gibt es für diese Flugmappe schon mit dieser ID.
    Known(i64),
    /// Der aktive (letzte) Abschnitt gehört keiner Flugmappe: Er bekommt die ID.
    Bind(i64),
    /// Der aktive gehört einer anderen Flugmappe: ein neuer, der dann der aktive ist.
    Create,
}

/// `sections` in Anlegereihenfolge, mindestens einer.
pub fn section_for_link(sections: &[SectionFact], caller_section: &str) -> SectionStep {
    if let Some(s) = sections.iter().find(|s| s.own_id == Some(caller_section)) {
        return SectionStep::Known(s.id);
    }
    let active = sections.last().expect("Einsatz hat einen Abschnitt");
    if active.owned {
        SectionStep::Create
    } else {
        SectionStep::Bind(active.id)
    }
}

/// Eine Verknüpfung für den Abschluss: fertig gemeldet, gelöst.
#[derive(Debug, Clone, Copy)]
pub struct LinkProgress {
    pub done: bool,
    pub released: bool,
}

/// Der Einsatz schließt aus der Flugmappe erst, wenn keine nicht gelöste Verknüpfung mehr offen
/// ist. Ohne nicht gelöste bleibt er offen (geschlossen wird dann in Suchlage).
pub fn all_done(links: &[LinkProgress]) -> bool {
    let mut active = links.iter().filter(|l| !l.released).peekable();
    active.peek().is_some() && active.all(|l| l.done)
}

/// Wer einen Telemetrie-Flug schon im Flugbuch hat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Holder<'a> {
    pub link_id: i64,
    pub aircraft_label: &'a str,
}

/// Ein Flug steht in höchstens einem Flugbuch: Hat ihn eine andere Flugmappe schon bestätigt, 409.
pub fn confirmation(holder: Option<Holder>, caller_link: i64) -> Result<(), Refusal> {
    match holder {
        Some(h) if h.link_id != caller_link => Err(Refusal::Conflict(
            format!("Flug wurde von {} bestätigt", h.aircraft_label).into(),
        )),
        _ => Ok(()),
    }
}

/// Reihenfolge beim Nachholen: Suchlage übernimmt eine Meldung (`PUT` wie `DELETE`) nur mit
/// höherer Revision als gespeichert. Sonst bleibt der gespeicherte Stand, ohne Fehler.
pub fn newer(stored: Option<i64>, incoming: i64) -> bool {
    stored.is_none_or(|s| incoming > s)
}

/// Eintrags-ID der Flugmappe: eine UUID wie die Einsatz-ID.
pub fn logbook_entry_id(raw: &str) -> Result<String, String> {
    uuid::Uuid::try_parse(raw.trim())
        .map(|u| u.hyphenated().to_string())
        .map_err(|_| "Eintrags-ID der Flugmappe muss eine UUID sein".into())
}

pub fn revision(raw: i64) -> Result<i64, String> {
    if raw >= 1 {
        Ok(raw)
    } else {
        Err("Revision: ab 1".into())
    }
}

/// Fluggerät der Flugmappe: Pflicht, wie ein Name.
pub fn aircraft_label(raw: &str) -> Result<String, String> {
    name(raw, "Fluggerät")
}

fn optional_text(raw: Option<String>, field: &str) -> Result<Option<String>, String> {
    match location(raw) {
        Some(t) if t.chars().count() > MAX_NAME_CHARS => {
            Err(format!("{field}: höchstens {MAX_NAME_CHARS} Zeichen"))
        }
        t => Ok(t),
    }
}

/// Prüft einen Flugbucheintrag: Zeiten in Reihenfolge, Texte begrenzt; leere Texte heißen keiner.
pub fn logbook_entry(e: FmLogbookEntry) -> Result<FmLogbookEntry, String> {
    if e.landed_at_ms.is_some_and(|l| l < e.started_at_ms) {
        return Err("Landung liegt vor dem Start".into());
    }
    let section = match e.flugmappe_section_id.as_deref().map(str::trim) {
        Some(s) if !s.is_empty() => Some(flugmappe_section_id(s)?),
        _ => None,
    };
    Ok(FmLogbookEntry {
        revision: revision(e.revision)?,
        flugmappe_section_id: section,
        aircraft_label: aircraft_label(&e.aircraft_label)?,
        pilot: optional_text(e.pilot, "Fernpilot")?,
        landing_status: optional_text(e.landing_status, "Landestatus")?,
        ..e
    })
}

/// Ein Element, das aus der Flugmappe übernommen wird; geprüft wie eines aus dem Browser.
#[derive(Debug, Clone, PartialEq)]
pub struct Drawing {
    pub name: String,
    pub kind: MapElementKind,
    pub points: Vec<MapPoint>,
    pub color: &'static str,
}

fn points(raw: &[GeoPoint]) -> Vec<MapPoint> {
    raw.iter()
        .map(|p| MapPoint {
            lat: p.lat,
            lon: p.lon,
            alt_m: None,
        })
        .collect()
}

fn drawing(f: &FmImportFeature) -> Result<Drawing, String> {
    let name = match f.name.as_deref().map(str::trim) {
        Some(n) if !n.is_empty() => map_rules::name(n)?,
        _ => map_rules::default_name(f.kind).to_string(),
    };
    Ok(Drawing {
        name,
        kind: f.kind,
        points: map_rules::shape(f.kind, points(&f.points))?,
        color: IMPORT_COLOR,
    })
}

/// Prüft alle Zeichnungen, bevor eine angelegt wird: ein Fehler, nichts übernommen.
pub fn import(input: &FmMapImport) -> Result<Vec<Drawing>, String> {
    if input.features.len() > MAX_IMPORT_FEATURES {
        return Err(format!(
            "Höchstens {MAX_IMPORT_FEATURES} Zeichnungen auf einmal"
        ));
    }
    let mut out = input
        .features
        .iter()
        .enumerate()
        .map(|(i, f)| drawing(f).map_err(|e| format!("Zeichnung {}: {e}", i + 1)))
        .collect::<Result<Vec<_>, _>>()?;
    if let Some(p) = input.standort {
        out.push(Drawing {
            name: STANDORT_NAME.into(),
            kind: MapElementKind::Point,
            points: map_rules::shape(MapElementKind::Point, points(&[p]))
                .map_err(|e| format!("Standort: {e}"))?,
            color: STANDORT_COLOR,
        });
    }
    Ok(out)
}

/// Ort eines Abschnitts aus dem Standort, wenn er keinen hat.
pub fn standort_location(p: GeoPoint) -> String {
    format!("{:.5}, {:.5}", p.lat, p.lon)
}

/// Kartenfoto: nur JPEG, erkannt an den ersten Bytes (`FF D8 FF`), nicht am Header.
pub fn is_jpeg(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xFF, 0xD8, 0xFF])
}

fn label(state: MissionState) -> &'static str {
    match state {
        MissionState::Prepared => "vorbereitet",
        MissionState::Running => "laufend",
        MissionState::Closed => "abgeschlossen",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use MissionState::*;

    #[test]
    fn only_forward_transitions() {
        assert!(transition(Prepared, Running).is_ok());
        assert!(transition(Prepared, Closed).is_ok());
        assert!(transition(Running, Closed).is_ok());
        assert_eq!(
            transition(Closed, Running).unwrap_err(),
            "Einsatz kann nicht von „abgeschlossen“ nach „laufend“ wechseln"
        );
        assert!(transition(Running, Prepared).is_err());
        assert!(transition(Closed, Closed).is_err());
    }

    #[test]
    fn names_are_trimmed_and_bounded() {
        assert_eq!(name("  Wald ", "Name").unwrap(), "Wald");
        assert!(name("  ", "Name").is_err());
        assert!(name(&"x".repeat(121), "Name").is_err());
    }

    fn facts(state: MissionState, own: Own) -> LinkFacts {
        LinkFacts {
            state,
            own,
            other_running: false,
            caller_linked_elsewhere: false,
        }
    }

    #[test]
    fn linking_starts_a_prepared_mission_unless_another_runs() {
        assert_eq!(
            link(facts(Prepared, Own::None)),
            Ok(LinkPlan {
                start: true,
                write: LinkWriteKind::Insert
            })
        );
        assert_eq!(
            link(LinkFacts {
                other_running: true,
                ..facts(Prepared, Own::None)
            }),
            Err(Refusal::conflict(OTHER_RUNNING))
        );
        assert_eq!(
            link(facts(Running, Own::None)),
            Ok(LinkPlan {
                start: false,
                write: LinkWriteKind::Insert
            }),
            "ein laufender Einsatz darf verknüpft werden, auch wenn schon andere verknüpft sind"
        );
        assert_eq!(
            link(facts(Closed, Own::None)),
            Err(Refusal::unprocessable(CLOSED))
        );
        assert_eq!(
            link(facts(Closed, Own::Active)),
            Err(Refusal::unprocessable(CLOSED))
        );
    }

    #[test]
    fn linking_again_keeps_or_revives_the_own_link() {
        assert_eq!(
            link(facts(Running, Own::Active)),
            Ok(LinkPlan {
                start: false,
                write: LinkWriteKind::Keep
            }),
            "wiederholtes Verknüpfen ändert nichts"
        );
        assert_eq!(
            link(facts(Running, Own::Released)),
            Ok(LinkPlan {
                start: false,
                write: LinkWriteKind::Revive
            })
        );
        assert_eq!(
            link(LinkFacts {
                caller_linked_elsewhere: true,
                ..facts(Running, Own::None)
            }),
            Err(Refusal::conflict(FLUGMAPPE_TAKEN))
        );
    }

    #[test]
    fn creating_again_returns_only_an_open_mission_of_the_caller() {
        let mine = |state| Existing {
            state,
            created_by_caller: true,
        };
        assert_eq!(create_again(mine(Running)), Ok(()));
        assert_eq!(create_again(mine(Prepared)), Ok(()));
        assert_eq!(
            create_again(mine(Closed)),
            Err(Refusal::unprocessable(CLOSED))
        );
        assert_eq!(
            create_again(Existing {
                state: Running,
                created_by_caller: false,
            }),
            Err(Refusal::conflict(FLUGMAPPE_TAKEN)),
            "verknüpft, nicht angelegt"
        );
    }

    #[test]
    fn every_active_link_passes_the_binding() {
        assert_eq!(binding(Own::Active), Ok(()));
        assert_eq!(binding(Own::Released), Err(Refusal::conflict(RELEASED)));
        assert_eq!(binding(Own::None), Err(Refusal::conflict(NOT_LINKED)));
    }

    #[test]
    fn linking_binds_a_free_active_section_or_adds_one() {
        let s = |id, owned, own_id| SectionFact { id, owned, own_id };
        let free_last = [s(1, true, Some("s-alt")), s(2, false, None)];
        assert_eq!(section_for_link(&free_last, "s-neu"), SectionStep::Bind(2));
        assert_eq!(section_for_link(&free_last, "s-alt"), SectionStep::Known(1));
        let foreign_last = [s(1, false, None), s(2, true, None)];
        assert_eq!(
            section_for_link(&foreign_last, "s"),
            SectionStep::Create,
            "der aktive gehört einer anderen Flugmappe"
        );
        assert_eq!(
            section_for_link(&[s(5, false, None)], "s"),
            SectionStep::Bind(5)
        );
    }

    #[test]
    fn the_mission_closes_with_the_last_open_link() {
        let l = |done, released| LinkProgress { done, released };
        assert!(all_done(&[l(true, false)]));
        assert!(!all_done(&[l(true, false), l(false, false)]));
        assert!(
            all_done(&[l(true, false), l(false, true)]),
            "eine gelöste wartet niemand ab"
        );
        assert!(
            !all_done(&[l(false, true)]),
            "ohne aktive schließt Suchlage"
        );
        assert!(!all_done(&[]));
    }

    #[test]
    fn a_flight_is_confirmed_once() {
        let other = Holder {
            link_id: 2,
            aircraft_label: "Matrice 200",
        };
        assert_eq!(confirmation(None, 1), Ok(()));
        assert_eq!(
            confirmation(
                Some(Holder {
                    link_id: 1,
                    ..other
                }),
                1
            ),
            Ok(())
        );
        assert_eq!(
            confirmation(Some(other), 1),
            Err(Refusal::Conflict(
                "Flug wurde von Matrice 200 bestätigt".into()
            ))
        );
    }

    #[test]
    fn only_a_higher_revision_wins() {
        assert!(newer(None, 1));
        assert!(newer(Some(1), 2));
        assert!(!newer(Some(2), 2));
        assert!(!newer(Some(3), 2));
    }

    fn entry() -> FmLogbookEntry {
        FmLogbookEntry {
            revision: 1,
            flight_id: None,
            flugmappe_section_id: Some(" s1 ".into()),
            started_at_ms: 1_000,
            landed_at_ms: Some(2_000),
            aircraft_label: " Matrice 200 ".into(),
            pilot: Some(" ".into()),
            landing_status: Some("normal".into()),
            changed_at_ms: 3_000,
        }
    }

    #[test]
    fn logbook_entries_are_checked() {
        let ok = logbook_entry(entry()).unwrap();
        assert_eq!(
            (
                ok.flugmappe_section_id.as_deref(),
                ok.aircraft_label.as_str(),
                ok.pilot.as_deref()
            ),
            (Some("s1"), "Matrice 200", None)
        );
        for bad in [
            FmLogbookEntry {
                revision: 0,
                ..entry()
            },
            FmLogbookEntry {
                landed_at_ms: Some(999),
                ..entry()
            },
            FmLogbookEntry {
                aircraft_label: " ".into(),
                ..entry()
            },
            FmLogbookEntry {
                landing_status: Some("x".repeat(121)),
                ..entry()
            },
            FmLogbookEntry {
                flugmappe_section_id: Some("a/b".into()),
                ..entry()
            },
        ] {
            assert!(logbook_entry(bad.clone()).is_err(), "{bad:?}");
        }
        assert!(
            logbook_entry(FmLogbookEntry {
                landed_at_ms: None,
                flugmappe_section_id: Some("".into()),
                ..entry()
            })
            .is_ok(),
            "in der Luft, ohne Abschnitt"
        );
        assert!(logbook_entry_id("0F6B2A1E-5D43-4C8E-9B7A-2E1F3C4D5A6B").is_ok());
        assert!(logbook_entry_id("e1").is_err());
    }

    #[test]
    fn ids_of_the_flugmappe_are_checked() {
        assert_eq!(
            flugmappe_mission_id(" 0F6B2A1E-5D43-4C8E-9B7A-2E1F3C4D5A6B ").unwrap(),
            "0f6b2a1e-5d43-4c8e-9b7a-2e1f3c4d5a6b"
        );
        assert!(flugmappe_mission_id("fm-a").is_err());
        assert_eq!(flugmappe_section_id(" s_1-a ").unwrap(), "s_1-a");
        for bad in ["", "a/b", "ä", &"x".repeat(65)] {
            assert!(flugmappe_section_id(bad).is_err(), "{bad}");
        }
        assert_eq!(location(Some("  ".into())), None);
        assert_eq!(location(Some(" Wald ".into())).as_deref(), Some("Wald"));
    }

    fn feature(kind: MapElementKind, n: usize) -> FmImportFeature {
        FmImportFeature {
            name: None,
            kind,
            points: (0..n)
                .map(|i| GeoPoint {
                    lat: 52.0 + i as f64 * 0.001,
                    lon: 10.0,
                })
                .collect(),
        }
    }

    #[test]
    fn import_names_colors_and_checks_every_drawing() {
        let standort = GeoPoint {
            lat: 52.430_004,
            lon: 10.01,
        };
        let input = FmMapImport {
            features: vec![
                feature(MapElementKind::Polygon, 4),
                FmImportFeature {
                    name: Some(" Zufahrt ".into()),
                    ..feature(MapElementKind::Line, 2)
                },
            ],
            standort: Some(standort),
        };
        let drawings = import(&input).unwrap();
        let summary: Vec<_> = drawings
            .iter()
            .map(|d| (d.name.as_str(), d.kind, d.color))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("Fläche", MapElementKind::Polygon, IMPORT_COLOR),
                ("Zufahrt", MapElementKind::Line, IMPORT_COLOR),
                (STANDORT_NAME, MapElementKind::Point, STANDORT_COLOR),
            ]
        );
        assert_eq!(standort_location(standort), "52.43000, 10.01000");

        let broken = FmMapImport {
            features: vec![
                feature(MapElementKind::Point, 1),
                feature(MapElementKind::Polygon, 2),
            ],
            standort: None,
        };
        assert!(import(&broken).unwrap_err().starts_with("Zeichnung 2: "));
        let too_many = FmMapImport {
            features: vec![feature(MapElementKind::Point, 1); MAX_IMPORT_FEATURES + 1],
            standort: None,
        };
        assert!(import(&too_many).is_err());
        let bad_standort = FmMapImport {
            features: vec![],
            standort: Some(GeoPoint {
                lat: 99.0,
                lon: 10.0,
            }),
        };
        assert!(import(&bad_standort).unwrap_err().starts_with("Standort: "));
    }

    #[test]
    fn map_photos_are_recognised_by_their_first_bytes() {
        assert!(is_jpeg(&[0xFF, 0xD8, 0xFF, 0xE0, 0, 0x10]));
        assert!(!is_jpeg(b"\x89PNG\r\n"));
        assert!(!is_jpeg(&[0xFF, 0xD8]));
        assert!(!is_jpeg(&[]));
    }
}

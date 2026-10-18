//! Prüfregeln für Einsätze und ihre Verknüpfung mit der Flugmappe. Rein: kein Speicher, keine
//! Laufzeit.

use crate::contract::{
    FmImportFeature, FmMapImport, GeoPoint, MAP_COLORS, MapElementKind, MapPoint, MissionState,
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
    Conflict(&'static str),
    /// 422
    Unprocessable(&'static str),
}

pub const CLOSED: &str = "Der Einsatz ist abgeschlossen";
pub const OTHER_RUNNING: &str = "Es läuft bereits ein anderer Einsatz";
pub const OTHER_FLUGMAPPE: &str = "Der Einsatz ist mit einer anderen Flugmappe verknüpft";
pub const NOT_LINKED: &str = "Der Einsatz ist nicht mit dieser Flugmappe verknüpft";
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

/// Nur die verknüpfte Flugmappe darf an einem Einsatz lesen und schreiben. Nach „Übernehmen“
/// bekommt die abgelöste nichts mehr.
///
/// Die Bindung schützt vor veralteten Geräten (eine abgelöste Flugmappe schreibt nicht weiter),
/// nicht vor böswilligen Konten: Die UUID ist kein Geheimnis, und jedes Konto ist `operator` oder
/// `admin` und darf dasselbe ohnehin im Browser.
pub fn binding(linked: Option<&str>, caller: &str) -> Result<(), Refusal> {
    match linked {
        Some(l) if l == caller => Ok(()),
        Some(_) => Err(Refusal::Conflict(OTHER_FLUGMAPPE)),
        None => Err(Refusal::Conflict(NOT_LINKED)),
    }
}

/// Ein Einsatz, an den die UUID beim Anlegen schon gebunden ist.
#[derive(Debug, Clone, Copy)]
pub struct Existing {
    pub state: MissionState,
    /// Diese Flugmappe hat ihn mit dieser UUID angelegt: Herkunft Flugmappe und Verknüpfung seit
    /// der Anlage unverändert (Verknüpfen und Übernehmen setzen eine spätere Zeit).
    pub created_by_caller: bool,
}

/// Anlegen mit einer UUID, die schon gebunden ist. Hat diese Flugmappe den Einsatz selbst angelegt
/// und ist er offen, war es eine Wiederholung (Antwort verloren): Ergebnis ist der vorhandene
/// (200). Ist er abgeschlossen, ist das ein Übergang, den es nicht mehr gibt (422) – kein Konflikt
/// mit jemand anderem. Ist die UUID an einen fremden oder übernommenen Einsatz gebunden, 409.
pub fn create_again(e: Existing) -> Result<(), Refusal> {
    match (e.created_by_caller, e.state) {
        (true, MissionState::Closed) => Err(Refusal::Unprocessable(CLOSED)),
        (true, _) => Ok(()),
        (false, _) => Err(Refusal::Conflict(FLUGMAPPE_TAKEN)),
    }
}

/// Was die Flugmappe über den Einsatz weiß, wenn sie verknüpfen will.
#[derive(Debug, Clone, Copy)]
pub struct LinkFacts<'a> {
    pub state: MissionState,
    /// Mit welcher Flugmappe der Einsatz verknüpft ist.
    pub linked: Option<&'a str>,
    pub caller: &'a str,
    pub take_over: bool,
    /// Ein anderer Einsatz läuft.
    pub other_running: bool,
    /// Die Flugmappe ist schon mit einem anderen Einsatz verknüpft.
    pub caller_linked_elsewhere: bool,
}

/// Was beim Verknüpfen zu tun ist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinkPlan {
    /// vorbereitet → laufend
    pub start: bool,
    /// Verknüpfung schreiben (neu oder übernommen); `false`, wenn sie schon so besteht.
    pub write_link: bool,
}

/// Verknüpfen: vorbereitet oder laufend, kein anderer läuft, und eine fremde Verknüpfung nur mit
/// ausdrücklichem Übernehmen. Wiederholen ist harmlos.
pub fn link(f: LinkFacts) -> Result<LinkPlan, Refusal> {
    if f.state == MissionState::Closed {
        return Err(Refusal::Unprocessable(CLOSED));
    }
    if f.caller_linked_elsewhere {
        return Err(Refusal::Conflict(FLUGMAPPE_TAKEN));
    }
    let start = f.state == MissionState::Prepared;
    if start && f.other_running {
        return Err(Refusal::Conflict(OTHER_RUNNING));
    }
    match f.linked {
        Some(l) if l == f.caller => Ok(LinkPlan {
            start,
            write_link: false,
        }),
        Some(_) if !f.take_over => Err(Refusal::Conflict(OTHER_FLUGMAPPE)),
        _ => Ok(LinkPlan {
            start,
            write_link: true,
        }),
    }
}

/// Welcher Abschnitt beim Verknüpfen der der Flugmappe wird.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionStep {
    /// Den gibt es schon mit dieser ID.
    Known(i64),
    /// Der aktive (letzte) Abschnitt bekommt die ID – auch beim Übernehmen, dann ersetzt sie die
    /// der abgelösten Flugmappe.
    Bind(i64),
}

/// `sections`: (ID in Suchlage, ID der Flugmappe) in Anlegereihenfolge, mindestens einer.
pub fn section_for_link(sections: &[(i64, Option<&str>)], caller_section: &str) -> SectionStep {
    match sections.iter().find(|(_, fm)| *fm == Some(caller_section)) {
        Some((id, _)) => SectionStep::Known(*id),
        None => SectionStep::Bind(sections.last().expect("Einsatz hat einen Abschnitt").0),
    }
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

    const FM: &str = "fm-a";

    fn facts(state: MissionState, linked: Option<&'static str>) -> LinkFacts<'static> {
        LinkFacts {
            state,
            linked,
            caller: FM,
            take_over: false,
            other_running: false,
            caller_linked_elsewhere: false,
        }
    }

    #[test]
    fn linking_starts_a_prepared_mission_unless_another_runs() {
        assert_eq!(
            link(facts(Prepared, None)),
            Ok(LinkPlan {
                start: true,
                write_link: true
            })
        );
        assert_eq!(
            link(LinkFacts {
                other_running: true,
                ..facts(Prepared, None)
            }),
            Err(Refusal::Conflict(OTHER_RUNNING))
        );
        assert_eq!(
            link(facts(Running, None)),
            Ok(LinkPlan {
                start: false,
                write_link: true
            }),
            "ein laufender Einsatz ohne Flugmappe darf verknüpft werden"
        );
        assert_eq!(
            link(facts(Closed, None)),
            Err(Refusal::Unprocessable(CLOSED))
        );
        assert_eq!(
            link(facts(Closed, Some(FM))),
            Err(Refusal::Unprocessable(CLOSED))
        );
    }

    #[test]
    fn a_foreign_link_needs_an_explicit_take_over() {
        assert_eq!(
            link(facts(Running, Some("fm-b"))),
            Err(Refusal::Conflict(OTHER_FLUGMAPPE))
        );
        assert_eq!(
            link(LinkFacts {
                take_over: true,
                ..facts(Running, Some("fm-b"))
            }),
            Ok(LinkPlan {
                start: false,
                write_link: true
            })
        );
        assert_eq!(
            link(facts(Running, Some(FM))),
            Ok(LinkPlan {
                start: false,
                write_link: false
            }),
            "wiederholtes Verknüpfen ändert nichts"
        );
        assert_eq!(
            link(LinkFacts {
                caller_linked_elsewhere: true,
                take_over: true,
                ..facts(Running, None)
            }),
            Err(Refusal::Conflict(FLUGMAPPE_TAKEN))
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
            Err(Refusal::Unprocessable(CLOSED))
        );
        assert_eq!(
            create_again(Existing {
                state: Running,
                created_by_caller: false,
            }),
            Err(Refusal::Conflict(FLUGMAPPE_TAKEN)),
            "verknüpft oder übernommen, nicht angelegt"
        );
    }

    #[test]
    fn only_the_linked_flugmappe_passes_the_binding() {
        assert_eq!(binding(Some(FM), FM), Ok(()));
        assert_eq!(
            binding(Some("fm-b"), FM),
            Err(Refusal::Conflict(OTHER_FLUGMAPPE))
        );
        assert_eq!(binding(None, FM), Err(Refusal::Conflict(NOT_LINKED)));
    }

    #[test]
    fn linking_binds_the_active_section_unless_one_is_known() {
        let sections = [(1, None), (2, Some("s-alt"))];
        assert_eq!(section_for_link(&sections, "s-neu"), SectionStep::Bind(2));
        assert_eq!(section_for_link(&sections, "s-alt"), SectionStep::Known(2));
        assert_eq!(section_for_link(&[(5, None)], "s"), SectionStep::Bind(5));
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

//! Schnittstelle für die Flugmappe (`/api/flugmappe/v2`, SUL-66, mehrere Flugmappen SUL-113). Die Flugmappe übernimmt die
//! erzeugten TypeScript-Typen als Kopie. Felder `snake_case`, Zeiten in ms seit Epoch.
//!
//! Einsatzbezogene Aufrufe (`/missions/{id}/…`) tragen den Header [`FLUGMAPPE_MISSION_HEADER`]
//! mit der Einsatz-ID der Flugmappe; Abschnitte adressiert die Flugmappe über ihre eigene ID.
//! Mehrere Flugmappen dürfen gleichzeitig mit einem Einsatz verknüpft sein; jede sieht nur ihre
//! eigenen Abschnitts-IDs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::admin::Role;
use super::flights::FlightEndReason;
use super::live::GeoPoint;
use super::map::MapElementKind;
use super::missions::MissionState;

/// Version der Schnittstelle, steht im Pfad (`/v2`) und in [`FmMe`].
pub const FLUGMAPPE_API_VERSION: u32 = 2;
/// Einsatz-ID der Flugmappe (UUID) bei allen Aufrufen unter `/missions/{id}/…`.
pub const FLUGMAPPE_MISSION_HEADER: &str = "X-Flugmappe-Einsatz";

/// Antwort der Probe `GET /me`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMe {
    /// Anzeigename des Kontos in Suchlage.
    pub name: String,
    pub role: Role,
    pub api_version: u32,
}

/// Eine Flugmappe, die mit dem Einsatz verknüpft ist. Die Flugmappe erkennt ihre eigene an der
/// UUID; die anderen zeigt sie als „ebenfalls verknüpft“.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmLinkInfo {
    pub flugmappe_mission_id: String,
    /// Fluggerät der Flugmappe, z. B. „Matrice 200“.
    pub aircraft_label: String,
    /// Anzeigename des Kontos, das verknüpft hat.
    pub linked_by: String,
    #[ts(type = "number")]
    pub linked_at_ms: i64,
    /// Die Flugmappe hat ihren Teil abgeschlossen.
    #[ts(type = "number | null")]
    pub done_at_ms: Option<i64>,
    /// In Suchlage gelöst: Diese Flugmappe darf nichts mehr am Einsatz, bis sie neu verknüpft.
    #[ts(type = "number | null")]
    pub released_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmSection {
    /// ID in Suchlage.
    #[ts(type = "number")]
    pub id: i64,
    /// Abschnitts-ID der aufrufenden Flugmappe; `null`: in Suchlage oder von einer anderen
    /// Flugmappe angelegt (deren IDs sieht sie nicht).
    pub flugmappe_section_id: Option<String>,
    pub label: String,
    pub location: Option<String>,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    pub has_map_photo: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMission {
    #[ts(type = "number")]
    pub id: i64,
    pub name: String,
    pub location: Option<String>,
    pub state: MissionState,
    #[ts(type = "number | null")]
    pub started_at_ms: Option<i64>,
    #[ts(type = "number")]
    pub created_at_ms: i64,
    pub created_by: String,
    /// Älteste zuerst; der letzte ist der aktive des Einsatzes, der letzte mit eigener ID der
    /// aktive der Flugmappe.
    pub sections: Vec<FmSection>,
    /// Alle Verknüpfungen, älteste zuerst, auch gelöste; leer: mit keiner Flugmappe verknüpft.
    pub links: Vec<FmLinkInfo>,
}

/// `POST /missions`: neuer Einsatz aus der Flugmappe, läuft sofort.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMissionCreate {
    pub name: String,
    pub location: Option<String>,
    pub section_label: String,
    pub section_location: Option<String>,
    /// UUID des Einsatzes in der Flugmappe.
    pub flugmappe_mission_id: String,
    pub flugmappe_section_id: String,
    /// Fluggerät der Flugmappe, z. B. „Matrice 350 RTK“.
    pub aircraft_label: String,
}

/// `POST /missions/{id}/link`: vorbereiteten oder laufenden Einsatz verknüpfen. Andere
/// verknüpfte Flugmappen stören nicht („Dazukommen“). Verknüpft dieselbe Flugmappe denselben
/// Einsatz wieder, lebt ihre alte (auch gelöste) Verknüpfung wieder auf.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMissionLink {
    pub flugmappe_mission_id: String,
    /// Aktiver Abschnitt der Flugmappe; wird der aktive Abschnitt der Flugmappe in Suchlage.
    pub flugmappe_section_id: String,
    pub section_label: String,
    pub section_location: Option<String>,
    /// Fluggerät der Flugmappe.
    pub aircraft_label: String,
}

/// `POST /missions/{id}/sections`: Verlegung in der Flugmappe; wird der aktive Abschnitt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmSectionCreate {
    pub flugmappe_section_id: String,
    pub label: String,
    pub location: Option<String>,
}

/// `PUT /missions/{id}/sections/{fm_section_id}`: Bezeichnung und Ort eines Abschnitts, meist
/// der Ort, den der Truppführer erst nach der Verlegung setzt (SUL-81). Ersetzt beides; der
/// aktive Abschnitt bleibt, wer er ist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmSectionUpdate {
    pub label: String,
    /// `null` oder leer: kein Ort.
    pub location: Option<String>,
}

/// Ein Flug aus der Telemetrie, als Vorschlag für das Flugbuch.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmFlight {
    #[ts(type = "number")]
    pub id: i64,
    /// Abschnitt der Flugmappe, der beim Start aktiv war; `null`, wenn er keinen hat.
    pub flugmappe_section_id: Option<String>,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    /// `null`: Der Flug läuft noch.
    #[ts(type = "number | null")]
    pub ended_at_ms: Option<i64>,
    pub end_reason: Option<FlightEndReason>,
    pub aircraft_sn: String,
    /// Bezeichnung der Fernsteuerung in Suchlage, sonst `null`.
    pub aircraft_label: Option<String>,
    /// Höchste Höhe über Startpunkt.
    pub max_elevation_m: Option<f64>,
    pub distance_m: f64,
    /// Auslösungen (eine Auslösung mit mehreren Linsen zählt einmal).
    pub photo_count: u32,
    /// Fluggerät einer **anderen** Flugmappe, die den Flug bestätigt hat: dann kein Vorschlag
    /// mehr. `null`: unbestätigt oder von der aufrufenden bestätigt (die kennt ihren Eintrag).
    pub confirmed_by: Option<String>,
}

/// `PUT /missions/{id}/logbook/{entry_id}`: ein Flugbucheintrag der Flugmappe, bestätigt
/// (`flight_id`) oder von Hand. Wiederholbar; Suchlage übernimmt ihn nur mit höherer `revision`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmLogbookEntry {
    /// Steigt in der Flugmappe bei jeder Änderung und beim Löschen um eins, ab 1.
    #[ts(type = "number")]
    pub revision: i64,
    /// Bestätigter Telemetrie-Flug dieses Einsatzes; `null`: Handeintrag.
    #[ts(type = "number | null")]
    pub flight_id: Option<i64>,
    /// Abschnitt der Flugmappe; beim bestätigten Flug übernimmt Suchlage ihn auch für den Flug.
    pub flugmappe_section_id: Option<String>,
    #[ts(type = "number")]
    pub started_at_ms: i64,
    /// `null`: noch in der Luft.
    #[ts(type = "number | null")]
    pub landed_at_ms: Option<i64>,
    /// Fluggerät, z. B. „Matrice 200“.
    pub aircraft_label: String,
    /// Fernpilot.
    pub pilot: Option<String>,
    /// Landestatus wie in der Flugmappe (Freitext, z. B. „normal“).
    pub landing_status: Option<String>,
    /// Änderungszeitpunkt in der Flugmappe, nur zur Anzeige.
    #[ts(type = "number")]
    pub changed_at_ms: i64,
}

/// Antwort auf `PUT …/logbook/{entry_id}`: der gespeicherte Stand. Bei gleicher oder kleinerer
/// Revision ist das der ältere gespeicherte (kein Fehler), `entry` ist `null` bei einer Löschmarke.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmLogbookStored {
    pub entry_id: String,
    #[ts(type = "number")]
    pub revision: i64,
    pub entry: Option<FmLogbookEntry>,
}

/// Eine Zeichnung aus der Einsatzkarte der Flugmappe. Kreise kommen als Polygon.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmImportFeature {
    /// `null` oder leer: „Fläche“, „Linie“ oder „Punkt“.
    pub name: Option<String>,
    pub kind: MapElementKind,
    /// Fläche: Ring ohne wiederholten Schlusspunkt (ein wiederholter wird entfernt).
    pub points: Vec<GeoPoint>,
}

/// `POST /missions/{id}/map/import`: einmal je Einsatz, beim Verknüpfen; die Zeichnungen einer
/// später dazukommenden Flugmappe übernimmt Suchlage nicht mehr.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMapImport {
    pub features: Vec<FmImportFeature>,
    /// Standort aus der Flugmappe: roter Punkt, und Ort des aktiven Abschnitts, falls leer.
    pub standort: Option<GeoPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmMapImportResult {
    /// Angelegte Kartenelemente; 0, wenn schon übernommen war.
    pub imported: u32,
}

/// `[Längengrad, Breitengrad]` wie in GeoJSON.
pub type FmPosition = [f64; 2];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FmFeatureCollectionType {
    FeatureCollection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[ts(export)]
pub enum FmFeatureType {
    Feature,
}

/// Die Geometrien, die Suchlage liefert (RFC 7946).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "type")]
#[ts(export)]
pub enum FmGeometry {
    Point {
        coordinates: FmPosition,
    },
    LineString {
        coordinates: Vec<FmPosition>,
    },
    /// Ein Ring, geschlossen (erster Punkt am Ende wiederholt).
    Polygon {
        coordinates: Vec<Vec<FmPosition>>,
    },
    /// Eine Flugspur, je Segment eine Linie.
    MultiLineString {
        coordinates: Vec<Vec<FmPosition>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[ts(export)]
pub enum FmFeatureProperties {
    /// Kartenelement des Einsatzes (Kartenelemente haben keinen Abschnitt).
    MapElement {
        id: String,
        name: String,
        /// `#RRGGBB`.
        color: String,
    },
    FlightTrack {
        #[ts(type = "number")]
        flight_id: i64,
        name: String,
        color: String,
        flugmappe_section_id: Option<String>,
        #[ts(type = "number")]
        started_at_ms: i64,
        #[ts(type = "number | null")]
        ended_at_ms: Option<i64>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmFeature {
    #[serde(rename = "type")]
    pub kind: FmFeatureType,
    pub geometry: FmGeometry,
    pub properties: FmFeatureProperties,
}

/// `GET /missions/{id}/map`: GeoJSON der Einsatzkarte.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FmFeatureCollection {
    #[serde(rename = "type")]
    pub kind: FmFeatureCollectionType,
    pub features: Vec<FmFeature>,
}

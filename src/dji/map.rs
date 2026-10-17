//! DJI-Format der Kartenelemente (Referenz 6.1; Original `10.https/10.map-elements/*` und
//! `20.websocket/10.map-elements/10.message-push.md`). Übersetzt zwischen GeoJSON-Inhalt und
//! `contract::MapElement`; prüfen tut `map::rules`.

use serde::Deserialize;
use serde_json::{Value, json};

use crate::contract::{MapElement, MapElementKind, MapPoint};

/// Die eine Gruppe vom Typ 2 (App-Shared). Ohne sie lädt Pilot 2 neue Elemente nicht hoch.
pub const SHARED_GROUP_ID: &str = "8f2d6c1e-3b7a-4c5e-9d10-5a6b7c8d9e0f";
const SHARED_GROUP_NAME: &str = "Suchlage";
const GROUP_TYPE_APP_SHARED: u8 = 2;

/// Body von `POST …/element-groups/{group_id}/elements`.
#[derive(Debug, Deserialize)]
pub struct ElementCreateInput {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    pub resource: ResourceInput,
}

#[derive(Debug, Deserialize)]
pub struct ResourceInput {
    #[serde(default)]
    pub user_name: Option<String>,
    pub content: ContentInput,
}

/// Body von `PUT …/elements/{id}`. Beide Felder sind laut Doku optional.
#[derive(Debug, Deserialize)]
pub struct ElementUpdateInput {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub content: Option<ContentInput>,
}

/// GeoJSON-Feature. `resource.type` wird nicht gelesen: Das Beispiel der Doku meldet eine Linie
/// mit `type: 0` (Pin). Maßgeblich ist `geometry.type`.
#[derive(Debug, Deserialize)]
pub struct ContentInput {
    #[serde(default)]
    properties: PropertiesInput,
    geometry: GeometryInput,
}

#[derive(Debug, Default, Deserialize)]
struct PropertiesInput {
    #[serde(default)]
    color: Option<String>,
    #[serde(default, rename = "clampToGround")]
    clamp_to_ground: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct GeometryInput {
    #[serde(rename = "type")]
    kind: String,
    coordinates: Value,
}

/// Was der Inhalt beschreibt; die Farbe roh, wie Pilot 2 sie schickt.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    pub kind: MapElementKind,
    pub points: Vec<MapPoint>,
    pub color: Option<String>,
    pub clamp_to_ground: Option<bool>,
}

impl ContentInput {
    pub fn shape(&self) -> Result<Shape, String> {
        let unreadable = || format!("Koordinaten der Geometrie {} unlesbar", self.geometry.kind);
        let coords = &self.geometry.coordinates;
        let (kind, points) = match self.geometry.kind.as_str() {
            "Point" => (
                MapElementKind::Point,
                vec![position(coords).ok_or_else(unreadable)?],
            ),
            "LineString" => (
                MapElementKind::Line,
                positions(coords).ok_or_else(unreadable)?,
            ),
            // Nur der äußere Ring: Löcher kann Pilot 2 nicht zeichnen.
            "Polygon" => (
                MapElementKind::Polygon,
                coords.get(0).and_then(positions).ok_or_else(unreadable)?,
            ),
            other => return Err(format!("Geometrie {other} wird nicht unterstützt")),
        };
        Ok(Shape {
            kind,
            points,
            color: self.properties.color.clone(),
            clamp_to_ground: self.properties.clamp_to_ground,
        })
    }
}

/// `[lon, lat]` oder `[lon, lat, alt]`.
fn position(v: &Value) -> Option<MapPoint> {
    let a = v.as_array()?;
    if !(2..=3).contains(&a.len()) {
        return None;
    }
    Some(MapPoint {
        lon: a[0].as_f64()?,
        lat: a[1].as_f64()?,
        alt_m: a.get(2).and_then(Value::as_f64),
    })
}

fn positions(v: &Value) -> Option<Vec<MapPoint>> {
    v.as_array()?.iter().map(position).collect()
}

fn resource_type(kind: MapElementKind) -> u8 {
    match kind {
        MapElementKind::Point => 0,
        MapElementKind::Line => 1,
        MapElementKind::Polygon => 2,
    }
}

fn coordinate(p: &MapPoint) -> Value {
    match p.alt_m {
        Some(alt) => json!([p.lon, p.lat, alt]),
        None => json!([p.lon, p.lat]),
    }
}

/// GeoJSON wie im Beispiel der Doku; der Ring einer Fläche bleibt offen wie dort.
fn content(e: &MapElement) -> Value {
    let all: Vec<Value> = e.points.iter().map(coordinate).collect();
    let (kind, coordinates) = match e.kind {
        MapElementKind::Point => ("Point", all.into_iter().next().unwrap_or(Value::Null)),
        MapElementKind::Line => ("LineString", Value::Array(all)),
        MapElementKind::Polygon => ("Polygon", json!([all])),
    };
    json!({
        "type": "Feature",
        "properties": { "color": e.color, "clampToGround": e.clamp_to_ground },
        "geometry": { "type": kind, "coordinates": coordinates },
    })
}

fn resource(e: &MapElement) -> Value {
    json!({ "type": resource_type(e.kind), "user_name": e.author, "content": content(e) })
}

/// Ein Element in der Gruppenliste (`map.ElementItem`).
pub fn element(e: &MapElement) -> Value {
    json!({
        "id": e.id,
        "name": e.name,
        "create_time": e.created_at_ms,
        "update_time": e.updated_at_ms,
        "resource": resource(e),
    })
}

/// Antwort auf `GET …/element-groups`: genau die App-Shared-Gruppe.
pub fn groups(elements: &[MapElement]) -> Value {
    json!([{
        "id": SHARED_GROUP_ID,
        "name": SHARED_GROUP_NAME,
        "type": GROUP_TYPE_APP_SHARED,
        "is_lock": false,
        "create_time": 0,
        "elements": elements.iter().map(element).collect::<Vec<_>>(),
    }])
}

/// HTTPS-Antworthülle (`40.https.md`).
pub fn ok(data: Value) -> Value {
    json!({ "code": 0, "message": "success", "data": data })
}

/// Fehlerhülle; `code` ist der HTTP-Status, den Pilot 2 im Toast zeigt.
pub fn failure(code: u16, message: &str) -> Value {
    json!({ "code": code, "message": message, "data": {} })
}

fn push(biz_code: &str, data: Value, timestamp_ms: i64) -> String {
    json!({ "biz_code": biz_code, "version": "1.0", "timestamp": timestamp_ms, "data": data })
        .to_string()
}

fn element_push(e: &MapElement) -> Value {
    json!({ "id": e.id, "group_id": SHARED_GROUP_ID, "name": e.name, "resource": resource(e) })
}

pub fn push_created(e: &MapElement, timestamp_ms: i64) -> String {
    push("map_element_create", element_push(e), timestamp_ms)
}

pub fn push_updated(e: &MapElement, timestamp_ms: i64) -> String {
    push("map_element_update", element_push(e), timestamp_ms)
}

pub fn push_deleted(id: &str, timestamp_ms: i64) -> String {
    push(
        "map_element_delete",
        json!({ "id": id, "group_id": SHARED_GROUP_ID }),
        timestamp_ms,
    )
}

/// Pilot 2 lädt daraufhin die Gruppen per GET neu. Die Doku nennt die IDs der Gruppen.
pub fn push_group_refresh(timestamp_ms: i64) -> String {
    push(
        "map_group_refresh",
        json!({ "ids": [SHARED_GROUP_ID] }),
        timestamp_ms,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{MapElementOrigin, examples};

    /// Body aus `10.create.md`, mit echten Werten statt der Schema-Platzhalter.
    const CREATE: &str = r##"{
        "id": "94c51c50-f111-45e8-ac8c-4f96c93ced44",
        "name": "Pilot 3",
        "resource": {
            "type": 0,
            "user_name": "RC Plus 1",
            "content": {
                "type": "Feature",
                "properties": { "color": "#E23C39", "clampToGround": false },
                "geometry": { "type": "Point", "coordinates": [10.0124, 52.4318, 84.2] }
            }
        }
    }"##;

    fn read<T: serde::de::DeserializeOwned>(json: &str) -> T {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn create_body_from_the_documentation_is_understood() {
        let input: ElementCreateInput = read(CREATE);
        assert_eq!(input.id, "94c51c50-f111-45e8-ac8c-4f96c93ced44");
        assert_eq!(input.resource.user_name.as_deref(), Some("RC Plus 1"));
        let shape = input.resource.content.shape().unwrap();
        assert_eq!(shape.kind, MapElementKind::Point);
        assert_eq!(
            shape.points,
            vec![MapPoint {
                lat: 52.4318,
                lon: 10.0124,
                alt_m: Some(84.2)
            }]
        );
        assert_eq!(
            (shape.color.as_deref(), shape.clamp_to_ground),
            (Some("#E23C39"), Some(false))
        );
    }

    /// Die Elemente aus dem Beispiel von `30.obtain.md`: Linie mit `resource.type: 0`, offener Ring.
    #[test]
    fn geometry_type_decides_not_resource_type() {
        let line: ContentInput = read(
            r##"{"type":"Feature","properties":{"color":"#0091FF","clampToGround":false},
                "geometry":{"type":"LineString","coordinates":[[-112.49,48.18],[-104.47,46.60]]}}"##,
        );
        let shape = line.shape().unwrap();
        assert_eq!(shape.kind, MapElementKind::Line);
        assert_eq!(shape.points.len(), 2);
        assert_eq!(shape.points[0].alt_m, None);

        let area: ContentInput = read(
            r#"{"type":"Feature","geometry":{"type":"Polygon",
                "coordinates":[[[-112.49,48.18],[-104.47,46.60],[-109.20,42.63]],[[0,0],[1,1],[1,0]]]}}"#,
        );
        let shape = area.shape().unwrap();
        assert_eq!(shape.kind, MapElementKind::Polygon);
        assert_eq!(shape.points.len(), 3, "nur der äußere Ring");
        assert_eq!((shape.color, shape.clamp_to_ground), (None, None));
    }

    #[test]
    fn unknown_or_broken_geometry_is_a_readable_error() {
        let circle: ContentInput =
            read(r#"{"geometry":{"type":"Circle","coordinates":[10.0,52.0]}}"#);
        assert_eq!(
            circle.shape().unwrap_err(),
            "Geometrie Circle wird nicht unterstützt"
        );
        for coords in [r#""x""#, "[10.0]", r#"[10.0, "52"]"#, "[1, 2, 3, 4]"] {
            let broken: ContentInput = read(&format!(
                r#"{{"geometry":{{"type":"Point","coordinates":{coords}}}}}"#
            ));
            assert!(broken.shape().is_err(), "{coords}");
        }
    }

    #[test]
    fn update_body_may_carry_only_a_name() {
        let only_name: ElementUpdateInput = read(r#"{"name":"Fundstelle"}"#);
        assert_eq!(only_name.name.as_deref(), Some("Fundstelle"));
        assert!(only_name.content.is_none());
    }

    #[test]
    fn group_list_has_the_app_shared_group_with_doc_shaped_elements() {
        let elements = examples::map_elements();
        let list = groups(&elements);
        let group = &list[0];
        assert_eq!(
            group["type"], 2,
            "ohne App-Shared-Gruppe lädt Pilot 2 nichts hoch"
        );
        assert_eq!(group["id"], SHARED_GROUP_ID);
        assert_eq!(group["is_lock"], false);

        let area = &group["elements"][0];
        assert_eq!(area["name"], "Suchgebiet Nord");
        assert_eq!(area["resource"]["type"], 2);
        let content = &area["resource"]["content"];
        assert_eq!(content["type"], "Feature");
        assert_eq!(content["properties"]["clampToGround"], true);
        assert_eq!(content["geometry"]["type"], "Polygon");
        assert_eq!(
            content["geometry"]["coordinates"][0][0],
            json!([10.008, 52.433]),
            "[lon, lat], ohne Höhe"
        );
        assert_eq!(
            content["geometry"]["coordinates"][0]
                .as_array()
                .unwrap()
                .len(),
            4
        );

        let pin = &group["elements"][1];
        assert_eq!(pin["resource"]["type"], 0);
        assert_eq!(pin["resource"]["user_name"], "RC Plus 1");
        assert_eq!(
            pin["resource"]["content"]["geometry"]["coordinates"],
            json!([10.0124, 52.4318, 84.2])
        );
    }

    #[test]
    fn an_element_survives_the_round_trip_through_dji() {
        for e in examples::map_elements() {
            let back: ContentInput =
                serde_json::from_value(element(&e)["resource"]["content"].clone()).unwrap();
            let shape = back.shape().unwrap();
            assert_eq!(
                (shape.kind, shape.points, shape.color, shape.clamp_to_ground),
                (e.kind, e.points, Some(e.color), Some(e.clamp_to_ground))
            );
        }
    }

    #[test]
    fn pushes_have_the_websocket_envelope() {
        let e = MapElement {
            origin: MapElementOrigin::Web,
            ..examples::map_elements().remove(0)
        };
        let created: Value = serde_json::from_str(&push_created(&e, 7)).unwrap();
        assert_eq!(created["biz_code"], "map_element_create");
        assert_eq!(created["version"], "1.0");
        assert_eq!(created["timestamp"], 7);
        assert_eq!(created["data"]["group_id"], SHARED_GROUP_ID);
        assert_eq!(created["data"]["id"], e.id.as_str());
        assert_eq!(created["data"]["resource"]["type"], 2);

        let updated: Value = serde_json::from_str(&push_updated(&e, 8)).unwrap();
        assert_eq!(updated["biz_code"], "map_element_update");
        let deleted: Value = serde_json::from_str(&push_deleted("x", 9)).unwrap();
        assert_eq!(
            (deleted["biz_code"].as_str(), &deleted["data"]),
            (
                Some("map_element_delete"),
                &json!({"id": "x", "group_id": SHARED_GROUP_ID})
            )
        );
        let refresh: Value = serde_json::from_str(&push_group_refresh(10)).unwrap();
        assert_eq!(refresh["data"]["ids"], json!([SHARED_GROUP_ID]));
    }

    #[test]
    fn response_envelopes() {
        assert_eq!(
            ok(json!({"id": "a"})),
            json!({"code": 0, "message": "success", "data": {"id": "a"}})
        );
        assert_eq!(failure(401, "x")["code"], 401);
    }
}

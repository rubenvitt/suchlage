//! DJI-Format der Medienverwaltung (Cloud API v1.11.3, `30.media-management/*`): was Pilot 2 vor,
//! während und nach dem Upload eines Fotos schickt, und die Antwort auf `sts`. Die Hülle
//! `{code, message, data}` ist dieselbe wie bei den Kartenelementen (`dji::map::ok`).

use serde::{Deserialize, Deserializer};
use serde_json::{Value, json};

/// Anbieter für Pilot 2. Garage spricht S3 im Path-Style wie MinIO; die DJI-Demo nutzt `minio`
/// mit eigenem Endpunkt (Plan M3, offener Punkt 1: Rückfall `aws`).
pub const PROVIDER: &str = "minio";

/// Zahl oder Text als `f64`: Die Doku typisiert `shoot_position` als integer, Pilot 2 schickt
/// Kommazahlen; manche Felder kommen als Text.
fn lenient_f64<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::Number(n)) => n.as_f64(),
        Some(Value::String(s)) => s.trim().parse().ok(),
        _ => None,
    })
}

/// `created_time` ist ein Text ohne festes Format; eine Zahl wird zu Text.
fn lenient_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Option::<Value>::deserialize(d)? {
        Some(Value::String(s)) => Some(s),
        Some(Value::Number(n)) => Some(n.to_string()),
        _ => None,
    })
}

#[derive(Debug, Default, Deserialize)]
pub struct MediaExt {
    pub file_group_id: Option<String>,
    pub drone_model_key: Option<String>,
    pub payload_model_key: Option<String>,
    pub is_original: Option<bool>,
    /// So heißt das Feld bei DJI wirklich (Tippfehler im Original).
    #[serde(rename = "tinny_fingerprint", alias = "tiny_fingerprint")]
    pub tiny_fingerprint: Option<String>,
    /// Seriennummer des Fluggeräts.
    pub sn: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ShootPosition {
    #[serde(default, deserialize_with = "lenient_f64")]
    pub lat: Option<f64>,
    #[serde(default, deserialize_with = "lenient_f64")]
    pub lng: Option<f64>,
}

#[derive(Debug, Default, Deserialize)]
pub struct Metadata {
    #[serde(default, deserialize_with = "lenient_f64")]
    pub absolute_altitude: Option<f64>,
    #[serde(default, deserialize_with = "lenient_f64")]
    pub relative_altitude: Option<f64>,
    #[serde(default, deserialize_with = "lenient_f64")]
    pub gimbal_yaw_degree: Option<f64>,
    #[serde(default, deserialize_with = "lenient_text")]
    pub created_time: Option<String>,
    pub shoot_position: Option<ShootPosition>,
}

#[derive(Debug, Deserialize)]
pub struct TinyFingerprintsInput {
    #[serde(default)]
    pub tiny_fingerprints: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct FastUploadInput {
    pub fingerprint: String,
    pub name: Option<String>,
    pub path: Option<String>,
    pub ext: Option<MediaExt>,
}

#[derive(Debug, Deserialize)]
pub struct UploadCallbackInput {
    /// 0 = Upload in den Speicher gelungen.
    pub result: i64,
    pub name: String,
    pub object_key: String,
    pub path: Option<String>,
    pub fingerprint: Option<String>,
    /// 0 normales Foto, 1 Panorama.
    pub sub_file_type: Option<i64>,
    #[serde(default)]
    pub ext: MediaExt,
    #[serde(default)]
    pub metadata: Metadata,
}

#[derive(Debug, Deserialize)]
pub struct GroupUploadCallbackInput {
    pub file_group_id: Option<String>,
    pub file_count: i64,
    pub file_uploaded_count: i64,
}

/// Antwort von `tiny-fingerprints`: die schon bekannten aus der Anfrage.
pub fn known_tiny_fingerprints(known: &[String]) -> Value {
    json!({ "tiny_fingerprints": known })
}

/// Zugang für den Upload; `security_token` bleibt leer, Garage kennt keine Sitzungen.
pub struct Sts<'a> {
    pub endpoint: &'a str,
    pub bucket: &'a str,
    pub region: &'a str,
    pub object_key_prefix: &'a str,
    pub access_key_id: &'a str,
    pub access_key_secret: &'a str,
    /// Restlaufzeit in Sekunden.
    pub expire_s: i64,
}

pub fn sts(s: &Sts) -> Value {
    json!({
        "bucket": s.bucket,
        "credentials": {
            "access_key_id": s.access_key_id,
            "access_key_secret": s.access_key_secret,
            "expire": s.expire_s,
            "security_token": "",
        },
        "endpoint": s.endpoint,
        "object_key_prefix": s.object_key_prefix,
        "provider": PROVIDER,
        "region": s.region,
    })
}

/// `fast-upload` für eine unbekannte Datei: HTTP 200 mit Fehler-Code wie in der DJI-Demo, damit
/// Pilot 2 normal hochlädt.
pub fn not_uploaded_yet() -> Value {
    json!({ "code": -1, "message": "Datei ist noch nicht im Speicher", "data": {} })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upload_callback_reads_the_documented_example_and_real_numbers() {
        // Beispiel aus `40.mediafile-upload-result-report.md`, Position mit Kommazahlen.
        let body = json!({
            "result": 0,
            "ext": {
                "file_group_id": "g1", "drone_model_key": "0-89-0", "is_original": true,
                "payload_model_key": "1-61-0", "tinny_fingerprint": "5aec_2026_10_17", "sn": "AC1"
            },
            "fingerprint": "fp1",
            "metadata": {
                "absolute_altitude": 142.3, "created_time": "2026-10-17 14:30:12",
                "gimbal_yaw_degree": 87.4, "relative_altitude": 61,
                "shoot_position": {"lat": 52.9612345, "lng": 10.5612345}
            },
            "name": "DJI_20261017143012_0001_T.JPG",
            "object_key": "fotos/RC1/DJI_20261017143012_0001_T.JPG",
            "path": "",
            "sub_file_type": 0
        });
        let input: UploadCallbackInput = serde_json::from_value(body).unwrap();
        assert_eq!(
            input.ext.tiny_fingerprint.as_deref(),
            Some("5aec_2026_10_17")
        );
        assert_eq!(input.ext.sn.as_deref(), Some("AC1"));
        let pos = input.metadata.shoot_position.unwrap();
        assert_eq!(pos.lat, Some(52.961_234_5));
        assert_eq!(pos.lng, Some(10.561_234_5));
        assert_eq!(input.metadata.relative_altitude, Some(61.0));
        assert_eq!(
            input.metadata.created_time.as_deref(),
            Some("2026-10-17 14:30:12")
        );
    }

    #[test]
    fn only_result_name_and_object_key_are_required() {
        let input: UploadCallbackInput =
            serde_json::from_value(json!({"result": 0, "name": "a.jpg", "object_key": "k"}))
                .unwrap();
        assert!(input.metadata.shoot_position.is_none());
        assert!(input.ext.sn.is_none());
        let input: UploadCallbackInput = serde_json::from_value(json!({
            "result": 0, "name": "a.jpg", "object_key": "k",
            "metadata": {"created_time": 1792248612, "shoot_position": {"lat": "52.5", "lng": null}}
        }))
        .unwrap();
        assert_eq!(input.metadata.created_time.as_deref(), Some("1792248612"));
        let pos = input.metadata.shoot_position.unwrap();
        assert_eq!((pos.lat, pos.lng), (Some(52.5), None));
        assert!(
            serde_json::from_value::<UploadCallbackInput>(json!({"result": 0, "name": "a"}))
                .is_err()
        );
    }

    #[test]
    fn fast_upload_and_tiny_fingerprints_read_the_examples() {
        let fast: FastUploadInput = serde_json::from_value(json!({
            "ext": {"drone_model_key": "0-89-0", "is_original": true, "payload_model_key": "1-61-0",
                    "tinny_fingerprint": "t", "sn": "AC1"},
            "fingerprint": "fp1", "name": "a.jpg", "path": ""
        }))
        .unwrap();
        assert_eq!(fast.fingerprint, "fp1");
        let tiny: TinyFingerprintsInput =
            serde_json::from_value(json!({"tiny_fingerprints": ["a", "b"]})).unwrap();
        assert_eq!(tiny.tiny_fingerprints, ["a", "b"]);
        assert_eq!(
            known_tiny_fingerprints(&["a".into()]),
            json!({"tiny_fingerprints": ["a"]})
        );
        let group: GroupUploadCallbackInput = serde_json::from_value(
            json!({"file_group_id": "xxx", "file_count": 3, "file_uploaded_count": 2}),
        )
        .unwrap();
        assert_eq!((group.file_count, group.file_uploaded_count), (3, 2));
    }

    #[test]
    fn sts_has_the_documented_shape() {
        let out = sts(&Sts {
            endpoint: "http://100.64.0.10:3900",
            bucket: "suchlage-fotos",
            region: "garage",
            object_key_prefix: "fotos/RC1",
            access_key_id: "GK1",
            access_key_secret: "s",
            expire_s: 3600,
        });
        assert_eq!(
            out,
            json!({
                "bucket": "suchlage-fotos",
                "credentials": {"access_key_id": "GK1", "access_key_secret": "s", "expire": 3600, "security_token": ""},
                "endpoint": "http://100.64.0.10:3900",
                "object_key_prefix": "fotos/RC1",
                "provider": "minio",
                "region": "garage"
            })
        );
    }
}

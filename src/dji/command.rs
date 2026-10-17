//! Livestream-Services (Referenz Kap. 5.2). MQTT-`url_type` 1 = RTMP – nicht mit dem
//! JSBridge-Enum (2 = RTMP) verwechseln.

use serde_json::{Value, json};

use crate::contract::Lens;

/// Baut die Nachricht für `thing/product/{gateway}/services`. Die `tid` kommt zurück, weil
/// `services_reply` über sie zugeordnet wird.
pub fn envelope(method: &str, data: Value, at_ms: i64) -> (String, Vec<u8>) {
    let tid = uuid::Uuid::new_v4().to_string();
    let body = json!({
        "tid": tid,
        "bid": uuid::Uuid::new_v4().to_string(),
        "timestamp": at_ms,
        "method": method,
        "data": data,
    });
    (tid, body.to_string().into_bytes())
}

/// `video_quality` 0 = adaptiv: Pilot 2 passt die Bitrate der Funkstrecke an.
pub fn live_start_push(video_id: &str, url: &str) -> Value {
    json!({ "url_type": 1, "url": url, "video_id": video_id, "video_quality": 0 })
}

pub fn live_stop_push(video_id: &str) -> Value {
    json!({ "video_id": video_id })
}

pub fn live_lens_change(video_id: &str, lens: Lens) -> Value {
    let video_type = match lens {
        Lens::Normal => "normal",
        Lens::Wide => "wide",
        Lens::Zoom => "zoom",
        Lens::Thermal => "thermal",
    };
    json!({ "video_id": video_id, "video_type": video_type })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_push_uses_mqtt_rtmp_enum_and_adaptive_quality() {
        assert_eq!(
            live_start_push("AC1/61-0-0/normal-0", "rtmp://h:1935/live/RC1"),
            json!({"url_type": 1, "url": "rtmp://h:1935/live/RC1", "video_id": "AC1/61-0-0/normal-0", "video_quality": 0})
        );
    }

    #[test]
    fn envelope_carries_tid_bid_method_and_timestamp() {
        let (tid, bytes) = envelope("live_stop_push", live_stop_push("v"), 42);
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(v["tid"], tid.as_str());
        assert_eq!(
            (v["method"].as_str(), v["timestamp"].as_i64()),
            (Some("live_stop_push"), Some(42))
        );
        assert_eq!(v["data"], json!({"video_id": "v"}));
        assert!(v["bid"].as_str().unwrap().len() >= 32);
        assert_ne!(v["bid"], v["tid"], "bid und tid sind eigene Kennungen");
    }

    #[test]
    fn lens_uses_dji_spelling() {
        assert_eq!(
            live_lens_change("v", Lens::Thermal),
            json!({"video_id": "v", "video_type": "thermal"})
        );
        assert_eq!(live_lens_change("v", Lens::Wide)["video_type"], "wide");
    }
}

//! Der Browser liest Video nur über diese Proxies, mit seiner Sitzung; zu MediaMTX geht der
//! interne Leser (ADR 0003). Adressen, Cookies und Umleitungen von MediaMTX bleiben intern.

use std::time::Duration;

use axum::body::{Body, Bytes};
use axum::http::{HeaderName, HeaderValue, header};
use axum::response::Response;

use crate::app::AppState;
use crate::contract::{FlightSummary, MediaSettings};
use crate::error::AppError;

use super::hook::INTERNAL_USER;

/// Antwortköpfe, die der Browser von MediaMTX sehen darf. `Set-Cookie` und `Location` nie.
/// `Cache-Control` setzt der Proxy selbst.
const PASSED_HEADERS: [HeaderName; 4] = [
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::ACCEPT_RANGES,
    header::CONTENT_RANGE,
];

fn media(state: &AppState) -> Result<MediaSettings, AppError> {
    state
        .settings
        .current()
        .media
        .ok_or_else(|| AppError::Unavailable("MediaMTX ist nicht eingerichtet".into()))
}

fn unreachable_media(e: reqwest::Error) -> AppError {
    tracing::warn!(error = %e, "MediaMTX nicht erreichbar");
    AppError::Unavailable("Videoserver nicht erreichbar".into())
}

/// Lehnt MediaMTX den internen Leser ab, stimmt die Einrichtung nicht. Als 401 durchgereicht,
/// hielte der Browser seine eigene Sitzung für abgelaufen.
fn reject_refused(res: reqwest::Response) -> Result<reqwest::Response, AppError> {
    match res.status() {
        reqwest::StatusCode::UNAUTHORIZED | reqwest::StatusCode::FORBIDDEN => {
            tracing::warn!(status = %res.status(), "MediaMTX lehnt den internen Leser ab");
            Err(AppError::Unavailable(
                "Videoserver lehnt den Zugang des Servers ab".into(),
            ))
        }
        _ => Ok(res),
    }
}

/// Erlaubt nur `[A-Za-z0-9._-]`-Segmente, nie `..`.
pub fn safe_file(file: &str) -> bool {
    !file.is_empty()
        && file.split('/').all(|seg| {
            !seg.is_empty()
                && seg != ".."
                && seg
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        })
}

fn passthrough(res: reqwest::Response, extra: Option<(HeaderName, HeaderValue)>) -> Response {
    // Video hinter der Sitzung: Ein öffentliches `max-age` von MediaMTX (HLS-Segmente) ließe
    // Cloudflare es am Edge zwischenspeichern und ohne Anmeldung ausliefern.
    let mut out = Response::builder()
        .status(res.status())
        .header(header::CACHE_CONTROL, "private, no-cache");
    for name in PASSED_HEADERS {
        if let Some(v) = res.headers().get(&name) {
            out = out.header(name, v.clone());
        }
    }
    if let Some((name, value)) = extra {
        out = out.header(name, value);
    }
    out.body(Body::from_stream(res.bytes_stream()))
        .expect("Status und Köpfe stammen aus einer gültigen Antwort")
}

async fn send_internal(
    state: &AppState,
    req: reqwest::RequestBuilder,
) -> Result<reqwest::Response, AppError> {
    let res = req
        .basic_auth(
            INTERNAL_USER,
            Some(state.settings.current().internal_secret),
        )
        .send()
        .await
        .map_err(unreachable_media)?;
    reject_refused(res)
}

/// Die eingerichteten Adressen sind geprüft (`settings::address`); scheitert das Zusammensetzen
/// trotzdem, ist es ein Fehler im Server.
fn endpoint(base: &str, path: &str, query: &[(&str, String)]) -> Result<url::Url, AppError> {
    url::Url::parse_with_params(&format!("{base}{path}"), query)
        .map_err(|e| AppError::Internal(format!("MediaMTX-Adresse {base}{path}: {e}")))
}

pub async fn whep(state: &AppState, gateway_sn: &str, sdp: Bytes) -> Result<Response, AppError> {
    let m = media(state)?;
    let req = state
        .http
        .post(format!("{}/live/{gateway_sn}/whep", m.webrtc_url))
        .header(header::CONTENT_TYPE, "application/sdp")
        .timeout(Duration::from_secs(10))
        .body(sdp);
    let res = send_internal(state, req).await?;
    // 404: Pilot 2 sendet noch nicht. Der Player wartet dann, statt auf HLS zu wechseln (F4).
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(AppError::NotFound);
    }
    // shortcut: ohne Trickle-ICE (alle Kandidaten im Offer), daher kein PATCH/DELETE-Proxy; MediaMTX räumt Sitzungen selbst ab.
    Ok(passthrough(res, None))
}

pub async fn hls(
    state: &AppState,
    gateway_sn: &str,
    file: &str,
    query: &str,
) -> Result<Response, AppError> {
    if !safe_file(file) {
        return Err(AppError::NotFound);
    }
    let m = media(state)?;
    let mut url = format!("{}/live/{gateway_sn}/{file}", m.hls_url);
    if !query.is_empty() {
        url.push('?');
        url.push_str(query);
    }
    let req = state.http.get(url).timeout(Duration::from_secs(15));
    Ok(passthrough(send_internal(state, req).await?, None))
}

fn rfc3339(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or_default()
        .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Was MediaMTX im Zeitfenster des Flugs aufgezeichnet hat: Beginn (ms) und Dauer (s). Beginnt bei
/// `max(Flugstart, erste Aufnahme)`, denn das Livebild startet erst nach dem Abheben.
/// `Ok(None)`: Flug läuft noch oder es gibt keine Aufnahme.
async fn recording_window(
    state: &AppState,
    m: &MediaSettings,
    flight: &FlightSummary,
) -> Result<Option<(i64, i64)>, AppError> {
    let Some(end) = flight.ended_at_ms else {
        return Ok(None);
    };
    let url = endpoint(
        &m.playback_url,
        "/list",
        &[
            ("path", format!("live/{}", flight.gateway_sn)),
            ("start", rfc3339(flight.started_at_ms)),
            ("end", rfc3339(end)),
        ],
    )?;
    let res = send_internal(state, state.http.get(url).timeout(Duration::from_secs(5))).await?;
    // MediaMTX meldet 404, wenn im Fenster nichts aufgezeichnet ist.
    if !res.status().is_success() {
        return Ok(None);
    }
    let list = res
        .json::<Vec<serde_json::Value>>()
        .await
        .map_err(unreachable_media)?;
    let first = list
        .first()
        .and_then(|r| r["start"].as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        // Auf die volle Sekunde aufgerundet: `rfc3339` schneidet Bruchteile ab, und `/get` vor
        // dem ersten Segment findet nichts.
        .map(|t| (t.timestamp() + i64::from(t.timestamp_subsec_nanos() > 0)) * 1000);
    let Some(first) = first else {
        return Ok(None);
    };
    let start = flight.started_at_ms.max(first);
    Ok((end > start).then_some((start, ((end - start) / 1000).max(1))))
}

/// Für das Flugdetail: Beginn des Videos (ms), sofern es eins gibt. Ist MediaMTX nicht
/// erreichbar, fehlt nur das Video, das Detail bleibt.
pub async fn flight_video_start(state: &AppState, flight: &FlightSummary) -> Option<i64> {
    let m = media(state).ok()?;
    recording_window(state, &m, flight)
        .await
        .ok()
        .flatten()
        .map(|(start, _)| start)
}

pub async fn flight_video(
    state: &AppState,
    flight: &FlightSummary,
    download: bool,
    range: Option<HeaderValue>,
) -> Result<Response, AppError> {
    let m = media(state)?;
    if flight.ended_at_ms.is_none() {
        return Err(AppError::Unprocessable(
            "Flug läuft noch – das Video gibt es nach der Landung".into(),
        ));
    }
    let (start, duration) = recording_window(state, &m, flight)
        .await?
        .ok_or(AppError::NotFound)?;
    let url = endpoint(
        &m.playback_url,
        "/get",
        &[
            ("path", format!("live/{}", flight.gateway_sn)),
            ("start", rfc3339(start)),
            ("duration", duration.to_string()),
            ("format", "mp4".to_string()),
        ],
    )?;
    let mut req = state.http.get(url);
    if let Some(range) = range {
        req = req.header(header::RANGE, range);
    }
    let res = send_internal(state, req).await?;
    if !res.status().is_success() {
        return Err(AppError::NotFound);
    }
    let disposition = if download {
        HeaderValue::from_str(&format!(
            "attachment; filename=\"suchlage-flug-{}.mp4\"",
            flight.id
        ))
        .expect("Dateiname aus Ziffern ist ein gültiger Kopf")
    } else {
        HeaderValue::from_static("inline")
    };
    Ok(passthrough(
        res,
        Some((header::CONTENT_DISPOSITION, disposition)),
    ))
}

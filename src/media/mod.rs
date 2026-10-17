//! Video über MediaMTX: Auth-Hook und Proxies für WHEP, HLS und Playback (ADR 0003).

pub mod hook;
pub mod proxy;

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(3))
        // MediaMTX leitet die erste HLS-Anfrage auf ?cookieCheck=1 um (M0-Befund); über http fällt es
        // für die Sitzung auf den Query-Parameter zurück, den die Playlist selbst mitführt.
        .redirect(reqwest::redirect::Policy::limited(3))
        .build()
        .expect("HTTP-Client baubar")
}

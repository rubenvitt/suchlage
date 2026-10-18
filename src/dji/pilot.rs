//! Anmeldeseite für DJI Pilot 2 (aus M0). Liefert die Zugangsdaten nur gegen einen gültigen
//! Kopplungscode – die Seite ist öffentlich erreichbar (Spec §6). Zwei Schritte, weil Pilot 2 die
//! Seriennummer erst nach der Lizenzprüfung herausgibt.

use serde::{Deserialize, Serialize};

pub const INDEX_HTML: &str = include_str!("pilot/index.html");
pub const PILOT_JS: &str = include_str!("pilot/pilot.js");

/// Schritt 1 (`/pilot/api/license`). Feldnamen wie in M0 (`config.js`).
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PilotLicense {
    pub app_id: String,
    pub app_key: String,
    pub license: String,
    pub workspace_id: String,
    pub rc_label: String,
}

/// Schritt 2 (`/pilot/api/connect`): Zugang dieser einen Fernsteuerung. `suchlage-sim` liest
/// dieselbe Antwort.
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PilotConnection {
    pub mqtt_host: String,
    pub mqtt_user: String,
    pub mqtt_pass: String,
    pub rtmp_url: String,
    /// Modul `api`: Pilot 2 hängt `/map/api/v1/…` an.
    pub api_host: String,
    /// Modul `ws`: Kartenelemente-Pushes (`wss://…/pilot/ws`).
    pub ws_host: String,
    /// Für `api` und `ws`. Jedes Koppeln erzeugt einen neuen.
    pub api_token: String,
    /// Modul `media` laden (M3): Der Server hat einen Fotospeicher. Fehlt bei älteren Servern.
    #[serde(default)]
    pub media: bool,
}

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::live::Lens;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum Role {
    Admin,
    Operator,
    Kiosk,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Me {
    pub display_name: String,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct AuthOptions {
    pub oidc: bool,
    pub dev_login: bool,
    pub setup_required: bool,
    /// In Pocket ID einzutragende Callback-URL, aus `SUCHLAGE_PUBLIC_URL` – nie aus der Browser-Adresse
    /// (hinter Proxy oder Vite weicht die ab).
    pub redirect_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SetupRequest {
    pub token: String,
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SetupResult {
    pub login_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OidcSettingsView {
    pub issuer: String,
    pub client_id: String,
    pub has_client_secret: bool,
    /// In Pocket ID als Callback-URL einzutragen.
    pub redirect_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct OidcSettingsInput {
    pub issuer: String,
    pub client_id: String,
    /// `null` behält das gespeicherte Geheimnis.
    pub client_secret: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DjiSettingsView {
    pub app_id: String,
    pub has_app_key: bool,
    pub has_license: bool,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct DjiSettingsInput {
    pub app_id: String,
    pub app_key: Option<String>,
    pub license: Option<String>,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BrokerSettingsView {
    /// Adresse, unter der der Server den Broker erreicht (Dev: 127.0.0.1, Betrieb: mqtt).
    pub host: String,
    pub port: u16,
    pub admin_username: String,
    pub has_admin_password: bool,
    /// Adresse, die die Fernsteuerung bekommt, z. B. `tcp://100.64.0.10:1883`.
    pub rc_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct BrokerSettingsInput {
    pub host: String,
    pub port: u16,
    pub admin_username: String,
    pub admin_password: Option<String>,
    pub rc_url: String,
}

/// MediaMTX-Adressen. Keine Geheimnisse – der Zugriff läuft über den Auth-Hook (ADR 0003).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct MediaSettings {
    /// Interne WHEP-Basis, z. B. `http://media:8889`.
    pub webrtc_url: String,
    /// Interne HLS-Basis, z. B. `http://media:8888`.
    pub hls_url: String,
    /// Interner Playback-Server, z. B. `http://media:9996`.
    pub playback_url: String,
    /// RTMP-Basis für die Fernsteuerung, z. B. `rtmp://100.64.0.10:1935`.
    pub rtmp_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct GeneralSettings {
    /// Beim Start eines Flugs automatisch das Livebild anfordern.
    pub auto_live_on_takeoff: bool,
    /// Rohe MQTT-Nachrichten als JSONL mitschneiden.
    pub record_raw_mqtt: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct SettingsView {
    pub oidc: Option<OidcSettingsView>,
    pub dji: Option<DjiSettingsView>,
    pub broker: Option<BrokerSettingsView>,
    pub media: Option<MediaSettings>,
    pub general: GeneralSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemoteControllerView {
    #[ts(type = "number")]
    pub id: i64,
    pub label: String,
    /// Erst nach der ersten Anmeldung aus Pilot 2 bekannt.
    pub sn: Option<String>,
    #[ts(type = "number | null")]
    pub paired_at_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemoteControllerCreate {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct RemoteControllerCreated {
    pub controller: RemoteControllerView,
    /// Nur einmal sichtbar: `https://<host>/pilot#<code>`. Fragment, damit der Code in keinem Log steht.
    pub pairing_url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KioskTokenView {
    #[ts(type = "number")]
    pub id: i64,
    pub label: String,
    #[ts(type = "number")]
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KioskTokenCreate {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KioskTokenCreated {
    pub token: KioskTokenView,
    /// Nur einmal sichtbar: `https://<host>/kiosk#<token>`.
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct KioskEnroll {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamStartRequest {
    pub gateway_sn: String,
    /// `null`: Standardquelle (erste Nutzlastkamera, sonst FPV).
    pub video_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamStopRequest {
    pub gateway_sn: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct StreamLensRequest {
    pub gateway_sn: String,
    pub lens: Lens,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct ApiError {
    pub error: String,
}

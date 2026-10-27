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

/// Antwort von `/api/health`: zeigt, welcher Stand läuft.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct Health {
    pub status: String,
    /// Release-Version wie im Git-Tag ohne `v`, lokal `<Cargo-Version>-dev`.
    pub version: String,
    /// Kurzer Commit des Builds, `null` außerhalb der Release-Pipeline.
    pub commit: Option<String>,
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
    pub admin_group: String,
    pub user_group: String,
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
    /// Pocket-ID-Gruppe der Verwaltung (Rolle Admin).
    pub admin_group: String,
    /// Pocket-ID-Gruppe der Nutzung (Rolle Operator).
    pub user_group: String,
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
    pub admin_group: String,
    pub user_group: String,
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

/// Fotospeicher (Garage, M3). Geheimnisse verlassen den Server nie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PhotoStorageSettingsView {
    /// S3-Adresse, unter der der Server Garage erreicht, z. B. `http://fotos:3900`.
    pub s3_url: String,
    /// S3-Adresse für die Fernsteuerung (nur im VPN), z. B. `http://100.64.0.10:3900`.
    pub rc_url: String,
    /// Admin-API von Garage, z. B. `http://fotos:3903`.
    pub admin_url: String,
    pub bucket: String,
    pub region: String,
    /// Schlüssel des Servers (Lesen und Schreiben im Bucket).
    pub access_key_id: String,
    pub has_secret_access_key: bool,
    /// Eingeschränkter Admin-Token (nur Schlüssel anlegen, freigeben, löschen).
    pub has_admin_token: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct PhotoStorageSettingsInput {
    pub s3_url: String,
    pub rc_url: String,
    pub admin_url: String,
    pub bucket: String,
    pub region: String,
    pub access_key_id: String,
    /// `null` behält das gespeicherte Geheimnis.
    pub secret_access_key: Option<String>,
    /// `null` behält den gespeicherten Token.
    pub admin_token: Option<String>,
}

/// Schnittstelle für die Flugmappe (SUL-66). Keine Geheimnisse. Fehlt sie, ist die Schnittstelle
/// aus (503); ein leeres Formular entfernt sie.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[ts(export)]
pub struct FlugmappeSettings {
    /// Client-ID der Flugmappe in Pocket ID; ihre Zugriffstokens tragen sie in `aud`.
    pub client_id: String,
    /// Adresse der Flugmappe für CORS, z. B. `https://uav.example.org` (ohne Pfad).
    pub allowed_origin: String,
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
    pub photo_storage: Option<PhotoStorageSettingsView>,
    pub flugmappe: Option<FlugmappeSettings>,
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

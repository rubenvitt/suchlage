//! Fachliche Einstellungen leben in der DB (ADR 0002). Geheimnisse verlassen den Server nie –
//! die Sicht (`SettingsView`) meldet nur `has_*`.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::watch;

use crate::config::BootConfig;
use crate::contract::{
    BrokerSettingsInput, BrokerSettingsView, DjiSettingsInput, DjiSettingsView, FlugmappeSettings,
    GeneralSettings, MediaSettings, OidcSettingsInput, OidcSettingsView, PhotoStorageSettingsInput,
    PhotoStorageSettingsView, SettingsView,
};
use crate::error::AppError;
use crate::store;
use crate::time::now_ms;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OidcSettings {
    pub issuer: String,
    pub client_id: String,
    pub client_secret: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DjiSettings {
    pub app_id: String,
    pub app_key: String,
    pub license: String,
    pub workspace_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrokerSettings {
    pub host: String,
    pub port: u16,
    pub admin_username: String,
    pub admin_password: String,
    pub rc_url: String,
}

/// Fotospeicher (Garage, M3). Ohne ihn nimmt der Server keine Fotos an (503).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhotoStorageSettings {
    pub s3_url: String,
    pub rc_url: String,
    pub admin_url: String,
    pub bucket: String,
    pub region: String,
    pub access_key_id: String,
    pub secret_access_key: String,
    pub admin_token: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub oidc: Option<OidcSettings>,
    pub dji: Option<DjiSettings>,
    pub broker: Option<BrokerSettings>,
    pub media: Option<MediaSettings>,
    pub photo_storage: Option<PhotoStorageSettings>,
    /// Schnittstelle für die Flugmappe (SUL-66); ohne sie ist sie aus.
    pub flugmappe: Option<FlugmappeSettings>,
    pub general: GeneralSettings,
    /// Geheimnis des internen MediaMTX-Lesers (ADR 0003); beim ersten Start erzeugt.
    pub internal_secret: String,
}

pub fn default_general() -> GeneralSettings {
    GeneralSettings {
        auto_live_on_takeoff: true,
        record_raw_mqtt: true,
    }
}

pub fn redirect_url(boot: &BootConfig) -> String {
    format!(
        "{}/api/auth/oidc/callback",
        boot.public_url.as_str().trim_end_matches('/')
    )
}

#[derive(Clone)]
pub struct SettingsHandle {
    pool: SqlitePool,
    tx: watch::Sender<Settings>,
}

fn invalid(msg: &str) -> AppError {
    AppError::Validation(msg.into())
}

fn non_empty(v: &str, field: &str) -> Result<String, AppError> {
    let v = v.trim();
    if v.is_empty() {
        Err(invalid(&format!("{field} darf nicht leer sein")))
    } else {
        Ok(v.to_string())
    }
}

/// Adresse mit einem der Schemata und einem Rechner. Gespeichert wird die Eingabe ohne Rand und
/// ohne Schrägstrich am Ende, nicht die normalisierte Form von `url` (die hängt `/` an).
fn address(v: &str, schemes: &[&str], field: &str) -> Result<String, AppError> {
    let v = non_empty(v, field)?;
    let valid = url::Url::parse(&v).is_ok_and(|u| {
        schemes.contains(&u.scheme()) && u.host_str().is_some_and(|h| !h.is_empty())
    });
    if valid {
        Ok(v.trim_end_matches('/').to_string())
    } else {
        let schemes = schemes
            .iter()
            .map(|s| format!("{s}://"))
            .collect::<Vec<_>>()
            .join(" oder ");
        Err(invalid(&format!(
            "{field} muss mit {schemes} beginnen und einen Rechner nennen"
        )))
    }
}

fn is_uuid(v: &str) -> bool {
    v.len() == 36
        && v.char_indices().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == '-'
            } else {
                c.is_ascii_hexdigit()
            }
        })
}

/// S3-Regel für Bucket-Namen (ohne die Sonderfälle wie IP-Adressen).
fn is_bucket_name(v: &str) -> bool {
    (3..=63).contains(&v.len())
        && v.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '.' || c == '-')
        && v.starts_with(|c: char| c.is_ascii_alphanumeric())
        && v.ends_with(|c: char| c.is_ascii_alphanumeric())
}

/// Ursprung für CORS: `https://Rechner[:Port]` ohne Pfad, Anfrage und Zugangsdaten; http nur für
/// Loopback. Gespeichert in der Form, die Browser im `Origin`-Header schicken.
fn origin(v: &str, field: &str) -> Result<String, AppError> {
    let v = non_empty(v, field)?;
    url::Url::parse(&v)
        .ok()
        .filter(|u| {
            issuer_allowed(u.as_str())
                && u.host_str().is_some_and(|h| !h.is_empty())
                && u.path() == "/"
                && u.query().is_none()
                && u.fragment().is_none()
                && u.username().is_empty()
                && u.password().is_none()
        })
        .map(|u| u.origin().ascii_serialization())
        .ok_or_else(|| {
            invalid(&format!(
                "{field} muss https://Rechner[:Port] ohne Pfad sein (http nur für localhost)"
            ))
        })
}

pub fn issuer_allowed(issuer: &str) -> bool {
    let Ok(url) = url::Url::parse(issuer) else {
        return false;
    };
    match url.scheme() {
        "https" => true,
        "http" => match url.host() {
            Some(url::Host::Domain(d)) => d == "localhost",
            Some(url::Host::Ipv4(ip)) => ip.is_loopback(),
            Some(url::Host::Ipv6(ip)) => ip.is_loopback(),
            None => false,
        },
        _ => false,
    }
}

/// Neues Geheimnis oder das gespeicherte; beim ersten Speichern Pflicht.
fn secret(input: Option<String>, stored: Option<&String>, field: &str) -> Result<String, AppError> {
    match input
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
    {
        Some(s) => Ok(s),
        None => stored
            .cloned()
            .ok_or_else(|| invalid(&format!("{field} fehlt"))),
    }
}

fn oidc_value(
    input: OidcSettingsInput,
    stored: Option<&OidcSettings>,
) -> Result<OidcSettings, AppError> {
    let issuer = non_empty(&input.issuer, "Issuer")?
        .trim_end_matches('/')
        .to_string();
    if !issuer_allowed(&issuer) {
        return Err(invalid(
            "Issuer muss mit https:// beginnen (http nur für localhost)",
        ));
    }
    Ok(OidcSettings {
        issuer,
        client_id: non_empty(&input.client_id, "Client-ID")?,
        client_secret: secret(
            input.client_secret,
            stored.map(|c| &c.client_secret),
            "Client-Secret",
        )?,
    })
}

/// Prüft die Anmeldedaten der Ersteinrichtung, ohne etwas zu speichern (dort gibt es kein
/// gespeichertes Geheimnis).
pub fn validate_oidc(input: &OidcSettingsInput) -> Result<(), AppError> {
    oidc_value(input.clone(), None).map(drop)
}

impl SettingsHandle {
    pub async fn load(pool: SqlitePool) -> Result<Self, sqlx::Error> {
        let internal_secret = match store::settings::get::<String>(&pool, "internal_secret").await?
        {
            Some(s) => s,
            None => {
                let s = crate::token::generate();
                store::settings::put(&pool, "internal_secret", &s, now_ms()).await?;
                s
            }
        };
        let settings = Settings {
            oidc: store::settings::get(&pool, "oidc").await?,
            dji: store::settings::get(&pool, "dji").await?,
            broker: store::settings::get(&pool, "broker").await?,
            media: store::settings::get(&pool, "media").await?,
            photo_storage: store::settings::get(&pool, "photo_storage").await?,
            flugmappe: store::settings::get(&pool, "flugmappe").await?,
            general: store::settings::get(&pool, "general")
                .await?
                .unwrap_or_else(default_general),
            internal_secret,
        };
        Ok(Self {
            pool,
            tx: watch::Sender::new(settings),
        })
    }

    pub fn current(&self) -> Settings {
        self.tx.borrow().clone()
    }

    pub fn subscribe(&self) -> watch::Receiver<Settings> {
        self.tx.subscribe()
    }

    /// Erst in die DB, dann an alle Abonnenten. `send_modify` gelingt auch ohne Empfänger.
    async fn save<T: Serialize>(
        &self,
        key: &str,
        value: &T,
        apply: impl FnOnce(&mut Settings),
    ) -> Result<(), AppError> {
        store::settings::put(&self.pool, key, value, now_ms()).await?;
        self.tx.send_modify(apply);
        Ok(())
    }

    pub async fn update_oidc(&self, input: OidcSettingsInput) -> Result<(), AppError> {
        let value = oidc_value(input, self.current().oidc.as_ref())?;
        self.save("oidc", &value, |s| s.oidc = Some(value.clone()))
            .await
    }

    pub async fn update_dji(&self, input: DjiSettingsInput) -> Result<(), AppError> {
        let workspace_id = input.workspace_id.trim().to_lowercase();
        if !is_uuid(&workspace_id) {
            return Err(invalid("Workspace-ID muss eine UUID sein"));
        }
        let current = self.current().dji;
        let value = DjiSettings {
            app_id: non_empty(&input.app_id, "App-ID")?,
            app_key: secret(
                input.app_key,
                current.as_ref().map(|c| &c.app_key),
                "App-Key",
            )?,
            license: secret(
                input.license,
                current.as_ref().map(|c| &c.license),
                "Lizenz",
            )?,
            workspace_id,
        };
        self.save("dji", &value, |s| s.dji = Some(value.clone()))
            .await
    }

    pub async fn update_broker(&self, input: BrokerSettingsInput) -> Result<(), AppError> {
        if input.port == 0 {
            return Err(invalid("Port muss zwischen 1 und 65535 liegen"));
        }
        let current = self.current().broker;
        let value = BrokerSettings {
            host: non_empty(&input.host, "Broker-Adresse")?,
            port: input.port,
            admin_username: non_empty(&input.admin_username, "Admin-Benutzer")?,
            admin_password: secret(
                input.admin_password,
                current.as_ref().map(|c| &c.admin_password),
                "Admin-Passwort",
            )?,
            rc_url: address(&input.rc_url, &["tcp"], "Adresse für die Fernsteuerung")?,
        };
        self.save("broker", &value, |s| s.broker = Some(value.clone()))
            .await
    }

    pub async fn update_media(&self, input: MediaSettings) -> Result<(), AppError> {
        let web = ["http", "https"];
        let value = MediaSettings {
            webrtc_url: address(&input.webrtc_url, &web, "WebRTC-Adresse")?,
            hls_url: address(&input.hls_url, &web, "HLS-Adresse")?,
            playback_url: address(&input.playback_url, &web, "Playback-Adresse")?,
            rtmp_url: address(&input.rtmp_url, &["rtmp"], "RTMP-Adresse")?,
        };
        self.save("media", &value, |s| s.media = Some(value.clone()))
            .await
    }

    pub async fn update_photo_storage(
        &self,
        input: PhotoStorageSettingsInput,
    ) -> Result<(), AppError> {
        let web = ["http", "https"];
        let bucket = non_empty(&input.bucket, "Bucket")?;
        if !is_bucket_name(&bucket) {
            return Err(invalid(
                "Bucket: 3–63 Zeichen, Kleinbuchstaben, Ziffern, Punkt und Bindestrich",
            ));
        }
        let current = self.current().photo_storage;
        let value = PhotoStorageSettings {
            s3_url: address(&input.s3_url, &web, "S3-Adresse")?,
            rc_url: address(&input.rc_url, &web, "S3-Adresse für die Fernsteuerung")?,
            admin_url: address(&input.admin_url, &web, "Admin-Adresse")?,
            bucket,
            region: non_empty(&input.region, "Region")?,
            access_key_id: non_empty(&input.access_key_id, "Schlüssel-ID")?,
            secret_access_key: secret(
                input.secret_access_key,
                current.as_ref().map(|c| &c.secret_access_key),
                "Geheimer Schlüssel",
            )?,
            admin_token: secret(
                input.admin_token,
                current.as_ref().map(|c| &c.admin_token),
                "Admin-Token",
            )?,
        };
        self.save("photo_storage", &value, |s| {
            s.photo_storage = Some(value.clone())
        })
        .await
    }

    /// Beide Felder leer: Die Schnittstelle wird ausgeschaltet.
    pub async fn update_flugmappe(&self, input: FlugmappeSettings) -> Result<(), AppError> {
        if input.client_id.trim().is_empty() && input.allowed_origin.trim().is_empty() {
            return self.remove_flugmappe().await;
        }
        let value = FlugmappeSettings {
            client_id: non_empty(&input.client_id, "Client-ID der Flugmappe")?,
            allowed_origin: origin(&input.allowed_origin, "Adresse der Flugmappe")?,
        };
        self.save("flugmappe", &value, |s| s.flugmappe = Some(value.clone()))
            .await
    }

    pub async fn remove_flugmappe(&self) -> Result<(), AppError> {
        store::settings::delete(&self.pool, "flugmappe").await?;
        self.tx.send_modify(|s| s.flugmappe = None);
        Ok(())
    }

    pub async fn update_general(&self, input: GeneralSettings) -> Result<(), AppError> {
        self.save("general", &input, |s| s.general = input.clone())
            .await
    }

    pub fn view(&self, boot: &BootConfig) -> SettingsView {
        let s = self.current();
        SettingsView {
            oidc: s.oidc.map(|o| OidcSettingsView {
                issuer: o.issuer,
                client_id: o.client_id,
                has_client_secret: !o.client_secret.is_empty(),
                redirect_url: redirect_url(boot),
            }),
            dji: s.dji.map(|d| DjiSettingsView {
                app_id: d.app_id,
                has_app_key: !d.app_key.is_empty(),
                has_license: !d.license.is_empty(),
                workspace_id: d.workspace_id,
            }),
            broker: s.broker.map(|b| BrokerSettingsView {
                host: b.host,
                port: b.port,
                admin_username: b.admin_username,
                has_admin_password: !b.admin_password.is_empty(),
                rc_url: b.rc_url,
            }),
            media: s.media,
            photo_storage: s.photo_storage.map(|p| PhotoStorageSettingsView {
                s3_url: p.s3_url,
                rc_url: p.rc_url,
                admin_url: p.admin_url,
                bucket: p.bucket,
                region: p.region,
                access_key_id: p.access_key_id,
                has_secret_access_key: !p.secret_access_key.is_empty(),
                has_admin_token: !p.admin_token.is_empty(),
            }),
            flugmappe: s.flugmappe,
            general: s.general,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(webrtc_url: &str) -> MediaSettings {
        MediaSettings {
            webrtc_url: webrtc_url.into(),
            hls_url: "http://media:8888".into(),
            playback_url: "http://media:9996".into(),
            rtmp_url: "rtmp://100.64.0.10:1935".into(),
        }
    }

    #[tokio::test]
    async fn secrets_are_kept_when_omitted_and_never_shown() {
        let pool = crate::db::test_pool().await;
        let h = SettingsHandle::load(pool.clone()).await.unwrap();
        let input = |secret: Option<&str>| OidcSettingsInput {
            issuer: "https://id.example.org".into(),
            client_id: "suchlage".into(),
            client_secret: secret.map(Into::into),
        };
        assert!(
            matches!(
                h.update_oidc(input(None)).await,
                Err(AppError::Validation(_))
            ),
            "erstes Speichern braucht das Geheimnis"
        );
        h.update_oidc(input(Some("geheim"))).await.unwrap();
        h.update_oidc(input(None)).await.unwrap();
        h.update_oidc(input(Some("  "))).await.unwrap();
        assert_eq!(h.current().oidc.unwrap().client_secret, "geheim");

        let boot = BootConfig::from_lookup(|_| None).unwrap();
        let json = serde_json::to_string(&h.view(&boot)).unwrap();
        assert!(!json.contains("geheim"));
        assert!(json.contains("\"has_client_secret\":true"));
        assert!(json.contains("http://127.0.0.1:8080/api/auth/oidc/callback"));
    }

    #[tokio::test]
    async fn settings_survive_a_reload_and_internal_secret_is_stable() {
        let pool = crate::db::test_pool().await;
        let h = SettingsHandle::load(pool.clone()).await.unwrap();
        assert_eq!(h.current().general, default_general());
        h.update_general(GeneralSettings {
            auto_live_on_takeoff: false,
            record_raw_mqtt: true,
        })
        .await
        .unwrap();
        h.update_media(media("http://media:8889/")).await.unwrap();
        let secret = h.current().internal_secret;
        assert_eq!(secret.len(), 64);
        let again = SettingsHandle::load(pool).await.unwrap();
        assert!(!again.current().general.auto_live_on_takeoff);
        assert_eq!(
            again.current().media.unwrap().webrtc_url,
            "http://media:8889",
            "ohne Schrägstrich am Ende"
        );
        assert_eq!(again.current().internal_secret, secret);
    }

    #[tokio::test]
    async fn invalid_values_are_rejected_with_a_reason() {
        let h = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        let err = h
            .update_oidc(OidcSettingsInput {
                issuer: "http://id.example.org".into(),
                client_id: "x".into(),
                client_secret: Some("s".into()),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("https"), "{err}");
        let err = h
            .update_broker(BrokerSettingsInput {
                host: "mqtt".into(),
                port: 1883,
                admin_username: "a".into(),
                admin_password: Some("p".into()),
                rc_url: "mqtt://x".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("tcp://"), "{err}");
        let err = h
            .update_broker(BrokerSettingsInput {
                host: "mqtt".into(),
                port: 0,
                admin_username: "a".into(),
                admin_password: Some("p".into()),
                rc_url: "tcp://x:1883".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Port"), "{err}");
        let err = h
            .update_dji(DjiSettingsInput {
                app_id: "1".into(),
                app_key: Some("k".into()),
                license: Some("l".into()),
                workspace_id: "keine-uuid".into(),
            })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Workspace"), "{err}");
        let err = h.update_media(media("tcp://media:8889")).await.unwrap_err();
        assert!(err.to_string().contains("WebRTC"), "{err}");
        assert_eq!(
            h.current().broker,
            None,
            "Abgelehntes wird nicht gespeichert"
        );
        assert_eq!(h.current().media, None);
    }

    fn photo_storage(bucket: &str, secret: Option<&str>) -> PhotoStorageSettingsInput {
        PhotoStorageSettingsInput {
            s3_url: "http://fotos:3900/".into(),
            rc_url: "http://100.64.0.10:3900".into(),
            admin_url: "http://fotos:3903".into(),
            bucket: bucket.into(),
            region: "garage".into(),
            access_key_id: "GK1".into(),
            secret_access_key: secret.map(Into::into),
            admin_token: secret.map(|s| format!("{s}-admin")),
        }
    }

    #[tokio::test]
    async fn photo_storage_keeps_secrets_and_checks_the_bucket() {
        let h = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        let err = h
            .update_photo_storage(photo_storage("suchlage-fotos", None))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Geheimer Schlüssel"), "{err}");
        for bad in ["Fotos", "ab", "-fotos", "fotos_1"] {
            let err = h
                .update_photo_storage(photo_storage(bad, Some("s")))
                .await
                .unwrap_err();
            assert!(err.to_string().contains("Bucket"), "{bad}: {err}");
        }
        h.update_photo_storage(photo_storage("suchlage-fotos", Some("geheim")))
            .await
            .unwrap();
        h.update_photo_storage(photo_storage("suchlage-fotos", None))
            .await
            .unwrap();
        let stored = h.current().photo_storage.unwrap();
        assert_eq!(stored.secret_access_key, "geheim");
        assert_eq!(stored.admin_token, "geheim-admin");
        assert_eq!(
            stored.s3_url, "http://fotos:3900",
            "ohne Schrägstrich am Ende"
        );

        let boot = BootConfig::from_lookup(|_| None).unwrap();
        let json = serde_json::to_string(&h.view(&boot)).unwrap();
        assert!(!json.contains("geheim"), "{json}");
        assert!(json.contains("\"has_admin_token\":true"));
    }

    #[test]
    fn addresses_need_scheme_and_host() {
        for ok in ["tcp://100.64.0.10:1883", "tcp://mqtt:1883/"] {
            assert!(address(ok, &["tcp"], "x").is_ok(), "{ok}");
        }
        for bad in ["tcp://", "tcp:100.64.0.10", "100.64.0.10:1883", "  "] {
            assert!(address(bad, &["tcp"], "x").is_err(), "{bad}");
        }
        assert!(
            issuer_allowed("http://127.0.0.1:9000"),
            "Loopback für Tests und Dev"
        );
        assert!(issuer_allowed("http://localhost:1411"));
        assert!(issuer_allowed("http://[::1]:1411"));
        assert!(!issuer_allowed("http://10.0.0.1"));
        assert!(!issuer_allowed("ftp://id.example.org"));
        assert!(is_uuid("768d4a2e-1f34-477c-9fcc-872285e895f6"));
        assert!(!is_uuid("768d4a2e1f34477c9fcc872285e895f6"));
    }

    #[test]
    fn setup_input_is_validated_without_a_stored_secret() {
        let input = |issuer: &str, secret: Option<&str>| OidcSettingsInput {
            issuer: issuer.into(),
            client_id: "suchlage".into(),
            client_secret: secret.map(Into::into),
        };
        assert!(validate_oidc(&input("https://id.example.org", Some("s"))).is_ok());
        assert!(validate_oidc(&input("https://id.example.org", None)).is_err());
        assert!(validate_oidc(&input("https://id.example.org", Some(" "))).is_err());
        assert!(validate_oidc(&input("id.example.org", Some("s"))).is_err());
    }

    fn flugmappe(client_id: &str, allowed_origin: &str) -> FlugmappeSettings {
        FlugmappeSettings {
            client_id: client_id.into(),
            allowed_origin: allowed_origin.into(),
        }
    }

    #[tokio::test]
    async fn flugmappe_is_validated_survives_a_reload_and_an_empty_form_switches_it_off() {
        let pool = crate::db::test_pool().await;
        let h = SettingsHandle::load(pool.clone()).await.unwrap();
        assert_eq!(h.current().flugmappe, None);
        h.update_flugmappe(flugmappe(" flugmappe ", "https://UAV.example.org:443/"))
            .await
            .unwrap();
        assert_eq!(
            h.current().flugmappe,
            Some(flugmappe("flugmappe", "https://uav.example.org")),
            "so, wie der Browser den Ursprung schickt"
        );
        let again = SettingsHandle::load(pool.clone()).await.unwrap();
        assert_eq!(again.current().flugmappe, h.current().flugmappe);
        let boot = BootConfig::from_lookup(|_| None).unwrap();
        assert_eq!(
            h.view(&boot).flugmappe.unwrap().allowed_origin,
            "https://uav.example.org"
        );

        h.update_flugmappe(flugmappe("  ", "")).await.unwrap();
        assert_eq!(h.current().flugmappe, None);
        assert_eq!(
            SettingsHandle::load(pool)
                .await
                .unwrap()
                .current()
                .flugmappe,
            None
        );
    }

    #[tokio::test]
    async fn flugmappe_origin_is_scheme_host_and_port_only() {
        let h = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        for ok in [
            "https://uav.example.org",
            "https://uav.example.org:8443",
            "http://localhost:5173",
            "http://127.0.0.1:5173/",
        ] {
            h.update_flugmappe(flugmappe("fm", ok)).await.unwrap();
        }
        assert_eq!(
            h.current().flugmappe.unwrap().allowed_origin,
            "http://127.0.0.1:5173"
        );
        for bad in [
            "uav.example.org",
            "http://uav.example.org",
            "https://uav.example.org/flugmappe",
            "https://uav.example.org/?x=1",
            "https://user@uav.example.org",
            "ftp://uav.example.org",
        ] {
            let err = h.update_flugmappe(flugmappe("fm", bad)).await.unwrap_err();
            assert!(
                err.to_string().contains("Adresse der Flugmappe"),
                "{bad}: {err}"
            );
        }
        let err = h
            .update_flugmappe(flugmappe(" ", "https://uav.example.org"))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Client-ID"), "{err}");
        h.remove_flugmappe().await.unwrap();
        assert_eq!(h.current().flugmappe, None);
    }

    #[tokio::test]
    async fn changes_are_broadcast() {
        let h = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        let mut rx = h.subscribe();
        h.update_general(GeneralSettings {
            auto_live_on_takeoff: false,
            record_raw_mqtt: false,
        })
        .await
        .unwrap();
        rx.changed().await.unwrap();
        assert!(!rx.borrow().general.record_raw_mqtt);
    }
}

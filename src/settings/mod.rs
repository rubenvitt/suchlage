//! Fachliche Einstellungen leben in der DB (ADR 0002). Geheimnisse verlassen den Server nie –
//! die Sicht (`SettingsView`) meldet nur `has_*`.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use tokio::sync::watch;

use crate::config::BootConfig;
use crate::contract::{
    BrokerSettingsInput, BrokerSettingsView, DjiSettingsInput, DjiSettingsView, GeneralSettings,
    MediaSettings, OidcSettingsInput, OidcSettingsView, SettingsView,
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

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub oidc: Option<OidcSettings>,
    pub dji: Option<DjiSettings>,
    pub broker: Option<BrokerSettings>,
    pub media: Option<MediaSettings>,
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

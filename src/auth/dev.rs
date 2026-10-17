//! Nur mit Feature `dev`: Anmeldung ohne OIDC als „Entwicklung“ (Admin, weil erstes Konto) und
//! Seed der Einstellungen für den lokalen Stack aus `deploy/compose.dev.yaml`.

use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::extract::CookieJar;

use crate::app::AppState;
use crate::auth::session::{secure_cookies, session_cookie};
use crate::contract::{BrokerSettingsInput, DjiSettingsInput, MediaSettings};
use crate::error::AppError;
use crate::settings::SettingsHandle;
use crate::store::{sessions, users};
use crate::time::now_ms;

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    let now = now_ms();
    let user = users::upsert_oidc(&state.pool, "dev", "dev", "Entwicklung", now).await?;
    let token = sessions::create_for_user(&state.pool, user.id, now).await?;
    let cookie = session_cookie(token, sessions::USER_TTL_MS, secure_cookies(&state.boot));
    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

/// Feste Dev-Werte für Broker, MediaMTX und DJI-App. Gesetzt wird nur, was fehlt: In der App
/// geänderte Werte überleben jeden Neustart.
pub async fn seed(settings: &SettingsHandle) -> Result<(), AppError> {
    let s = settings.current();
    if s.broker.is_none() {
        settings
            .update_broker(BrokerSettingsInput {
                host: "127.0.0.1".into(),
                port: 1883,
                admin_username: "suchlage-admin".into(),
                admin_password: Some("dev-admin".into()),
                rc_url: "tcp://127.0.0.1:1883".into(),
            })
            .await?;
    }
    if s.media.is_none() {
        settings
            .update_media(MediaSettings {
                webrtc_url: "http://127.0.0.1:8889".into(),
                hls_url: "http://127.0.0.1:8888".into(),
                playback_url: "http://127.0.0.1:9996".into(),
                rtmp_url: "rtmp://127.0.0.1:1935".into(),
            })
            .await?;
    }
    if s.dji.is_none() {
        // Workspace-ID aus dem M0-Spike (deploy/pilot/config.tpl.js).
        settings
            .update_dji(DjiSettingsInput {
                app_id: "dev".into(),
                app_key: Some("dev".into()),
                license: Some("dev".into()),
                workspace_id: "768d4a2e-1f34-477c-9fcc-872285e895f6".into(),
            })
            .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn seed_fills_only_what_is_missing() {
        let settings = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        settings
            .update_broker(BrokerSettingsInput {
                host: "mqtt".into(),
                port: 1884,
                admin_username: "eigener-admin".into(),
                admin_password: Some("eigenes-passwort".into()),
                rc_url: "tcp://100.64.0.10:1884".into(),
            })
            .await
            .unwrap();
        let before = settings.current().broker;
        seed(&settings).await.unwrap();
        let after = settings.current();
        assert_eq!(after.broker, before, "in der App gesetzter Broker bleibt");
        assert_eq!(after.media.unwrap().webrtc_url, "http://127.0.0.1:8889");
        let dji = after.dji.unwrap();
        assert_eq!(dji.app_id, "dev");
        assert_eq!(dji.workspace_id, "768d4a2e-1f34-477c-9fcc-872285e895f6");
        assert_eq!(after.oidc, None, "Dev meldet sich ohne OIDC an");

        let fresh = SettingsHandle::load(crate::db::test_pool().await)
            .await
            .unwrap();
        seed(&fresh).await.unwrap();
        let broker = fresh.current().broker.unwrap();
        assert_eq!(
            (
                broker.host.as_str(),
                broker.port,
                broker.admin_username.as_str()
            ),
            ("127.0.0.1", 1883, "suchlage-admin")
        );
        assert_eq!(broker.admin_password, "dev-admin");
        assert_eq!(broker.rc_url, "tcp://127.0.0.1:1883");
    }
}

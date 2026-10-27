//! Ersteinrichtung ohne Env (ADR 0002): ohne OIDC-Einstellungen und ohne Konto erzeugt der Server
//! einen einmaligen Token (24 h) und schreibt den Link ins Log.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::config::BootConfig;
use crate::error::AppError;
use crate::settings::SettingsHandle;
use crate::{store, token};

pub const TTL_MS: i64 = 24 * 3_600_000;
const KEY: &str = "setup_token";

#[derive(Serialize, Deserialize)]
struct Stored {
    hash: String,
    expires_at_ms: i64,
}

pub async fn required(pool: &SqlitePool, settings: &SettingsHandle) -> Result<bool, AppError> {
    Ok(!cfg!(feature = "dev")
        && settings.current().oidc.is_none()
        && store::users::count(pool).await? == 0)
}

/// Erzeugt bei Bedarf einen neuen Token und gibt den Link zurück (zum Loggen).
pub async fn ensure_token(
    pool: &SqlitePool,
    settings: &SettingsHandle,
    boot: &BootConfig,
    now_ms: i64,
) -> Result<Option<String>, AppError> {
    if !required(pool, settings).await? {
        return Ok(None);
    }
    let secret = token::generate();
    let stored = Stored {
        hash: token::hash(&secret),
        expires_at_ms: now_ms + TTL_MS,
    };
    store::settings::put(pool, KEY, &stored, now_ms).await?;
    Ok(Some(format!(
        "{}/einrichtung#{secret}",
        boot.public_url.as_str().trim_end_matches('/')
    )))
}

/// Prüft den Token, ohne ihn zu verbrauchen.
pub async fn verify(pool: &SqlitePool, presented: &str, now_ms: i64) -> Result<(), AppError> {
    check(pool, presented, now_ms).await.map(|_| ())
}

/// Prüft und verbraucht den Token. Danach ist er ungültig.
pub async fn consume(pool: &SqlitePool, presented: &str, now_ms: i64) -> Result<(), AppError> {
    let stored = check(pool, presented, now_ms).await?;
    // Nur löschen, wenn noch derselbe Token gilt: Von zwei gleichzeitigen Anfragen gewinnt eine.
    if !store::settings::delete_if(pool, KEY, &stored).await? {
        return Err(rejected());
    }
    Ok(())
}

fn rejected() -> AppError {
    AppError::UnauthorizedWith(
        "Einrichtungslink ungültig oder abgelaufen – Server neu starten für einen neuen Link"
            .into(),
    )
}

async fn check(pool: &SqlitePool, presented: &str, now_ms: i64) -> Result<Stored, AppError> {
    let stored: Stored = store::settings::get(pool, KEY)
        .await?
        .ok_or_else(rejected)?;
    if stored.expires_at_ms <= now_ms || stored.hash != token::hash(presented.trim()) {
        return Err(rejected());
    }
    Ok(stored)
}

#[cfg(all(test, not(feature = "dev")))]
mod tests {
    use super::*;
    use crate::contract::OidcSettingsInput;

    fn token_of(link: &str) -> String {
        link.split_once("/einrichtung#")
            .expect("Link auf /einrichtung#<token>")
            .1
            .to_string()
    }

    #[tokio::test]
    async fn token_is_single_use_and_expires() {
        let pool = crate::db::test_pool().await;
        let settings = SettingsHandle::load(pool.clone()).await.unwrap();
        let boot = BootConfig::from_lookup(|_| None).unwrap();
        let link = ensure_token(&pool, &settings, &boot, 0)
            .await
            .unwrap()
            .unwrap();
        assert!(
            link.starts_with("http://127.0.0.1:8080/einrichtung#"),
            "{link}"
        );
        let tok = token_of(&link);
        assert!(matches!(
            consume(&pool, "falsch", 1).await,
            Err(AppError::UnauthorizedWith(_))
        ));
        assert!(
            matches!(
                consume(&pool, &tok, TTL_MS + 1).await,
                Err(AppError::UnauthorizedWith(_))
            ),
            "abgelaufen"
        );
        let link = ensure_token(&pool, &settings, &boot, TTL_MS + 2)
            .await
            .unwrap()
            .unwrap();
        let tok = token_of(&link);
        consume(&pool, &tok, TTL_MS + 3).await.unwrap();
        assert!(
            consume(&pool, &tok, TTL_MS + 4).await.is_err(),
            "nur einmal"
        );
    }

    #[tokio::test]
    async fn wrong_token_does_not_burn_the_link() {
        let pool = crate::db::test_pool().await;
        let settings = SettingsHandle::load(pool.clone()).await.unwrap();
        let boot = BootConfig::from_lookup(|_| None).unwrap();
        let link = ensure_token(&pool, &settings, &boot, 0)
            .await
            .unwrap()
            .unwrap();
        assert!(consume(&pool, "falsch", 1).await.is_err());
        consume(&pool, &token_of(&link), 2).await.unwrap();
    }

    #[tokio::test]
    async fn no_link_once_oidc_or_an_account_exists() {
        let pool = crate::db::test_pool().await;
        let settings = SettingsHandle::load(pool.clone()).await.unwrap();
        let boot = BootConfig::from_lookup(|_| None).unwrap();
        settings
            .update_oidc(OidcSettingsInput {
                issuer: "https://id.example.org".into(),
                client_id: "suchlage".into(),
                client_secret: Some("s".into()),
                admin_group: "suchlage-admin".into(),
                user_group: "suchlage".into(),
            })
            .await
            .unwrap();
        assert_eq!(
            ensure_token(&pool, &settings, &boot, 0).await.unwrap(),
            None
        );

        let pool = crate::db::test_pool().await;
        let settings = SettingsHandle::load(pool.clone()).await.unwrap();
        store::users::upsert_oidc(
            &pool,
            "iss",
            "sub",
            "Admin",
            crate::contract::Role::Admin,
            0,
        )
        .await
        .unwrap();
        assert_eq!(
            ensure_token(&pool, &settings, &boot, 0).await.unwrap(),
            None
        );
    }
}

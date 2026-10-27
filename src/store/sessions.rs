use sqlx::{Row, SqlitePool};

use super::users::{self, User};
use crate::token;

pub const USER_TTL_MS: i64 = 7 * 24 * 3_600_000;
/// Großbild läuft dauerhaft; Schluss macht der Widerruf des Tokens.
pub const KIOSK_TTL_MS: i64 = 365 * 24 * 3_600_000;

#[derive(Debug, Clone, PartialEq)]
pub enum Principal {
    User(User),
    Kiosk { token_id: i64, label: String },
}

async fn create(
    pool: &SqlitePool,
    user_id: Option<i64>,
    kiosk_id: Option<i64>,
    ttl: i64,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    let secret = token::generate();
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, kiosk_token_id, expires_at_ms, created_at_ms) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(token::hash(&secret))
    .bind(user_id)
    .bind(kiosk_id)
    .bind(now_ms + ttl)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(secret)
}

pub async fn create_for_user(
    pool: &SqlitePool,
    user_id: i64,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    create(pool, Some(user_id), None, USER_TTL_MS, now_ms).await
}

pub async fn create_for_kiosk(
    pool: &SqlitePool,
    token_id: i64,
    now_ms: i64,
) -> Result<String, sqlx::Error> {
    create(pool, None, Some(token_id), KIOSK_TTL_MS, now_ms).await
}

pub async fn lookup(
    pool: &SqlitePool,
    secret: &str,
    now_ms: i64,
) -> Result<Option<Principal>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT s.user_id, s.kiosk_token_id, k.label FROM sessions s LEFT JOIN kiosk_tokens k ON k.id = s.kiosk_token_id
         WHERE s.token_hash = ? AND s.expires_at_ms > ?",
    )
    .bind(token::hash(secret))
    .bind(now_ms)
    .fetch_optional(pool)
    .await?;
    let Some(row) = row else { return Ok(None) };
    if let Some(user_id) = row.get::<Option<i64>, _>("user_id") {
        return Ok(users::get(pool, user_id).await?.map(Principal::User));
    }
    Ok(Some(Principal::Kiosk {
        token_id: row.get("kiosk_token_id"),
        label: row.get("label"),
    }))
}

pub async fn delete(pool: &SqlitePool, secret: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
        .bind(token::hash(secret))
        .execute(pool)
        .await?;
    Ok(())
}

/// Beendet alle Sitzungen eines Kontos (z. B. aus allen Suchlage-Gruppen entfernt).
pub async fn delete_for_user(pool: &SqlitePool, user_id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::Role;
    use crate::db::test_pool;
    use crate::store::{kiosk, users};

    #[tokio::test]
    async fn sessions_expire_and_die_with_their_kiosk_token() {
        let pool = test_pool().await;
        let u = users::upsert_oidc(&pool, "i", "s", "Erika Muster", Role::Operator, 0)
            .await
            .unwrap();
        let s = create_for_user(&pool, u.id, 0).await.unwrap();
        assert!(matches!(
            lookup(&pool, &s, 1).await.unwrap(),
            Some(Principal::User(_))
        ));
        assert_eq!(
            lookup(&pool, &s, USER_TTL_MS).await.unwrap(),
            None,
            "abgelaufen"
        );
        assert_eq!(lookup(&pool, "falsch", 1).await.unwrap(), None);

        let (view, secret) = kiosk::create(&pool, "FüKw", 0).await.unwrap();
        let tid = kiosk::find_by_token(&pool, &secret).await.unwrap().unwrap();
        let ks = create_for_kiosk(&pool, tid, 0).await.unwrap();
        assert_eq!(
            lookup(&pool, &ks, 1).await.unwrap(),
            Some(Principal::Kiosk {
                token_id: view.id,
                label: "FüKw".into()
            })
        );
        assert!(kiosk::delete(&pool, view.id).await.unwrap());
        assert_eq!(
            lookup(&pool, &ks, 1).await.unwrap(),
            None,
            "Widerruf beendet die Sitzung"
        );
    }

    #[tokio::test]
    async fn logout_deletes_only_its_own_session() {
        let pool = test_pool().await;
        let u = users::upsert_oidc(&pool, "i", "s", "Erika Muster", Role::Operator, 0)
            .await
            .unwrap();
        let (a, b) = (
            create_for_user(&pool, u.id, 0).await.unwrap(),
            create_for_user(&pool, u.id, 0).await.unwrap(),
        );
        delete(&pool, &a).await.unwrap();
        assert_eq!(lookup(&pool, &a, 1).await.unwrap(), None);
        assert_eq!(
            lookup(&pool, &b, 1).await.unwrap(),
            Some(Principal::User(u))
        );
    }
}

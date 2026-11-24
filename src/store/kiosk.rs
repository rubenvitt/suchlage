use sqlx::{Row, SqlitePool};

use crate::contract::KioskTokenView;
use crate::token;

/// Legt einen Großbild-Token an und gibt das Geheimnis genau einmal zurück.
pub async fn create(
    pool: &SqlitePool,
    label: &str,
    now_ms: i64,
) -> Result<(KioskTokenView, String), sqlx::Error> {
    create_with(pool, label, false, false, now_ms).await
}

/// Wie [`create`]; `einsatztagebuch` macht ihn zum Zugang der IUK-Suite (IDA-359), `photos` gibt
/// ihm die Vorschaubilder des laufenden Einsatzes für Lifeline Hub (SUL-49).
pub async fn create_with(
    pool: &SqlitePool,
    label: &str,
    einsatztagebuch: bool,
    photos: bool,
    now_ms: i64,
) -> Result<(KioskTokenView, String), sqlx::Error> {
    let secret = token::generate();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO kiosk_tokens (label, token_hash, created_at_ms, einsatztagebuch, photos) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(label)
    .bind(token::hash(&secret))
    .bind(now_ms)
    .bind(einsatztagebuch)
    .bind(photos)
    .fetch_one(pool)
    .await?;
    let view = KioskTokenView {
        id,
        label: label.into(),
        created_at_ms: now_ms,
        einsatztagebuch,
        photos,
    };
    Ok((view, secret))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<KioskTokenView>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, label, created_at_ms, einsatztagebuch, photos FROM kiosk_tokens ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| KioskTokenView {
            id: r.get("id"),
            label: r.get("label"),
            created_at_ms: r.get("created_at_ms"),
            einsatztagebuch: r.get("einsatztagebuch"),
            photos: r.get("photos"),
        })
        .collect())
}

/// Widerruf: löscht den Token; Sitzungen fallen per ON DELETE CASCADE mit weg.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("DELETE FROM kiosk_tokens WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

/// Gibt es den Token, ob er ein Zugang der IUK-Suite ist.
pub async fn einsatztagebuch_by_token(
    pool: &SqlitePool,
    secret: &str,
) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar("SELECT einsatztagebuch FROM kiosk_tokens WHERE token_hash = ?")
        .bind(token::hash(secret))
        .fetch_optional(pool)
        .await
}

/// Gibt es den Token, ob er die Fotos des laufenden Einsatzes abrufen darf (Lifeline Hub).
pub async fn photos_by_token(pool: &SqlitePool, secret: &str) -> Result<Option<bool>, sqlx::Error> {
    sqlx::query_scalar("SELECT photos FROM kiosk_tokens WHERE token_hash = ?")
        .bind(token::hash(secret))
        .fetch_optional(pool)
        .await
}

pub async fn find_by_token(pool: &SqlitePool, secret: &str) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM kiosk_tokens WHERE token_hash = ?")
        .bind(token::hash(secret))
        .fetch_optional(pool)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn only_the_hash_is_stored_and_revocation_is_reported_once() {
        let pool = test_pool().await;
        let (view, secret) = create(&pool, "Großbild ELW", 7).await.unwrap();
        assert_eq!(list(&pool).await.unwrap(), vec![view.clone()]);
        let stored: String = sqlx::query_scalar("SELECT token_hash FROM kiosk_tokens")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(stored, token::hash(&secret));
        assert_eq!(
            find_by_token(&pool, &stored).await.unwrap(),
            None,
            "Hash ist kein Token"
        );
        assert_eq!(find_by_token(&pool, &secret).await.unwrap(), Some(view.id));

        assert!(delete(&pool, view.id).await.unwrap());
        assert!(!delete(&pool, view.id).await.unwrap());
        assert!(list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_photos_flag_is_off_unless_asked_for() {
        let pool = test_pool().await;
        let (plain, plain_secret) = create(&pool, "FüKw", 1).await.unwrap();
        let (hub, hub_secret) = create_with(&pool, "Lifeline Hub", false, true, 2)
            .await
            .unwrap();
        assert!(!plain.photos);
        assert!(hub.photos);
        assert_eq!(list(&pool).await.unwrap(), vec![plain, hub]);
        assert_eq!(
            photos_by_token(&pool, &plain_secret).await.unwrap(),
            Some(false)
        );
        assert_eq!(
            photos_by_token(&pool, &hub_secret).await.unwrap(),
            Some(true)
        );
        assert_eq!(photos_by_token(&pool, "falsch").await.unwrap(), None);
    }
}

use sqlx::{Row, SqlitePool};

use crate::contract::KioskTokenView;
use crate::token;

/// Legt einen Großbild-Token an und gibt das Geheimnis genau einmal zurück.
pub async fn create(
    pool: &SqlitePool,
    label: &str,
    now_ms: i64,
) -> Result<(KioskTokenView, String), sqlx::Error> {
    let secret = token::generate();
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO kiosk_tokens (label, token_hash, created_at_ms) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(label)
    .bind(token::hash(&secret))
    .bind(now_ms)
    .fetch_one(pool)
    .await?;
    let view = KioskTokenView {
        id,
        label: label.into(),
        created_at_ms: now_ms,
    };
    Ok((view, secret))
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<KioskTokenView>, sqlx::Error> {
    let rows = sqlx::query("SELECT id, label, created_at_ms FROM kiosk_tokens ORDER BY id")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .iter()
        .map(|r| KioskTokenView {
            id: r.get("id"),
            label: r.get("label"),
            created_at_ms: r.get("created_at_ms"),
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
}

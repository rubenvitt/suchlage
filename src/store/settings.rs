//! Einstellungen als JSON je Schlüssel. Ist ein gespeicherter Wert nicht mehr lesbar (Format
//! geändert), gilt die Vorgabe des Aufrufers, statt den Start zu verhindern.

use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::SqlitePool;

pub async fn get<T: DeserializeOwned>(
    pool: &SqlitePool,
    key: &str,
) -> Result<Option<T>, sqlx::Error> {
    let raw: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(raw.and_then(|r| match serde_json::from_str(&r) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!(key, error = %e, "gespeicherte Einstellung nicht lesbar – Vorgabe gilt");
            None
        }
    }))
}

pub async fn put<T: Serialize>(
    pool: &SqlitePool,
    key: &str,
    value: &T,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    let json = serde_json::to_string(value).expect("Einstellungen sind serialisierbar");
    sqlx::query(
        "INSERT INTO settings (key, value, updated_at_ms) VALUES (?, ?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(key)
    .bind(json)
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn put_overwrites_and_get_reads_back() {
        let pool = crate::db::test_pool().await;
        assert_eq!(super::get::<u32>(&pool, "x").await.unwrap(), None);
        super::put(&pool, "x", &1u32, 0).await.unwrap();
        super::put(&pool, "x", &2u32, 1).await.unwrap();
        assert_eq!(super::get::<u32>(&pool, "x").await.unwrap(), Some(2));
    }

    #[tokio::test]
    async fn unreadable_value_falls_back_to_none() {
        let pool = crate::db::test_pool().await;
        super::put(&pool, "x", &"kein u32", 0).await.unwrap();
        assert_eq!(super::get::<u32>(&pool, "x").await.unwrap(), None);
    }
}

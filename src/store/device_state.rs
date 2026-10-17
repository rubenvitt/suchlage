//! Letzter Gerätestand als JSON (Spec §5, Neustart). Der Speicher kennt das Format nicht; das
//! liest und schreibt nur der DJI-Adapter (B12).

use serde_json::Value;
use sqlx::{Row, SqlitePool};

#[derive(Debug, Clone, PartialEq)]
pub struct StoredDevice {
    pub sn: String,
    pub gateway_sn: String,
    pub data: Value,
    pub updated_at_ms: i64,
}

pub async fn put(
    pool: &SqlitePool,
    sn: &str,
    gateway_sn: &str,
    data: &Value,
    now_ms: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO device_state (sn, gateway_sn, data, updated_at_ms) VALUES (?, ?, ?, ?) ON CONFLICT(sn) DO UPDATE SET gateway_sn = excluded.gateway_sn, data = excluded.data, updated_at_ms = excluded.updated_at_ms",
    )
    .bind(sn)
    .bind(gateway_sn)
    .bind(data.to_string())
    .bind(now_ms)
    .execute(pool)
    .await?;
    Ok(())
}

/// Alle Geräte, ältester Stand zuerst. Ein unlesbarer Eintrag fällt mit Warnung weg; der nächste
/// Teil-Update des Geräts schreibt ihn neu.
pub async fn all(pool: &SqlitePool) -> Result<Vec<StoredDevice>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT sn, gateway_sn, data, updated_at_ms FROM device_state ORDER BY updated_at_ms",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .filter_map(|r| {
            let sn: String = r.get("sn");
            let data = serde_json::from_str(r.get::<&str, _>("data"))
                .inspect_err(
                    |e| tracing::warn!(sn, error = %e, "gespeicherter Gerätestand nicht lesbar"),
                )
                .ok()?;
            Some(StoredDevice {
                sn,
                gateway_sn: r.get("gateway_sn"),
                data,
                updated_at_ms: r.get("updated_at_ms"),
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn last_state_per_device_wins() {
        let pool = crate::db::test_pool().await;
        super::put(&pool, "RC1", "RC1", &serde_json::json!({"a": 1}), 1)
            .await
            .unwrap();
        super::put(&pool, "RC1", "RC1", &serde_json::json!({"a": 2}), 2)
            .await
            .unwrap();
        let all = super::all(&pool).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(
            (all[0].data["a"].as_i64(), all[0].updated_at_ms),
            (Some(2), 2)
        );
    }

    #[tokio::test]
    async fn unreadable_entry_is_skipped_others_survive() {
        let pool = crate::db::test_pool().await;
        super::put(&pool, "AC1", "RC1", &serde_json::json!({"a": 1}), 1)
            .await
            .unwrap();
        sqlx::query("INSERT INTO device_state (sn, gateway_sn, data, updated_at_ms) VALUES ('RC1', 'RC1', '{kaputt', 2)")
            .execute(&pool)
            .await
            .unwrap();
        let sns: Vec<String> = super::all(&pool)
            .await
            .unwrap()
            .into_iter()
            .map(|d| d.sn)
            .collect();
        assert_eq!(sns, vec!["AC1".to_string()]);
    }
}

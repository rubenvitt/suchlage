use sqlx::{Row, SqlitePool};

use crate::contract::RemoteControllerView;
use crate::token;

#[derive(Debug, Clone, PartialEq)]
pub struct RemoteController {
    pub id: i64,
    pub label: String,
    pub sn: Option<String>,
    pub mqtt_password: Option<String>,
    pub stream_key: Option<String>,
    pub paired_at_ms: Option<i64>,
}

/// Spaltenliste als Literal, damit jede Abfrage ein geprüftes `&'static str` bleibt (sqlx 0.9).
macro_rules! columns {
    () => {
        "id, label, sn, mqtt_password, stream_key, paired_at_ms"
    };
}

fn row(r: &sqlx::sqlite::SqliteRow) -> RemoteController {
    RemoteController {
        id: r.get("id"),
        label: r.get("label"),
        sn: r.get("sn"),
        mqtt_password: r.get("mqtt_password"),
        stream_key: r.get("stream_key"),
        paired_at_ms: r.get("paired_at_ms"),
    }
}

/// Sicht für die Oberfläche: ohne MQTT-Passwort und Stream-Schlüssel.
pub fn view(rc: &RemoteController) -> RemoteControllerView {
    RemoteControllerView {
        id: rc.id,
        label: rc.label.clone(),
        sn: rc.sn.clone(),
        paired_at_ms: rc.paired_at_ms,
    }
}

/// Legt eine Fernsteuerung an und gibt den Kopplungscode genau einmal zurück.
pub async fn create(
    pool: &SqlitePool,
    label: &str,
    now_ms: i64,
) -> Result<(RemoteController, String), sqlx::Error> {
    let code = token::generate();
    Ok((create_with_code(pool, label, &code, now_ms).await?, code))
}

/// Mit vorgegebenem Code. Außerhalb der Tests nur für die Simulator-Fernsteuerung im Feature `dev` (B14).
pub async fn create_with_code(
    pool: &SqlitePool,
    label: &str,
    code: &str,
    now_ms: i64,
) -> Result<RemoteController, sqlx::Error> {
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO remote_controllers (label, pairing_code_hash, created_at_ms) VALUES (?, ?, ?) RETURNING id",
    )
    .bind(label)
    .bind(token::hash(code))
    .bind(now_ms)
    .fetch_one(pool)
    .await?;
    Ok(RemoteController {
        id,
        label: label.into(),
        sn: None,
        mqtt_password: None,
        stream_key: None,
        paired_at_ms: None,
    })
}

pub async fn list(pool: &SqlitePool) -> Result<Vec<RemoteController>, sqlx::Error> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM remote_controllers ORDER BY id"
    ))
    .fetch_all(pool)
    .await?;
    Ok(rows.iter().map(row).collect())
}

/// Entfernt die Fernsteuerung und liefert ihren letzten Stand, damit der Aufrufer ihren
/// Broker-Zugang mit abbauen kann.
pub async fn delete(pool: &SqlitePool, id: i64) -> Result<Option<RemoteController>, sqlx::Error> {
    let r = sqlx::query(concat!(
        "DELETE FROM remote_controllers WHERE id = ? RETURNING ",
        columns!()
    ))
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(r.as_ref().map(row))
}

pub async fn find_by_code(
    pool: &SqlitePool,
    code: &str,
) -> Result<Option<RemoteController>, sqlx::Error> {
    let r = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM remote_controllers WHERE pairing_code_hash = ?"
    ))
    .bind(token::hash(code))
    .fetch_optional(pool)
    .await?;
    Ok(r.as_ref().map(row))
}

pub async fn find_by_sn(
    pool: &SqlitePool,
    sn: &str,
) -> Result<Option<RemoteController>, sqlx::Error> {
    let r = sqlx::query(concat!(
        "SELECT ",
        columns!(),
        " FROM remote_controllers WHERE sn = ?"
    ))
    .bind(sn)
    .fetch_optional(pool)
    .await?;
    Ok(r.as_ref().map(row))
}

/// Bindet die SN, wenn die Fernsteuerung noch keine hat. `false`: Eine andere Kopplung war
/// schneller; der gespeicherte Zugang bleibt unverändert.
pub async fn bind(
    pool: &SqlitePool,
    id: i64,
    sn: &str,
    mqtt_password: &str,
    stream_key: &str,
    now_ms: i64,
) -> Result<bool, sqlx::Error> {
    let done = sqlx::query(
        "UPDATE remote_controllers SET sn = ?, mqtt_password = ?, stream_key = ?, paired_at_ms = ? WHERE id = ? AND sn IS NULL",
    )
    .bind(sn)
    .bind(mqtt_password)
    .bind(stream_key)
    .bind(now_ms)
    .bind(id)
    .execute(pool)
    .await?;
    Ok(done.rows_affected() == 1)
}

/// (SN, Bezeichnung) aller gekoppelten Fernsteuerungen.
pub async fn labels(pool: &SqlitePool) -> Result<Vec<(String, String)>, sqlx::Error> {
    let rows = sqlx::query("SELECT sn, label FROM remote_controllers WHERE sn IS NOT NULL")
        .fetch_all(pool)
        .await?;
    Ok(rows.iter().map(|r| (r.get("sn"), r.get("label"))).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::error::AppError;

    #[tokio::test]
    async fn pairing_binds_the_serial_number_once() {
        let pool = test_pool().await;
        let (rc, code) = create(&pool, "RC Plus 1", 0).await.unwrap();
        assert_eq!(find_by_code(&pool, &code).await.unwrap().unwrap().id, rc.id);
        assert_eq!(find_by_code(&pool, "falsch").await.unwrap(), None);
        assert!(bind(&pool, rc.id, "RC1", "pw", "key", 5).await.unwrap());
        assert!(
            !bind(&pool, rc.id, "RC9", "pw2", "key2", 6).await.unwrap(),
            "zweite Kopplung überschreibt nicht"
        );
        assert_eq!(find_by_sn(&pool, "RC9").await.unwrap(), None);
        assert_eq!(
            find_by_sn(&pool, "RC1")
                .await
                .unwrap()
                .unwrap()
                .paired_at_ms,
            Some(5)
        );
        assert_eq!(
            labels(&pool).await.unwrap(),
            vec![("RC1".to_string(), "RC Plus 1".to_string())]
        );

        let (other, _) = create(&pool, "RC Plus 2", 0).await.unwrap();
        let err: AppError = bind(&pool, other.id, "RC1", "pw", "key", 6)
            .await
            .unwrap_err()
            .into();
        assert!(
            matches!(err, AppError::Conflict(_)),
            "SN gehört schon RC Plus 1"
        );
    }

    #[tokio::test]
    async fn delete_returns_the_removed_controller_and_view_hides_secrets() {
        let pool = test_pool().await;
        let rc = create_with_code(&pool, "RC Plus 1", "sim-code", 0)
            .await
            .unwrap();
        bind(&pool, rc.id, "RC1", "pw", "key", 5).await.unwrap();
        let removed = delete(&pool, rc.id).await.unwrap().unwrap();
        assert_eq!(
            (
                removed.mqtt_password.as_deref(),
                removed.stream_key.as_deref()
            ),
            (Some("pw"), Some("key"))
        );
        assert_eq!(
            view(&removed),
            RemoteControllerView {
                id: rc.id,
                label: "RC Plus 1".into(),
                sn: Some("RC1".into()),
                paired_at_ms: Some(5)
            }
        );
        assert_eq!(delete(&pool, rc.id).await.unwrap(), None);
        assert!(list(&pool).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn empty_label_is_rejected_by_the_schema() {
        let pool = test_pool().await;
        let err: AppError = create(&pool, "", 0).await.unwrap_err().into();
        assert!(matches!(err, AppError::Unprocessable(_)));
    }
}

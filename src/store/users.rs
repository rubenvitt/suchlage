use sqlx::{Row, SqlitePool};

use crate::contract::{AccountView, Role};

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: i64,
    pub display_name: String,
    pub role: Role,
}

fn role(s: &str) -> Role {
    if s == "admin" {
        Role::Admin
    } else {
        Role::Operator
    }
}

/// Findet das Konto über (issuer, subject) oder legt es an. Die Rolle kommt bei jedem Login aus
/// den Pocket-ID-Gruppen und überschreibt die gespeicherte.
pub async fn upsert_oidc(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
    display_name: &str,
    role: Role,
    now_ms: i64,
) -> Result<User, sqlx::Error> {
    let r = if role == Role::Admin {
        "admin"
    } else {
        "operator"
    };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO users (oidc_issuer, oidc_subject, display_name, role, created_at_ms) VALUES (?, ?, ?, ?, ?) \
         ON CONFLICT (oidc_issuer, oidc_subject) DO UPDATE SET display_name = excluded.display_name, role = excluded.role \
         RETURNING id",
    )
    .bind(issuer)
    .bind(subject)
    .bind(display_name)
    .bind(r)
    .bind(now_ms)
    .fetch_one(pool)
    .await?;
    Ok(User {
        id,
        display_name: display_name.into(),
        role: self::role(r),
    })
}

/// Ein nicht gesperrtes Konto.
pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, display_name, role FROM users WHERE id = ? AND blocked_at_ms IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| User {
        id: r.get("id"),
        display_name: r.get("display_name"),
        role: role(r.get("role")),
    }))
}

/// Nicht gesperrtes Konto zu Aussteller und Subjekt, ohne es anzulegen. Der Aussteller zählt ohne
/// Schrägstrich am Ende (Anmeldung und Zugriffstoken schreiben ihn nicht zwingend gleich).
pub async fn find_by_oidc(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, display_name, role FROM users \
         WHERE rtrim(oidc_issuer, '/') = ? AND oidc_subject = ? AND blocked_at_ms IS NULL",
    )
    .bind(issuer.trim_end_matches('/'))
    .bind(subject)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| User {
        id: r.get("id"),
        display_name: r.get("display_name"),
        role: role(r.get("role")),
    }))
}

pub async fn count(pool: &SqlitePool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await
}

/// Alle Konten für die Verwaltung, nach Name. `own` markiert das Konto `own_id`.
pub async fn list(pool: &SqlitePool, own_id: i64) -> Result<Vec<AccountView>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, display_name, role, created_at_ms, blocked_at_ms FROM users \
         ORDER BY display_name COLLATE NOCASE, id",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .iter()
        .map(|r| AccountView {
            id: r.get("id"),
            display_name: r.get("display_name"),
            role: role(r.get("role")),
            created_at_ms: r.get("created_at_ms"),
            blocked_at_ms: r.get("blocked_at_ms"),
            own: r.get::<i64, _>("id") == own_id,
        })
        .collect())
}

/// Ist das Konto gesperrt? Ein unbekanntes Konto gilt als nicht gesperrt.
pub async fn is_blocked(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let blocked: Option<Option<i64>> =
        sqlx::query_scalar("SELECT blocked_at_ms FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await?;
    Ok(matches!(blocked, Some(Some(_))))
}

/// Sperrt das Konto und löscht in derselben Transaktion alle seine Sitzungen. Ein schon gesperrtes
/// Konto behält den ersten Zeitpunkt. `false`, wenn es das Konto nicht gibt.
pub async fn block(pool: &SqlitePool, id: i64, now_ms: i64) -> Result<bool, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let found =
        sqlx::query("UPDATE users SET blocked_at_ms = coalesce(blocked_at_ms, ?) WHERE id = ?")
            .bind(now_ms)
            .bind(id)
            .execute(&mut *tx)
            .await?
            .rows_affected()
            == 1;
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(found)
}

/// Hebt die Sperre auf. Eine Sitzung gibt es erst wieder nach der nächsten Anmeldung.
pub async fn unblock(pool: &SqlitePool, id: i64) -> Result<bool, sqlx::Error> {
    let done = sqlx::query("UPDATE users SET blocked_at_ms = NULL WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(done.rows_affected() == 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;

    #[tokio::test]
    async fn login_keeps_the_account_and_takes_the_role_it_is_given() {
        let pool = test_pool().await;
        let a = upsert_oidc(&pool, "https://id", "sub-a", "Erika Muster", Role::Admin, 1)
            .await
            .unwrap();
        let b = upsert_oidc(&pool, "https://id", "sub-b", "Gast", Role::Operator, 2)
            .await
            .unwrap();
        let a2 = upsert_oidc(
            &pool,
            "https://id",
            "sub-a",
            "Erika Musterfrau",
            Role::Operator,
            3,
        )
        .await
        .unwrap();
        assert_eq!((a.role, b.role), (Role::Admin, Role::Operator));
        assert_eq!(
            (a2.id, a2.role, a2.display_name.as_str()),
            (a.id, Role::Operator, "Erika Musterfrau"),
            "aus der Admin-Gruppe entfernt"
        );
        assert_eq!(get(&pool, a.id).await.unwrap(), Some(a2.clone()));
        assert_eq!(get(&pool, b.id).await.unwrap(), Some(b.clone()));
        assert_eq!(count(&pool).await.unwrap(), 2);
        assert_eq!(
            find_by_oidc(&pool, "https://id/", "sub-b").await.unwrap(),
            Some(b)
        );
        assert_eq!(
            find_by_oidc(&pool, "https://id", "sub-x").await.unwrap(),
            None
        );
        assert_eq!(
            find_by_oidc(&pool, "https://other", "sub-a").await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn blocked_account_is_invisible_to_sessions_and_tokens_until_unblocked() {
        let pool = test_pool().await;
        let a = upsert_oidc(&pool, "https://id", "sub-a", "Erika Muster", Role::Admin, 1)
            .await
            .unwrap();
        let b = upsert_oidc(&pool, "https://id", "sub-b", "Gast", Role::Operator, 2)
            .await
            .unwrap();
        assert!(block(&pool, b.id, 10).await.unwrap());
        assert!(
            block(&pool, b.id, 20).await.unwrap(),
            "zweites Sperren schadet nicht"
        );
        assert!(!block(&pool, 999, 10).await.unwrap(), "unbekanntes Konto");
        assert!(is_blocked(&pool, b.id).await.unwrap());
        assert!(!is_blocked(&pool, a.id).await.unwrap());
        assert_eq!(get(&pool, b.id).await.unwrap(), None);
        assert_eq!(
            find_by_oidc(&pool, "https://id", "sub-b").await.unwrap(),
            None
        );
        let accounts = list(&pool, a.id).await.unwrap();
        assert_eq!(
            accounts
                .iter()
                .map(|v| (v.display_name.as_str(), v.blocked_at_ms, v.own))
                .collect::<Vec<_>>(),
            vec![("Erika Muster", None, true), ("Gast", Some(10), false)],
            "nach Name, erster Sperrzeitpunkt bleibt"
        );

        // Eine Anmeldung aktualisiert das Konto, hebt die Sperre aber nicht auf.
        upsert_oidc(&pool, "https://id", "sub-b", "Gast", Role::Operator, 30)
            .await
            .unwrap();
        assert!(is_blocked(&pool, b.id).await.unwrap());

        assert!(unblock(&pool, b.id).await.unwrap());
        assert!(!unblock(&pool, 999).await.unwrap());
        assert_eq!(get(&pool, b.id).await.unwrap(), Some(b));
    }
}

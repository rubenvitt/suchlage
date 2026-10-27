use sqlx::{Row, SqlitePool};

use crate::contract::Role;

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

pub async fn get(pool: &SqlitePool, id: i64) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query("SELECT id, display_name, role FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|r| User {
        id: r.get("id"),
        display_name: r.get("display_name"),
        role: role(r.get("role")),
    }))
}

/// Konto zu Aussteller und Subjekt, ohne es anzulegen. Der Aussteller zählt ohne Schrägstrich am
/// Ende (Anmeldung und Zugriffstoken schreiben ihn nicht zwingend gleich).
pub async fn find_by_oidc(
    pool: &SqlitePool,
    issuer: &str,
    subject: &str,
) -> Result<Option<User>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT id, display_name, role FROM users WHERE rtrim(oidc_issuer, '/') = ? AND oidc_subject = ?",
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
}

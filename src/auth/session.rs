//! Sitzungscookie und Rollen-Extractors. Rollen: admin ⊃ operator ⊃ viewer; Großbild ist nur viewer.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use sqlx::SqlitePool;
use std::time::Duration;

use crate::config::BootConfig;
use crate::contract::Role;
use crate::error::AppError;
use crate::store::sessions::{self, Principal};
use crate::store::users::User;
use crate::time::now_ms;

pub const COOKIE: &str = "suchlage_sid";

pub fn secure_cookies(boot: &BootConfig) -> bool {
    boot.public_url.scheme() == "https"
}

pub fn session_cookie(token: String, max_age_ms: i64, secure: bool) -> Cookie<'static> {
    Cookie::build((COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(secure)
        .max_age(time::Duration::milliseconds(max_age_ms))
        .build()
}

/// Wie oft ein offener Strom (SSE) seine Sitzung neu prüft.
pub const STREAM_CHECK: Duration = Duration::from_secs(5);

/// Geheimnis der Sitzung aus dem Cookie, z. B. für `ended`.
pub fn token(jar: &CookieJar) -> Option<String> {
    jar.get(COOKIE).map(|c| c.value().to_string())
}

/// Endet, sobald die Sitzung hinter `token` nicht mehr gilt oder `keep` sie nicht mehr zulässt:
/// abgemeldet, abgelaufen, Konto gesperrt, Großbild-Token widerrufen, Admin herabgestuft. Damit
/// enden auch offene Ströme, spätestens nach `every`. Ein Lesefehler der Datenbank beendet nichts.
pub async fn ended(
    pool: SqlitePool,
    token: String,
    every: Duration,
    keep: impl Fn(&Principal) -> bool,
) {
    let mut tick = tokio::time::interval(every);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    // Der erste Tick kommt sofort; gerade eben hat der Extractor die Sitzung geprüft.
    tick.tick().await;
    loop {
        tick.tick().await;
        match sessions::lookup(&pool, &token, now_ms()).await {
            Ok(Some(p)) if keep(&p) => {}
            Ok(_) => return,
            Err(e) => tracing::warn!(error = ?e, "Sitzung des Stroms nicht prüfbar"),
        }
    }
}

pub fn is_admin(p: &Principal) -> bool {
    matches!(p, Principal::User(u) if u.role == Role::Admin)
}

// Generisch über `FromRef`: So kennt `auth` den `AppState` nicht, und `app` ↔ `auth` bildet keinen Zyklus.
async fn principal<S>(parts: &mut Parts, state: &S) -> Result<Principal, AppError>
where
    SqlitePool: FromRef<S>,
{
    let jar = CookieJar::from_headers(&parts.headers);
    let token = jar.get(COOKIE).ok_or(AppError::Unauthorized)?;
    sessions::lookup(&SqlitePool::from_ref(state), token.value(), now_ms())
        .await?
        .ok_or(AppError::Unauthorized)
}

pub struct Viewer(pub Principal);
pub struct Operator(pub User);
pub struct Admin(pub User);

impl<S: Send + Sync> FromRequestParts<S> for Viewer
where
    SqlitePool: FromRef<S>,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        Ok(Viewer(principal(parts, state).await?))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Operator
where
    SqlitePool: FromRef<S>,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        match principal(parts, state).await? {
            Principal::User(u) => Ok(Operator(u)),
            Principal::Kiosk { .. } => Err(AppError::Forbidden),
        }
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Admin
where
    SqlitePool: FromRef<S>,
{
    type Rejection = AppError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, AppError> {
        match principal(parts, state).await? {
            Principal::User(u) if u.role == Role::Admin => Ok(Admin(u)),
            _ => Err(AppError::Forbidden),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::test_pool;
    use crate::store::users;

    const EVERY: Duration = Duration::from_millis(20);

    async fn user_session(pool: &SqlitePool, role: Role) -> (i64, String) {
        let u = users::upsert_oidc(pool, "i", &format!("{role:?}"), "Erika", role, 0)
            .await
            .unwrap();
        (
            u.id,
            sessions::create_for_user(pool, u.id, now_ms())
                .await
                .unwrap(),
        )
    }

    /// `true`, wenn `ended` innerhalb einer Sekunde zurückkehrt.
    async fn ends(fut: impl Future<Output = ()>) -> bool {
        tokio::time::timeout(Duration::from_secs(1), fut)
            .await
            .is_ok()
    }

    #[tokio::test]
    async fn stream_ends_when_the_account_is_blocked() {
        let pool = test_pool().await;
        let (id, token) = user_session(&pool, Role::Operator).await;
        // Als eigene Aufgabe: Die Test-DB hat nur eine Verbindung, ein halb gepolltes `ended`
        // hielte sie fest.
        let watch = tokio::spawn(ended(pool.clone(), token.clone(), EVERY, |_| true));
        tokio::time::sleep(EVERY * 5).await;
        assert!(!watch.is_finished(), "gültige Sitzung läuft weiter");
        users::block(&pool, id, 1).await.unwrap();
        assert!(ends(async { watch.await.unwrap() }).await);
    }

    #[tokio::test]
    async fn stream_ends_on_logout_and_when_keep_refuses() {
        let pool = test_pool().await;
        let (_, token) = user_session(&pool, Role::Operator).await;
        let watch = ended(pool.clone(), token.clone(), EVERY, |_| true);
        sessions::delete(&pool, &token).await.unwrap();
        assert!(ends(watch).await, "abgemeldet");

        let (_, op) = user_session(&pool, Role::Operator).await;
        assert!(
            ends(ended(pool.clone(), op, EVERY, is_admin)).await,
            "kein Admin"
        );
        assert!(ends(ended(pool, "falsch".into(), EVERY, |_| true)).await);
    }
}

//! Sitzungscookie und Rollen-Extractors. Rollen: admin ⊃ operator ⊃ viewer; Großbild ist nur viewer.

use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use sqlx::SqlitePool;

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

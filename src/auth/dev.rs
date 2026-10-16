//! Nur mit Feature `dev`: Anmeldung ohne OIDC als „Entwicklung“ (Admin, weil erstes Konto).

use axum::extract::State;
use axum::http::StatusCode;
use axum_extra::extract::CookieJar;

use crate::app::AppState;
use crate::auth::session::{secure_cookies, session_cookie};
use crate::error::AppError;
use crate::store::{sessions, users};
use crate::time::now_ms;

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), AppError> {
    let now = now_ms();
    let user = users::upsert_oidc(&state.pool, "dev", "dev", "Entwicklung", now).await?;
    let token = sessions::create_for_user(&state.pool, user.id, now).await?;
    let cookie = session_cookie(token, sessions::USER_TTL_MS, secure_cookies(&state.boot));
    Ok((jar.add(cookie), StatusCode::NO_CONTENT))
}

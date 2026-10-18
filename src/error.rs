//! Ein Fehlertyp für alle Handler. Antwort immer `{"error": "<deutscher Satz>"}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Nicht angemeldet")]
    Unauthorized,
    /// 401 mit eigener Meldung (z. B. ungültiger Zugangslink).
    #[error("{0}")]
    UnauthorizedWith(String),
    #[error("Keine Berechtigung für diese Aktion")]
    Forbidden,
    #[error("Nicht gefunden")]
    NotFound,
    /// Ein einzelnes Feld ist ungültig (400).
    #[error("{0}")]
    Validation(String),
    /// Zusammenhang oder Zustandsübergang verbietet die Anfrage (422).
    #[error("{0}")]
    Unprocessable(String),
    /// Konflikt mit dem Lebenszyklus (409).
    #[error("{0}")]
    Conflict(String),
    /// Body über der Grenze der Route (413).
    #[error("{0}")]
    TooLarge(String),
    /// Abhängiger Dienst (Broker, MediaMTX, IdP) nicht erreichbar oder nicht eingerichtet (503).
    #[error("{0}")]
    Unavailable(String),
    /// Details nur ins Log.
    #[error("Interner Serverfehler")]
    Internal(String),
}

impl AppError {
    pub fn status(&self) -> StatusCode {
        match self {
            AppError::Unauthorized | AppError::UnauthorizedWith(_) => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Conflict(_) => StatusCode::CONFLICT,
            AppError::TooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
            AppError::Unavailable(_) => StatusCode::SERVICE_UNAVAILABLE,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        if let AppError::Internal(detail) = &self {
            tracing::error!(%detail, "interner Fehler");
        }
        (
            self.status(),
            Json(serde_json::json!({ "error": self.to_string() })),
        )
            .into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db) = &e {
            match db.kind() {
                sqlx::error::ErrorKind::UniqueViolation
                | sqlx::error::ErrorKind::ForeignKeyViolation => {
                    return AppError::Conflict(
                        "Eintrag steht im Konflikt mit dem bestehenden Stand".into(),
                    );
                }
                sqlx::error::ErrorKind::CheckViolation => {
                    return AppError::Unprocessable(
                        "Eingabe verletzt eine Regel des Datenmodells".into(),
                    );
                }
                _ => {}
            }
            if is_busy_or_locked(db.code().as_deref()) {
                return AppError::Unavailable("Datenbank ist gerade beschäftigt".into());
            }
        }
        if matches!(e, sqlx::Error::PoolTimedOut) {
            return AppError::Unavailable("Datenbank ist gerade beschäftigt".into());
        }
        AppError::Internal(e.to_string())
    }
}

/// SQLITE_BUSY (5) und SQLITE_LOCKED (6): später erneut versuchen. sqlx meldet den erweiterten
/// Code (z. B. 517 BUSY_SNAPSHOT im WAL-Betrieb); der primäre steht im unteren Byte.
fn is_busy_or_locked(code: Option<&str>) -> bool {
    code.and_then(|c| c.parse::<i32>().ok())
        .is_some_and(|c| matches!(c & 0xff, 5 | 6))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_detail_never_reaches_the_client() {
        let err = AppError::Internal("geheimes Detail".into());
        assert_eq!(err.to_string(), "Interner Serverfehler");
        assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    async fn db_error(sql: &'static str) -> AppError {
        let pool = crate::db::test_pool().await;
        sqlx::query("INSERT INTO settings (key, value, updated_at_ms) VALUES ('k', 'v', 0)")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query(sql).execute(&pool).await.unwrap_err().into()
    }

    #[tokio::test]
    async fn database_errors_map_to_the_status_convention() {
        let duplicate =
            db_error("INSERT INTO settings (key, value, updated_at_ms) VALUES ('k', 'w', 1)").await;
        assert_eq!(duplicate.status(), StatusCode::CONFLICT, "{duplicate:?}");
        let dangling = db_error(
            "INSERT INTO sessions (token_hash, user_id, expires_at_ms, created_at_ms) VALUES ('t', 99, 0, 0)",
        )
        .await;
        assert_eq!(dangling.status(), StatusCode::CONFLICT, "{dangling:?}");
        let empty_label = db_error(
            "INSERT INTO kiosk_tokens (label, token_hash, created_at_ms) VALUES ('', 'h', 0)",
        )
        .await;
        assert_eq!(
            empty_label.status(),
            StatusCode::UNPROCESSABLE_ENTITY,
            "{empty_label:?}"
        );
    }

    #[test]
    fn exhausted_pool_and_busy_database_are_unavailable_everything_else_internal() {
        let timeout: AppError = sqlx::Error::PoolTimedOut.into();
        assert_eq!(timeout.status(), StatusCode::SERVICE_UNAVAILABLE);
        let missing: AppError = sqlx::Error::RowNotFound.into();
        assert_eq!(missing.status(), StatusCode::INTERNAL_SERVER_ERROR);

        for busy in ["5", "6", "261", "517", "773", "262"] {
            assert!(is_busy_or_locked(Some(busy)), "{busy}");
        }
        for other in ["19", "2067", "0", "x"] {
            assert!(!is_busy_or_locked(Some(other)), "{other}");
        }
        assert!(!is_busy_or_locked(None));
    }
}

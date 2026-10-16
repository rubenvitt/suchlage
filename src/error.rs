//! Ein Fehlertyp für alle Handler. Antwort immer `{"error": "<deutscher Satz>"}`.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Nicht angemeldet")]
    Unauthorized,
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
            AppError::Unauthorized => StatusCode::UNAUTHORIZED,
            AppError::Forbidden => StatusCode::FORBIDDEN,
            AppError::NotFound => StatusCode::NOT_FOUND,
            AppError::Validation(_) => StatusCode::BAD_REQUEST,
            AppError::Unprocessable(_) => StatusCode::UNPROCESSABLE_ENTITY,
            AppError::Conflict(_) => StatusCode::CONFLICT,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_detail_never_reaches_the_client() {
        let err = AppError::Internal("geheimes Detail".into());
        assert_eq!(err.to_string(), "Interner Serverfehler");
        assert_eq!(err.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
}

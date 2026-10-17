//! Extractor, damit auch abgelehnte Bodies im `{"error"}`-Format ankommen (Muster aus Lifeline).

use axum::Json;
use axum::extract::{FromRequest, Request};
use serde::de::DeserializeOwned;

use crate::error::AppError;

pub struct JsonBody<T>(pub T);

impl<S, T> FromRequest<S> for JsonBody<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AppError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(req, state).await {
            Ok(Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(AppError::Validation(format!(
                "Ungültige Anfrage: {}",
                rejection.body_text()
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{post_json, send};
    use axum::Router;
    use axum::http::StatusCode;
    use axum::routing::post;

    #[derive(serde::Deserialize)]
    struct Name {
        name: String,
    }

    fn echo() -> Router {
        Router::new().route(
            "/echo",
            post(|JsonBody(body): JsonBody<Name>| async move { body.name }),
        )
    }

    #[tokio::test]
    async fn valid_body_reaches_the_handler() {
        let (status, body) = send(
            &echo(),
            post_json("/echo", serde_json::json!({ "name": "Trupp" })),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "Trupp");
    }

    #[tokio::test]
    async fn rejected_body_answers_with_error_json() {
        let (status, body) = send(
            &echo(),
            post_json("/echo", serde_json::json!({ "name": 7 })),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        let message = body["error"].as_str().expect("Feld error");
        assert!(message.starts_with("Ungültige Anfrage: "), "{message}");
    }
}

//! Verwaltung für Admins: Großbild-Tokens. B10 ergänzt die Einstellungen, B14 die Fernsteuerungen.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get};
use axum::{Json, Router};

use crate::app::AppState;
use crate::auth::session::Admin;
use crate::contract::{KioskTokenCreate, KioskTokenCreated, KioskTokenView};
use crate::error::AppError;
use crate::extract::JsonBody;
use crate::store;
use crate::time::now_ms;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/kiosk-tokens", get(list_kiosk).post(create_kiosk))
        .route("/kiosk-tokens/{id}", delete(delete_kiosk))
}

async fn list_kiosk(
    _: Admin,
    State(state): State<AppState>,
) -> Result<Json<Vec<KioskTokenView>>, AppError> {
    Ok(Json(store::kiosk::list(&state.pool).await?))
}

async fn create_kiosk(
    _: Admin,
    State(state): State<AppState>,
    JsonBody(body): JsonBody<KioskTokenCreate>,
) -> Result<Json<KioskTokenCreated>, AppError> {
    let label = body.label.trim();
    if label.is_empty() || label.chars().count() > 80 {
        return Err(AppError::Validation("Bezeichnung: 1 bis 80 Zeichen".into()));
    }
    let (token, secret) = store::kiosk::create(&state.pool, label, now_ms()).await?;
    let url = format!(
        "{}/kiosk#{secret}",
        state.boot.public_url.as_str().trim_end_matches('/')
    );
    Ok(Json(KioskTokenCreated { token, url }))
}

async fn delete_kiosk(
    _: Admin,
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    if store::kiosk::delete(&state.pool, id).await? {
        Ok(StatusCode::NO_CONTENT)
    } else {
        Err(AppError::NotFound)
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};

    use crate::contract::Role;
    use crate::testing::{post_json, send, test_app, with_cookie};

    #[tokio::test]
    async fn kiosk_label_is_checked_and_unknown_token_is_404() {
        let app = test_app().await;
        let admin = app.cookie(Role::Admin).await;
        for label in ["   ", &"x".repeat(81)] {
            let req = post_json("/api/kiosk-tokens", serde_json::json!({ "label": label }));
            let (status, body) = send(&app.router, with_cookie(req, &admin)).await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{label:?}");
            assert_eq!(body["error"], "Bezeichnung: 1 bis 80 Zeichen");
        }
        let req = post_json(
            "/api/kiosk-tokens",
            serde_json::json!({ "label": "  ELW 1  " }),
        );
        let (status, created) = send(&app.router, with_cookie(req, &admin)).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(created["token"]["label"], "ELW 1", "Bezeichnung ohne Rand");
        assert!(
            created["url"]
                .as_str()
                .unwrap()
                .starts_with("https://suchlage.test/kiosk#")
        );

        let req = Request::delete("/api/kiosk-tokens/999")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            send(&app.router, with_cookie(req, &admin)).await.0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn kiosk_tokens_need_a_session() {
        let app = test_app().await;
        let req = Request::get("/api/kiosk-tokens")
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(&app.router, req).await.0, StatusCode::UNAUTHORIZED);
    }
}

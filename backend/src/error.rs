use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;
use tracing::error;

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

/// Unified error type for all route handlers.
///
/// Implements Axum's `IntoResponse`, so handlers can return
/// `Result<impl IntoResponse, AppError>` and use `?` directly
/// instead of writing match arms for every fallible call.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum AppError {
    /// 400 — validation failures, malformed input
    #[error("{0}")]
    BadRequest(String),
    /// 401 — wrong credentials, expired/missing token
    #[error("{0}")]
    Unauthorized(String),
    /// 403 — action not permitted for this user
    #[error("{0}")]
    Forbidden(String),
    /// 404 — resource not found
    #[error("{0}")]
    NotFound(String),
    /// 409 — state conflict (e.g. closed session) or uniqueness violation (e.g. duplicate username)
    #[error("{0}")]
    Conflict(String),
    /// 500 — unexpected internal failures (DB, crypto, etc.)
    /// The String is a log-only message; the user sees "Internal server error".
    #[error("{0}")]
    Internal(String),
    /// 500 — JWT encode/decode failure. User-visible response is "Internal server error";
    /// the wrapped error is logged via the source chain at the response boundary.
    #[error("Token error")]
    Token(#[from] jsonwebtoken::errors::Error),
    /// 500 — password hashing/verification failure. Same response semantics as `Token`.
    #[error("Password hashing error")]
    Hash(#[from] argon2::password_hash::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, user_message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Internal(_) | AppError::Token(_) | AppError::Hash(_) => {
                error!("{}", format_error_chain(&self));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
        };

        (
            status,
            Json(ErrorBody {
                error: user_message,
            }),
        )
            .into_response()
    }
}

fn format_error_chain(err: &(dyn std::error::Error + 'static)) -> String {
    let mut parts = vec![err.to_string()];
    let mut source = err.source();
    while let Some(e) = source {
        parts.push(e.to_string());
        source = e.source();
    }
    parts.join(": ")
}

// ── From impls for common error types ──────────────────────────────
//
// The `Token` and `Hash` variants get their `From` impls from `#[from]` on the
// variant. `DbErr` needs hand-written discrimination — different `DbErr`
// variants map to different `AppError` variants (404 / 409 / 400 / 500), which
// `#[from]` can't express.

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self {
        use sea_orm::{DbErr, SqlErr};
        match &e {
            DbErr::RecordNotFound(msg) => AppError::NotFound(msg.clone()),
            _ => match e.sql_err() {
                Some(SqlErr::UniqueConstraintViolation(m)) => AppError::Conflict(m),
                Some(SqlErr::ForeignKeyConstraintViolation(m)) => AppError::BadRequest(m),
                _ => AppError::Internal(format!("Database error: {e}")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use sea_orm::{ActiveModelTrait, DbErr, Set};

    use super::*;
    use crate::{
        entities::users,
        test_helpers::{create_user, setup_db},
    };

    #[test]
    fn test_record_not_found_maps_to_not_found() {
        let err: AppError = DbErr::RecordNotFound("user not found".to_string()).into();
        match err {
            AppError::NotFound(msg) => assert_eq!(msg, "user not found"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_unrecognized_dberr_maps_to_internal() {
        // DbErr::Custom does not produce a SqlErr, so it should fall through to Internal.
        let err: AppError = DbErr::Custom("something went wrong".to_string()).into();
        match err {
            AppError::Internal(msg) => assert!(msg.contains("something went wrong")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_unique_constraint_violation_maps_to_conflict() {
        let db = setup_db().await;
        // First insert succeeds — username has a unique constraint.
        create_user(&db, "alice").await;

        // Second insert with the same username triggers UniqueConstraintViolation.
        let now = chrono::Utc::now().naive_utc();
        let result = users::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            username: Set("alice".to_string()),
            email: Set(None),
            password_hash: Set("placeholder".to_string()),
            preferred_character_id: Set(None),
            preferred_body_id: Set(None),
            preferred_wheel_id: Set(None),
            preferred_glider_id: Set(None),
            preferred_drink_type_id: Set(None),
            refresh_token_version: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await;

        let dberr = result.expect_err("duplicate username should fail");
        let app_err: AppError = dberr.into();
        match app_err {
            AppError::Conflict(_) => {}
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_foreign_key_constraint_violation_maps_to_bad_request() {
        let db = setup_db().await;

        // Insert a user with a preferred_character_id that doesn't exist in characters.
        let now = chrono::Utc::now().naive_utc();
        let result = users::ActiveModel {
            id: Set(uuid::Uuid::new_v4().to_string()),
            username: Set("bob".to_string()),
            email: Set(None),
            password_hash: Set("placeholder".to_string()),
            preferred_character_id: Set(Some(99_999)),
            preferred_body_id: Set(None),
            preferred_wheel_id: Set(None),
            preferred_glider_id: Set(None),
            preferred_drink_type_id: Set(None),
            refresh_token_version: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(&db)
        .await;

        let dberr = result.expect_err("missing FK should fail");
        let app_err: AppError = dberr.into();
        match app_err {
            AppError::BadRequest(_) => {}
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_jwt_error_maps_to_token_variant_with_source() {
        let jwt_err =
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken);
        let app_err: AppError = jwt_err.into();
        match &app_err {
            AppError::Token(_) => {}
            other => panic!("expected Token, got {other:?}"),
        }
        // The wrapped error must be reachable via the source chain so the
        // `IntoResponse` log gets the underlying jwt detail, not just "Token error".
        assert!(
            app_err.source().is_some(),
            "Token variant should expose its source"
        );
    }

    #[test]
    fn test_argon2_error_maps_to_hash_variant_with_source() {
        let app_err: AppError = argon2::password_hash::Error::Password.into();
        match &app_err {
            AppError::Hash(_) => {}
            other => panic!("expected Hash, got {other:?}"),
        }
        assert!(
            app_err.source().is_some(),
            "Hash variant should expose its source"
        );
    }

    #[test]
    fn test_format_error_chain_joins_sources() {
        let jwt_err =
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken);
        let app_err: AppError = jwt_err.into();
        let chain = format_error_chain(&app_err);
        // First segment is the variant's Display ("Token error"); the second is
        // the wrapped jwt error's Display, joined by ": ".
        assert!(chain.starts_with("Token error: "), "got: {chain}");
        assert!(
            chain.contains("InvalidToken") || chain.contains("invalid"),
            "got: {chain}"
        );
    }
}

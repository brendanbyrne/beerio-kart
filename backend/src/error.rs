use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
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
#[derive(Debug)]
pub enum AppError {
    /// 400 — validation failures, malformed input
    BadRequest(String),
    /// 401 — wrong credentials, expired/missing token
    Unauthorized(String),
    /// 403 — action not permitted for this user
    Forbidden(String),
    /// 404 — resource not found
    NotFound(String),
    /// 409 — duplicate username, etc.
    Conflict(String),
    /// 500 — unexpected internal failures (DB, crypto, etc.)
    /// The String is a log-only message; the user sees "Internal server error".
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, user_message) = match &self {
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, msg.clone()),
            AppError::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            AppError::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            AppError::Conflict(msg) => (StatusCode::CONFLICT, msg.clone()),
            AppError::Internal(log_msg) => {
                error!("{log_msg}");
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

// ── From impls for common error types ──────────────────────────────

impl From<sea_orm::DbErr> for AppError {
    fn from(e: sea_orm::DbErr) -> Self {
        AppError::Internal(format!("Database error: {e}"))
    }
}

impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(e: jsonwebtoken::errors::Error) -> Self {
        AppError::Internal(format!("Token error: {e}"))
    }
}

impl From<argon2::password_hash::Error> for AppError {
    fn from(e: argon2::password_hash::Error) -> Self {
        AppError::Internal(format!("Password hashing error: {e}"))
    }
}

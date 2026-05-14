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
/// `Result<impl IntoResponse, Error>` and use `?` directly
/// instead of writing match arms for every fallible call.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// 400 — validation failures, malformed input. `client` is the sanitized
    /// user-facing message; `detail` is internal-only context (e.g., a raw
    /// `DbErr` driver string) that is logged at the `IntoResponse` boundary
    /// and never returned to the client. Construct the no-detail common case
    /// via `Error::bad_request("...")`.
    #[error("{client}")]
    BadRequest {
        /// User-facing message included in the JSON response body.
        client: String,
        /// Internal-only context (e.g., a raw `DbErr` driver string) logged
        /// via `tracing::warn!` at the `IntoResponse` boundary and never
        /// returned to the client.
        detail: Option<String>,
    },
    /// 401 — wrong credentials, expired/missing token
    #[error("{0}")]
    Unauthorized(String),
    /// 403 — action not permitted for this user
    #[error("{0}")]
    Forbidden(String),
    /// 404 — resource not found
    #[error("{0}")]
    NotFound(String),
    /// 409 — state conflict (e.g. closed session) or uniqueness violation
    /// (e.g. duplicate username). Same `client` / `detail` shape as
    /// `BadRequest`; construct via `Error::conflict("...")` for the common case.
    #[error("{client}")]
    Conflict {
        /// User-facing message included in the JSON response body.
        client: String,
        /// Internal-only context (e.g., a raw `DbErr` driver string) logged
        /// via `tracing::warn!` at the `IntoResponse` boundary and never
        /// returned to the client.
        detail: Option<String>,
    },
    /// 500 — unexpected internal failures (DB, invariant violations, etc.).
    /// The wrapped `anyhow::Error` carries the call-site context plus any
    /// underlying source error; the user sees "Internal server error".
    ///
    /// Construct source-bearing internals as
    /// `anyhow::Error::new(e).context("Loading user")` and synthetic ones as
    /// `anyhow::anyhow!("Invariant violation: {detail}")`. The `IntoResponse`
    /// log path walks the full `error.source()` chain. Per `rust.md` § 1,
    /// context strings start with a capital letter and have no trailing
    /// punctuation.
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
    /// 500 — JWT encode/decode failure. User-visible response is "Internal server error";
    /// the wrapped error is logged via the source chain at the response boundary.
    #[error("Token error")]
    Token(#[from] jsonwebtoken::errors::Error),
    /// 500 — password hashing/verification failure. Same response semantics as `Token`.
    #[error("Password hashing error")]
    Hash(#[from] argon2::password_hash::Error),
}

impl Error {
    /// Build a `Conflict` with no internal detail. Use the `Conflict { client, detail }`
    /// struct-literal form when you have driver-string detail worth logging.
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            detail: None,
        }
    }

    /// Build a `BadRequest` with no internal detail. Use the struct-literal form
    /// when you have driver-string detail worth logging.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            detail: None,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let (status, user_message) = match &self {
            Self::BadRequest { client, detail } => {
                if let Some(d) = detail {
                    tracing::warn!(detail = %d, client = %client, "BadRequest");
                }
                (StatusCode::BAD_REQUEST, client.clone())
            }
            Self::Unauthorized(msg) => (StatusCode::UNAUTHORIZED, msg.clone()),
            Self::Forbidden(msg) => (StatusCode::FORBIDDEN, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Conflict { client, detail } => {
                if let Some(d) = detail {
                    tracing::warn!(detail = %d, client = %client, "Conflict");
                }
                (StatusCode::CONFLICT, client.clone())
            }
            Self::Internal(_) | Self::Token(_) | Self::Hash(_) => {
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
    std::iter::successors(Some(err), |e| e.source())
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(": ")
}

// ── From impls for common error types ──────────────────────────────
//
// The `Token` and `Hash` variants get their `From` impls from `#[from]` on the
// variant. `DbErr` needs hand-written discrimination — different `DbErr`
// variants map to different `Error` variants (404 / 409 / 400 / 500), which
// `#[from]` can't express.

impl From<sea_orm::DbErr> for Error {
    fn from(e: sea_orm::DbErr) -> Self {
        use sea_orm::{DbErr, SqlErr};
        match &e {
            DbErr::RecordNotFound(msg) => Self::NotFound(msg.clone()),
            _ => match e.sql_err() {
                // Driver strings like "UNIQUE constraint failed: users.username" leak
                // schema details. Stash them in `detail` (log-only) and return a
                // generic message to the client. Service-layer pre-checks remain the
                // way to get a *specific* 409/400 message — this is the safety net.
                Some(SqlErr::UniqueConstraintViolation(m)) => Self::Conflict {
                    client: "Resource already exists".into(),
                    detail: Some(m),
                },
                Some(SqlErr::ForeignKeyConstraintViolation(m)) => Self::BadRequest {
                    client: "Referenced record does not exist".into(),
                    detail: Some(m),
                },
                _ => Self::Internal(anyhow::Error::new(e).context("Database error")),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, DbErr, Set};

    use super::*;
    use crate::{
        entities::users,
        test_helpers::{create_user, setup_db},
    };

    #[test]
    fn test_record_not_found_maps_to_not_found() {
        let err: Error = DbErr::RecordNotFound("user not found".to_string()).into();
        match err {
            Error::NotFound(msg) => assert_eq!(msg, "user not found"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn test_unrecognized_dberr_maps_to_internal() {
        // DbErr::Custom does not produce a SqlErr, so it should fall through to Internal.
        let err: Error = DbErr::Custom("something went wrong".to_string()).into();
        match &err {
            Error::Internal(_) => {}
            other => panic!("expected Internal, got {other:?}"),
        }
        // The static "Database error" context is the topmost layer, the original
        // DbErr message is reachable via the source chain.
        let chain = format_error_chain(&err);
        assert!(chain.contains("Database error"), "got: {chain}");
        assert!(chain.contains("something went wrong"), "got: {chain}");
    }

    #[tokio::test]
    async fn test_unique_constraint_violation_maps_to_conflict() {
        let db = setup_db().await;
        // First insert succeeds — username has a unique constraint.
        create_user(&db, "alice").await;

        // Second insert with the same username triggers UniqueConstraintViolation.
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
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await;

        let dberr = result.expect_err("duplicate username should fail");
        let app_err: Error = dberr.into();
        match app_err {
            Error::Conflict { client, detail } => {
                // The client-facing message is the sanitized generic; the raw
                // driver detail is stashed for the log-only path.
                assert_eq!(client, "Resource already exists");
                let d = detail.expect("driver detail should be captured for logs");
                assert!(
                    d.contains("UNIQUE") || d.contains("unique"),
                    "expected driver detail to mention UNIQUE, got: {d}"
                );
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_foreign_key_constraint_violation_maps_to_bad_request() {
        let db = setup_db().await;

        // Insert a user with a preferred_character_id that doesn't exist in characters.
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
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await;

        let dberr = result.expect_err("missing FK should fail");
        let app_err: Error = dberr.into();
        match app_err {
            Error::BadRequest { client, detail } => {
                assert_eq!(client, "Referenced record does not exist");
                let d = detail.expect("driver detail should be captured for logs");
                assert!(
                    d.to_ascii_uppercase().contains("FOREIGN KEY"),
                    "expected driver detail to mention FOREIGN KEY, got: {d}"
                );
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_jwt_error_maps_to_token_variant_with_source() {
        let jwt_err =
            jsonwebtoken::errors::Error::from(jsonwebtoken::errors::ErrorKind::InvalidToken);
        let app_err: Error = jwt_err.into();
        match &app_err {
            Error::Token(_) => {}
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
        let app_err: Error = argon2::password_hash::Error::Password.into();
        match &app_err {
            Error::Hash(_) => {}
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
        let app_err: Error = jwt_err.into();
        let chain = format_error_chain(&app_err);
        // First segment is the variant's Display ("Token error"); the second is
        // the wrapped jwt error's Display, joined by ": ".
        assert!(chain.starts_with("Token error: "), "got: {chain}");
        assert!(
            chain.contains("InvalidToken") || chain.contains("invalid"),
            "got: {chain}"
        );
    }

    #[test]
    fn test_internal_synthetic_anyhow_round_trips() {
        // Synthetic Internal — no underlying error, just a runtime-formatted
        // message via anyhow::anyhow!. The chain should contain the message.
        let app_err: Error = anyhow::anyhow!("Invariant violation: {}", "stale state").into();
        match &app_err {
            Error::Internal(_) => {}
            other => panic!("expected Internal, got {other:?}"),
        }
        let chain = format_error_chain(&app_err);
        assert!(
            chain.contains("Invariant violation: stale state"),
            "got: {chain}"
        );
    }

    #[test]
    fn test_internal_source_bearing_chain_walks_context_then_source() {
        // Source-bearing Internal — anyhow::Error::new(source).context(static).
        // The chain walk should show the static context first, then the source's
        // Display. This is the shape produced by the From<DbErr> fallback.
        let inner = std::io::Error::other("disk gone");
        let app_err: Error = anyhow::Error::new(inner).context("Writing snapshot").into();
        let chain = format_error_chain(&app_err);
        assert!(chain.contains("Writing snapshot"), "got: {chain}");
        assert!(chain.contains("disk gone"), "got: {chain}");
    }

    // ── Helper constructors ────────────────────────────────────────────

    #[test]
    fn test_conflict_helper_sets_no_detail() {
        let err = Error::conflict("Username already taken");
        match err {
            Error::Conflict { client, detail } => {
                assert_eq!(client, "Username already taken");
                assert!(detail.is_none());
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn test_bad_request_helper_sets_no_detail() {
        let err = Error::bad_request("Invalid input");
        match err {
            Error::BadRequest { client, detail } => {
                assert_eq!(client, "Invalid input");
                assert!(detail.is_none());
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    // ── Response-body sanitization ─────────────────────────────────────
    //
    // These tests are the regression-blocker for Issue #84: the response body
    // must contain only the sanitized `client` text, never the raw `detail`
    // (which is allowed to contain table/column names from the DB driver).

    #[tokio::test]
    async fn test_conflict_response_body_omits_driver_detail() {
        let err = Error::Conflict {
            client: "Resource already exists".into(),
            detail: Some("UNIQUE constraint failed: users.email".into()),
        };
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body collect");
        let body = std::str::from_utf8(&bytes).expect("utf-8 body");
        assert!(
            !body.contains("UNIQUE"),
            "response leaked driver detail: {body}"
        );
        assert!(
            !body.contains("users.email"),
            "response leaked schema name: {body}"
        );
        assert!(
            body.contains("Resource already exists"),
            "response missing client text: {body}"
        );
    }

    #[tokio::test]
    async fn test_bad_request_response_body_omits_driver_detail() {
        let err = Error::BadRequest {
            client: "Referenced record does not exist".into(),
            detail: Some("FOREIGN KEY constraint failed".into()),
        };
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body collect");
        let body = std::str::from_utf8(&bytes).expect("utf-8 body");
        assert!(
            !body.contains("FOREIGN KEY"),
            "response leaked driver detail: {body}"
        );
        assert!(
            body.contains("Referenced record does not exist"),
            "response missing client text: {body}"
        );
    }

    // ── Tracing capture: detail must still reach logs ─────────────────
    //
    // The companion to the response-body tests above. The whole point of
    // keeping the driver detail in the `detail` field is to preserve it for
    // operators — losing it entirely would have been simpler but lost
    // debuggability. If this test starts failing, the warn! call site or the
    // `detail` plumbing has regressed.

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_conflict_with_detail_emits_warn_with_driver_detail() {
        let err = Error::Conflict {
            client: "Resource already exists".into(),
            detail: Some("UNIQUE constraint failed: users.email".into()),
        };
        let _resp = err.into_response();
        assert!(
            logs_contain("UNIQUE constraint failed: users.email"),
            "driver detail missing from captured logs"
        );
        assert!(
            logs_contain("client=Resource already exists"),
            "client field missing from captured logs"
        );
        assert!(
            logs_contain("Conflict"),
            "warn message missing from captured logs"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_bad_request_with_detail_emits_warn_with_driver_detail() {
        let err = Error::BadRequest {
            client: "Referenced record does not exist".into(),
            detail: Some("FOREIGN KEY constraint failed".into()),
        };
        let _resp = err.into_response();
        assert!(
            logs_contain("FOREIGN KEY constraint failed"),
            "driver detail missing from captured logs"
        );
        assert!(
            logs_contain("client=Referenced record does not exist"),
            "client field missing from captured logs"
        );
        assert!(
            logs_contain("BadRequest"),
            "warn message missing from captured logs"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_conflict_without_detail_emits_no_warn() {
        // The helper-constructed common case has no detail; the warn! call
        // is gated on Some(_), so nothing should land in the log buffer.
        let err = Error::conflict("Username already taken");
        let _resp = err.into_response();
        assert!(
            !logs_contain("Conflict"),
            "no-detail Conflict should not emit a warn"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_bad_request_without_detail_emits_no_warn() {
        // Symmetric with test_conflict_without_detail_emits_no_warn — the
        // helper-built no-detail case must not emit a warn for either arm.
        let err = Error::bad_request("Invalid input");
        let _resp = err.into_response();
        assert!(
            !logs_contain("BadRequest"),
            "no-detail BadRequest should not emit a warn"
        );
    }
}

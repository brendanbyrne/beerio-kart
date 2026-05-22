use std::time::Duration;

use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Serialize;
use thiserror::Error;
use tracing::{error, warn};

/// Stable error-code identifiers emitted in the `code` field of every error
/// response.
///
/// Mirrors the `api-contract.md` § 7 registry one-to-one — adding a new code
/// means adding a row there and a variant here in the same change.
///
/// Codes are public API contract: once the frontend starts pattern-matching
/// on them they become forever decisions. Rename via deprecation, never
/// in place.
///
/// `#[serde(rename_all = "snake_case")]` serializes variants as the snake-case
/// strings the registry documents (`bad_request`, `lap_times_mismatch`,
/// `gateway_timeout`, …).
#[derive(Copy, Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ErrorCode {
    // 400
    /// Generic validation failure (free-text message).
    BadRequest,
    /// Lap times don't sum to total time.
    LapTimesMismatch,
    /// Submitted `track_id` doesn't match the `session_race`'s track.
    TrackIdMismatch,
    /// Path-segment parse failure (e.g., non-UUID where `Path<SessionId>` is declared).
    InvalidPathParam,
    /// JSON body failed to parse or deserialize.
    InvalidRequestBody,
    // 401
    /// Login failed.
    InvalidCredentials,
    /// Access token expired (frontend should refresh).
    TokenExpired,
    /// Token malformed, missing, signature mismatch, or otherwise rejected.
    TokenInvalid,
    // 403
    /// Authenticated but not authorized for this action.
    Forbidden,
    /// Endpoint requires admin.
    AdminRequired,
    // 404
    /// Generic "resource doesn't exist".
    NotFound,
    // 409
    /// Generic state conflict.
    Conflict,
    /// Registration conflict.
    UsernameTaken,
    /// Submission to a closed session.
    SessionClosed,
    /// Must resolve pending races before current race.
    PendingRacesFirst,
    /// Pending race must be submitted in order.
    OutOfOrderSubmission,
    /// Concurrent `next-track` race lost (idempotency-key retry will return the winning response).
    RaceNumberConflict,
    // 500
    /// Unexpected internal failure. Frontend shows a generic message.
    Internal,
    // 504
    /// Per-call timeout budget elapsed; retry is safe.
    GatewayTimeout,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
    code: ErrorCode,
}

/// Unified error type for all route handlers.
///
/// Implements Axum's `IntoResponse`, so handlers can return
/// `Result<impl IntoResponse, Error>` and use `?` directly
/// instead of writing match arms for every fallible call.
///
/// Every variant carries (or is associated with) an [`ErrorCode`] that
/// surfaces in the response body's `code` field. Variants that span multiple
/// codes (`BadRequest`, `Conflict`, `Unauthorized`, `Forbidden`) carry the
/// code in a struct field; variants pinned to a single code derive theirs
/// from [`Error::code`].
///
/// Construction style:
/// - **Per-code helpers** for the named domain codes
///   (`Error::lap_times_mismatch(msg)`, `Error::username_taken(msg)`, etc.) —
///   readable at call sites, type-safe pairing of (HTTP status, code).
/// - **Generic helpers** for bespoke long-tail errors that share the
///   generic code: `Error::bad_request(msg)` → `ErrorCode::BadRequest`,
///   `Error::conflict(msg)` → `ErrorCode::Conflict`,
///   `Error::forbidden(msg)` → `ErrorCode::Forbidden`. 401 has no generic —
///   every `Unauthorized` must pick one of `invalid_credentials`,
///   `token_expired`, or `token_invalid`.
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// 400 — validation failures, malformed input. `client` is the sanitized
    /// user-facing message; `detail` is internal-only context (e.g., a raw
    /// `DbErr` driver string) that is logged at the `IntoResponse` boundary
    /// and never returned to the client. Construct the no-detail common case
    /// via `Error::bad_request("...")`; use per-code helpers for named codes
    /// (`Error::lap_times_mismatch`, `Error::track_id_mismatch`,
    /// `Error::invalid_path_param`, `Error::invalid_request_body`).
    #[error("{client}")]
    BadRequest {
        /// User-facing message included in the JSON response body.
        client: String,
        /// Specific code from the registry — drives the response's `code` field.
        code: ErrorCode,
        /// Internal-only context (e.g., a raw `DbErr` driver string) logged
        /// via `tracing::warn!` at the `IntoResponse` boundary and never
        /// returned to the client.
        detail: Option<String>,
    },
    /// 401 — wrong credentials, expired/missing/malformed token. 401 has no
    /// generic code — every construction picks one of `InvalidCredentials`,
    /// `TokenExpired`, or `TokenInvalid` via the corresponding helper
    /// (`Error::invalid_credentials()`, `Error::token_expired()`,
    /// `Error::token_invalid()`).
    #[error("{msg}")]
    Unauthorized {
        /// User-facing message included in the JSON response body.
        msg: String,
        /// Specific 401 code from the registry.
        code: ErrorCode,
    },
    /// 403 — action not permitted for this user. Generic case via
    /// `Error::forbidden("...")` → `ErrorCode::Forbidden`; admin-only paths
    /// use `Error::admin_required()` → `ErrorCode::AdminRequired`.
    #[error("{msg}")]
    Forbidden {
        /// User-facing message included in the JSON response body.
        msg: String,
        /// Specific 403 code from the registry.
        code: ErrorCode,
    },
    /// 404 — resource not found. Single code; tuple form retained.
    #[error("{0}")]
    NotFound(String),
    /// 409 — state conflict (e.g. closed session) or uniqueness violation
    /// (e.g. duplicate username). Same `client` / `detail` shape as
    /// `BadRequest`; construct via `Error::conflict("...")` for the common
    /// case, or use per-code helpers (`Error::username_taken`,
    /// `Error::session_closed`, `Error::pending_races_first`,
    /// `Error::out_of_order_submission`, `Error::race_number_conflict`).
    #[error("{client}")]
    Conflict {
        /// User-facing message included in the JSON response body.
        client: String,
        /// Specific code from the registry — drives the response's `code` field.
        code: ErrorCode,
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
    /// 504 — a per-call `tokio::time::timeout` budget elapsed before the wrapped
    /// future completed. Currently raised only by `timeout::db_query` /
    /// `timeout::db_txn` around `SeaORM` calls; `SQLite` is the "upstream" in
    /// the 504 sense, so the proxy-flavoured status code is semantically apt
    /// even though the database is in-process. Distinct from `Internal` so
    /// operators can chart timeouts independently of generic 500-class failures
    /// (a stuck query is an operational signal, not a bug-class one).
    #[error("Operation timed out after {budget:?}")]
    Timeout {
        /// Budget that elapsed. Logged via `tracing::warn!` at the
        /// `IntoResponse` boundary; not returned to the client.
        budget: Duration,
    },
}

impl Error {
    /// Return the [`ErrorCode`] this error surfaces in the response's `code`
    /// field. For variants that carry a code field, returns that value; for
    /// variants pinned to a single code, returns the constant.
    #[must_use]
    pub const fn code(&self) -> ErrorCode {
        match self {
            Self::BadRequest { code, .. }
            | Self::Unauthorized { code, .. }
            | Self::Forbidden { code, .. }
            | Self::Conflict { code, .. } => *code,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::Internal(_) | Self::Token(_) | Self::Hash(_) => ErrorCode::Internal,
            Self::Timeout { .. } => ErrorCode::GatewayTimeout,
        }
    }

    // ── Generic helpers ───────────────────────────────────────────────────

    /// Build a `BadRequest` with no detail and the generic `BadRequest` code.
    /// Use the struct-literal form when you have driver-string detail or
    /// reach for one of the per-code helpers below for named registry codes.
    pub fn bad_request(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            code: ErrorCode::BadRequest,
            detail: None,
        }
    }

    /// Build a `Conflict` with no detail and the generic `Conflict` code.
    pub fn conflict(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::Conflict,
            detail: None,
        }
    }

    /// Build a `Forbidden` with the generic `Forbidden` code.
    pub fn forbidden(msg: impl Into<String>) -> Self {
        Self::Forbidden {
            msg: msg.into(),
            code: ErrorCode::Forbidden,
        }
    }

    // ── 400 per-code helpers ──────────────────────────────────────────────

    /// 400 / `lap_times_mismatch` — lap times don't sum to the total time.
    pub fn lap_times_mismatch(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            code: ErrorCode::LapTimesMismatch,
            detail: None,
        }
    }

    /// 400 / `track_id_mismatch` — submitted `track_id` doesn't match the
    /// `session_race`'s track.
    pub fn track_id_mismatch(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            code: ErrorCode::TrackIdMismatch,
            detail: None,
        }
    }

    /// 400 / `invalid_path_param` — URL path segment failed to parse into the
    /// typed `Path<T>` extractor. Emitted by the project-local Path extractor
    /// when `axum::extract::rejection::PathRejection` fires.
    pub fn invalid_path_param(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            code: ErrorCode::InvalidPathParam,
            detail: None,
        }
    }

    /// 400 / `invalid_request_body` — JSON body failed to parse or deserialize
    /// into the typed `Json<T>` extractor. Covers both syntactic parse
    /// failures and newtype-deserialization rejections (e.g., a value that
    /// passes JSON parsing but violates a domain newtype's invariant).
    pub fn invalid_request_body(msg: impl Into<String>) -> Self {
        Self::BadRequest {
            client: msg.into(),
            code: ErrorCode::InvalidRequestBody,
            detail: None,
        }
    }

    // ── 401 per-code helpers (no generic — 401 always picks a specific code) ─

    /// 401 / `invalid_credentials` — login failed (wrong username or password).
    /// Both failure modes surface the same sentinel message to prevent
    /// username enumeration via response-differentiation.
    #[must_use]
    pub fn invalid_credentials() -> Self {
        Self::Unauthorized {
            msg: "Invalid username or password".to_string(),
            code: ErrorCode::InvalidCredentials,
        }
    }

    /// 401 / `token_expired` — access token expired; frontend should refresh.
    #[must_use]
    pub fn token_expired() -> Self {
        Self::Unauthorized {
            msg: "Access token expired".to_string(),
            code: ErrorCode::TokenExpired,
        }
    }

    /// 401 / `token_invalid` — token malformed, missing, signature mismatch,
    /// revoked, or otherwise unusable. Custom message lets the call site
    /// surface the specific reason ("Missing refresh token",
    /// "Refresh token has been revoked", etc.) without altering the code.
    pub fn token_invalid(msg: impl Into<String>) -> Self {
        Self::Unauthorized {
            msg: msg.into(),
            code: ErrorCode::TokenInvalid,
        }
    }

    // ── 403 per-code helper ───────────────────────────────────────────────

    /// 403 / `admin_required` — the endpoint requires admin privileges.
    #[must_use]
    pub fn admin_required() -> Self {
        Self::Forbidden {
            msg: "Admin access required".to_string(),
            code: ErrorCode::AdminRequired,
        }
    }

    // ── 409 per-code helpers ──────────────────────────────────────────────

    /// 409 / `username_taken` — registration conflict.
    pub fn username_taken(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::UsernameTaken,
            detail: None,
        }
    }

    /// 409 / `session_closed` — operation against a closed session.
    pub fn session_closed(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::SessionClosed,
            detail: None,
        }
    }

    /// 409 / `pending_races_first` — must resolve pending races before
    /// submitting to the current race.
    pub fn pending_races_first(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::PendingRacesFirst,
            detail: None,
        }
    }

    /// 409 / `out_of_order_submission` — pending race must be submitted in
    /// order; submitting a newer race before resolving an older one.
    pub fn out_of_order_submission(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::OutOfOrderSubmission,
            detail: None,
        }
    }

    /// 409 / `race_number_conflict` — concurrent `next-track` race lost; the
    /// idempotency-key retry path will return the winning response.
    pub fn race_number_conflict(msg: impl Into<String>) -> Self {
        Self::Conflict {
            client: msg.into(),
            code: ErrorCode::RaceNumberConflict,
            detail: None,
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let code = self.code();
        let (status, user_message) = match &self {
            Self::BadRequest { client, detail, .. } => {
                if let Some(d) = detail {
                    tracing::warn!(detail = %d, client = %client, ?code, "BadRequest");
                }
                (StatusCode::BAD_REQUEST, client.clone())
            }
            Self::Unauthorized { msg, .. } => (StatusCode::UNAUTHORIZED, msg.clone()),
            Self::Forbidden { msg, .. } => (StatusCode::FORBIDDEN, msg.clone()),
            Self::NotFound(msg) => (StatusCode::NOT_FOUND, msg.clone()),
            Self::Conflict { client, detail, .. } => {
                if let Some(d) = detail {
                    tracing::warn!(detail = %d, client = %client, ?code, "Conflict");
                }
                (StatusCode::CONFLICT, client.clone())
            }
            Self::Internal(_) | Self::Token(_) | Self::Hash(_) => {
                error!(?code, "{}", format_error_chain(&self));
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Internal server error".to_string(),
                )
            }
            Self::Timeout { budget } => {
                // `as_secs()` returns `u64` and `subsec_millis()` returns
                // `u32` (<1000), so this expression is direct integer math
                // with no fallible cast — no `u64::MAX` sentinel masking an
                // overflow as a real-looking number in the logs. The 2s /
                // 5s budgets are nowhere near u64's range.
                warn!(
                    ?code,
                    budget_ms = budget.as_secs() * 1000 + u64::from(budget.subsec_millis()),
                    "Operation timed out"
                );
                (StatusCode::GATEWAY_TIMEOUT, "Request timed out".to_string())
            }
        };

        (
            status,
            Json(ErrorBody {
                error: user_message,
                code,
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
                // way to get a *specific* 409/400 message (e.g., `username_taken`)
                // — this is the safety net and stays on the generic codes.
                Some(SqlErr::UniqueConstraintViolation(m)) => Self::Conflict {
                    client: "Resource already exists".into(),
                    code: ErrorCode::Conflict,
                    detail: Some(m),
                },
                Some(SqlErr::ForeignKeyConstraintViolation(m)) => Self::BadRequest {
                    client: "Referenced record does not exist".into(),
                    code: ErrorCode::BadRequest,
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
            Error::Conflict {
                client,
                code,
                detail,
            } => {
                // The client-facing message is the sanitized generic; the raw
                // driver detail is stashed for the log-only path. The code is
                // the generic Conflict — service-layer pre-checks are the
                // path to specific codes like `username_taken`.
                assert_eq!(client, "Resource already exists");
                assert_eq!(code, ErrorCode::Conflict);
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
            Error::BadRequest {
                client,
                code,
                detail,
            } => {
                assert_eq!(client, "Referenced record does not exist");
                assert_eq!(code, ErrorCode::BadRequest);
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
    fn test_conflict_helper_sets_generic_code_no_detail() {
        let err = Error::conflict("Generic 409");
        match err {
            Error::Conflict {
                client,
                code,
                detail,
            } => {
                assert_eq!(client, "Generic 409");
                assert_eq!(code, ErrorCode::Conflict);
                assert!(detail.is_none());
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn test_bad_request_helper_sets_generic_code_no_detail() {
        let err = Error::bad_request("Invalid input");
        match err {
            Error::BadRequest {
                client,
                code,
                detail,
            } => {
                assert_eq!(client, "Invalid input");
                assert_eq!(code, ErrorCode::BadRequest);
                assert!(detail.is_none());
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_per_code_helpers_assign_their_codes() {
        // Sweep through every per-code helper; each must produce the matching
        // ErrorCode. This is the regression guard against a copy-paste bug
        // where a new helper accidentally picks the wrong code.
        assert_eq!(
            Error::lap_times_mismatch("x").code(),
            ErrorCode::LapTimesMismatch
        );
        assert_eq!(
            Error::track_id_mismatch("x").code(),
            ErrorCode::TrackIdMismatch
        );
        assert_eq!(
            Error::invalid_path_param("x").code(),
            ErrorCode::InvalidPathParam
        );
        assert_eq!(
            Error::invalid_request_body("x").code(),
            ErrorCode::InvalidRequestBody
        );
        assert_eq!(
            Error::invalid_credentials().code(),
            ErrorCode::InvalidCredentials
        );
        assert_eq!(Error::token_expired().code(), ErrorCode::TokenExpired);
        assert_eq!(Error::token_invalid("x").code(), ErrorCode::TokenInvalid);
        assert_eq!(Error::admin_required().code(), ErrorCode::AdminRequired);
        assert_eq!(Error::username_taken("x").code(), ErrorCode::UsernameTaken);
        assert_eq!(Error::session_closed("x").code(), ErrorCode::SessionClosed);
        assert_eq!(
            Error::pending_races_first("x").code(),
            ErrorCode::PendingRacesFirst
        );
        assert_eq!(
            Error::out_of_order_submission("x").code(),
            ErrorCode::OutOfOrderSubmission
        );
        assert_eq!(
            Error::race_number_conflict("x").code(),
            ErrorCode::RaceNumberConflict
        );
        assert_eq!(Error::forbidden("x").code(), ErrorCode::Forbidden);
        assert_eq!(Error::bad_request("x").code(), ErrorCode::BadRequest);
        assert_eq!(Error::conflict("x").code(), ErrorCode::Conflict);
    }

    // ── Response-body sanitization ─────────────────────────────────────
    //
    // These tests are the regression-blocker for Issue #84: the response body
    // must contain only the sanitized `client` text, never the raw `detail`
    // (which is allowed to contain table/column names from the DB driver).

    #[tokio::test]
    async fn test_conflict_response_body_omits_driver_detail_and_includes_code() {
        let err = Error::Conflict {
            client: "Resource already exists".into(),
            code: ErrorCode::Conflict,
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
        assert!(
            body.contains("\"code\":\"conflict\""),
            "response missing code field: {body}"
        );
    }

    #[tokio::test]
    async fn test_bad_request_response_body_omits_driver_detail_and_includes_code() {
        let err = Error::BadRequest {
            client: "Referenced record does not exist".into(),
            code: ErrorCode::BadRequest,
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
        assert!(
            body.contains("\"code\":\"bad_request\""),
            "response missing code field: {body}"
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
            code: ErrorCode::Conflict,
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
            code: ErrorCode::BadRequest,
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

    // ── 500-class wire-shape (Internal / Token / Hash share one match arm) ──
    //
    // `code()` and the per-helper round-trip tests cover the variant→code
    // mapping; this test guards the response-body serialization for the
    // 500-class arm directly. If `IntoResponse`'s 500 path stops emitting
    // `code` (e.g., a future refactor returns a hand-rolled body that omits
    // the field), this is the regression-blocker. `Internal` is the
    // representative — `Token` and `Hash` flow through the same match arm,
    // so one wire-shape test covers all three.

    #[tokio::test]
    async fn test_internal_response_body_includes_internal_code() {
        let err: Error = anyhow::anyhow!("boom").into();
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body collect");
        let body = std::str::from_utf8(&bytes).expect("utf-8 body");
        assert!(
            body.contains("\"code\":\"internal\""),
            "response missing code field: {body}"
        );
        assert!(
            body.contains("Internal server error"),
            "response missing user-facing message: {body}"
        );
        // Internal must not leak the underlying error message to the client.
        assert!(
            !body.contains("boom"),
            "response leaked underlying error chain: {body}"
        );
    }

    // ── Timeout variant ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_timeout_response_status_is_504_gateway_timeout() {
        let err = Error::Timeout {
            budget: Duration::from_secs(2),
        };
        let resp = err.into_response();
        assert_eq!(resp.status(), StatusCode::GATEWAY_TIMEOUT);
    }

    #[tokio::test]
    async fn test_timeout_response_body_includes_code_and_omits_budget() {
        // The budget is operator-relevant context that goes to logs, not to the
        // client. The response body must not leak it (the user doesn't need to
        // know we set a 2s ceiling) and must use a stable user-facing message,
        // and must carry the `gateway_timeout` code.
        let err = Error::Timeout {
            budget: Duration::from_secs(2),
        };
        let resp = err.into_response();
        let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
            .await
            .expect("body collect");
        let body = std::str::from_utf8(&bytes).expect("utf-8 body");
        assert!(
            body.contains("Request timed out"),
            "response missing user-facing message: {body}"
        );
        assert!(
            !body.contains("2s") && !body.contains("2000"),
            "response leaked budget to client: {body}"
        );
        assert!(
            body.contains("\"code\":\"gateway_timeout\""),
            "response missing code field: {body}"
        );
    }

    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_timeout_emits_warn_with_budget_ms() {
        let err = Error::Timeout {
            budget: Duration::from_millis(2_500),
        };
        let _resp = err.into_response();
        assert!(
            logs_contain("budget_ms=2500"),
            "budget field missing from captured logs"
        );
        assert!(
            logs_contain("Operation timed out"),
            "warn message missing from captured logs"
        );
    }
}

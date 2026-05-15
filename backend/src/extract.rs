//! Project-local request extractors with envelope-conforming rejections.
//!
//! Wraps Axum's [`axum::extract::Path`] and [`axum::Json`] with rejection
//! handlers that produce the standard
//! [error envelope](crate::error::Error) — `{ "error": "...", "code": "..." }`
//! — instead of Axum's default plain-text bodies. See [Issue #146] for the
//! original motivation; [Issue #157] tracks the broader `code` field rollout
//! these extractors plug into.
//!
//! [Issue #146]: https://github.com/brendanbyrne/beerio-kart/issues/146
//! [Issue #157]: https://github.com/brendanbyrne/beerio-kart/issues/157
//!
//! Use:
//!
//! ```ignore
//! use crate::extract::{Json, Path};
//!
//! pub async fn get_session(
//!     Path(session_id): Path<SessionId>,
//! ) -> Result<Json<SessionDetail>, Error> { /* ... */ }
//! ```
//!
//! Drop-in replacements for `axum::extract::Path` and `axum::Json`. The tuple
//! shape matches axum's so destructuring (`Path(x): Path<T>`) keeps working.

use axum::{
    extract::{FromRequest, FromRequestParts, Request, rejection::JsonRejection},
    http::request::Parts,
    response::{IntoResponse, Response},
};
use serde::{Serialize, de::DeserializeOwned};

use crate::error::Error;

/// Typed path-segment extractor wrapping [`axum::extract::Path`].
///
/// Rejection failures (non-UUID where `Path<SessionId>` declared, etc.)
/// become [`Error::invalid_path_param`] with the standard JSON envelope
/// (400 + `code: "invalid_path_param"`) instead of axum's default
/// plain-text body.
pub struct Path<T>(pub T);

impl<T, S> FromRequestParts<S> for Path<T>
where
    T: DeserializeOwned + Send,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        match axum::extract::Path::<T>::from_request_parts(parts, state).await {
            Ok(axum::extract::Path(value)) => Ok(Self(value)),
            Err(rejection) => Err(Error::invalid_path_param(rejection.body_text())),
        }
    }
}

/// Typed JSON body extractor + response wrapper wrapping [`axum::Json`].
///
/// **Request side:** rejection failures (malformed JSON, type mismatches,
/// newtype invariant failures, missing or wrong `Content-Type`) become
/// [`Error::invalid_request_body`] with the standard JSON envelope (400 +
/// `code: "invalid_request_body"`).
///
/// **Response side:** serializes via `axum::Json`. Standard
/// `application/json` response, identical to `axum::Json` on the wire.
///
/// All `JsonRejection` variants collapse to a single 400 code in this pass.
/// 422 `unprocessable` is registered-but-unimplemented for now; splitting
/// 400/422 is a follow-up if data-validation surfaces enough to need its own
/// status. The original content-type-mismatch case (axum's default 415) folds
/// in here — a minor status-code change for an edge case in exchange for a
/// uniform error-body shape.
pub struct Json<T>(pub T);

impl<T, S> FromRequest<S> for Json<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = Error;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(req, state).await {
            Ok(axum::Json(value)) => Ok(Self(value)),
            Err(rejection) => Err(Error::invalid_request_body(json_rejection_text(&rejection))),
        }
    }
}

impl<T> IntoResponse for Json<T>
where
    T: Serialize,
{
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// Build a user-facing message from a `JsonRejection`. The default
/// `body_text()` is descriptive (Axum produces strings like
/// `"Failed to parse the request body as JSON: expected value at line ..."`)
/// which is fine for clients to receive — it doesn't leak server state and
/// helps developers correct their requests.
fn json_rejection_text(rejection: &JsonRejection) -> String {
    rejection.body_text()
}

#[cfg(test)]
mod tests {
    use axum::{Router, body::Body, http::Request, routing::post};
    use serde::Deserialize;
    use tower::ServiceExt;

    use super::*;
    use crate::error::ErrorCode;

    // ── Path extractor ────────────────────────────────────────────────────

    async fn path_id_handler(Path(id): Path<i32>) -> String {
        id.to_string()
    }

    #[tokio::test]
    async fn test_path_extractor_invalid_returns_400_with_envelope() {
        let app = Router::new().route("/items/{id}", axum::routing::get(path_id_handler));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/items/not-an-i32")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), 400);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("response body is JSON");
        assert_eq!(body["code"], "invalid_path_param");
        assert!(
            body["error"].as_str().is_some_and(|s| !s.is_empty()),
            "response missing user-facing error message: {body}"
        );
    }

    #[tokio::test]
    async fn test_path_extractor_valid_path_segment_extracts() {
        let app = Router::new().route("/items/{id}", axum::routing::get(path_id_handler));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/items/42")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(&bytes[..], b"42");
    }

    // ── Json extractor ────────────────────────────────────────────────────

    #[derive(Deserialize)]
    struct Echo {
        msg: String,
    }

    async fn json_echo_handler(Json(echo): Json<Echo>) -> String {
        echo.msg
    }

    #[tokio::test]
    async fn test_json_extractor_malformed_body_returns_400_with_envelope() {
        let app = Router::new().route("/echo", post(json_echo_handler));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .header("content-type", "application/json")
                    .body(Body::from("{ this is not valid json"))
                    .expect("build request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), 400);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("response body is JSON");
        assert_eq!(body["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn test_json_extractor_wrong_type_returns_400_with_envelope() {
        // Schema mismatch: `msg` is declared as `String` but the body sends an
        // integer. Axum's serde-deserialize path rejects with `JsonDataError`,
        // which our wrapper folds into the same 400 envelope.
        let app = Router::new().route("/echo", post(json_echo_handler));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"msg": 42}"#))
                    .expect("build request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), 400);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let body: serde_json::Value =
            serde_json::from_slice(&bytes).expect("response body is JSON");
        assert_eq!(body["code"], "invalid_request_body");
    }

    #[tokio::test]
    async fn test_json_extractor_valid_body_extracts() {
        let app = Router::new().route("/echo", post(json_echo_handler));
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/echo")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"msg": "hello"}"#))
                    .expect("build request"),
            )
            .await
            .expect("oneshot");

        assert_eq!(response.status(), 200);
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        assert_eq!(&bytes[..], b"hello");
    }

    // ── Code-mapping unit test ────────────────────────────────────────────

    #[test]
    fn test_invalid_path_param_helper_produces_correct_code() {
        let err = Error::invalid_path_param("Invalid URL: parameter could not be parsed");
        assert_eq!(err.code(), ErrorCode::InvalidPathParam);
    }

    #[test]
    fn test_invalid_request_body_helper_produces_correct_code() {
        let err = Error::invalid_request_body("Failed to parse JSON body");
        assert_eq!(err.code(), ErrorCode::InvalidRequestBody);
    }
}

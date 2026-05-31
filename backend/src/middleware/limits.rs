//! Error mapping for request-shape middleware (tokio.md § 12).
//!
//! Lives here rather than in `main.rs` so it's reachable from the lib's
//! test binary. The function is paired with `tower::load_shed::LoadShedLayer`
//! and `axum::error_handling::HandleErrorLayer` in `main.rs`'s router stack.

use axum::{
    BoxError, Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

/// Maps `LoadShedLayer`'s `Overloaded` error to a 503 JSON response.
///
/// Paired with `tower::load_shed::LoadShedLayer` around
/// `ConcurrencyLimitLayer` in `main.rs`. Anything other than `Overloaded`
/// reaching this handler is a programming bug (no other layer above
/// `HandleErrorLayer` errors fallibly today), so the fallback is 500.
///
/// The response body shape matches the project-wide JSON-error contract
/// (`{ "error": "<message>" }`) documented in `docs/api-contract.md` § 2
/// (Error response contract) and implemented for the normal `Error::IntoResponse`
/// path in `error.rs`.
//
// `async` looks unused (no `.await`), but `HandleErrorLayer::new` requires
// a fn returning `Future`. Drop the `async` and the layer no longer
// compiles. Allow the lint locally rather than rewriting to a manual
// `impl Future` return.
#[allow(clippy::unused_async)]
pub async fn handle_load_shed_error(err: BoxError) -> Response {
    if err.is::<tower::load_shed::error::Overloaded>() {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "Service overloaded" })),
        )
            .into_response()
    } else {
        tracing::error!(error = %err, "Unexpected error reached load-shed handler");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "Internal server error" })),
        )
            .into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::{body::to_bytes, http::header::CONTENT_TYPE};
    use tower::load_shed::error::Overloaded;

    use super::*;

    /// Pull the response body into a `serde_json::Value` so tests can assert
    /// on the JSON contract directly rather than substring-matching a string.
    /// `to_bytes` with `usize::MAX` is fine for tests — bodies here are
    /// small and bounded.
    async fn body_json(response: Response) -> (StatusCode, Option<String>, serde_json::Value) {
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|h| h.to_str().ok())
            .map(str::to_owned);
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let json: serde_json::Value = serde_json::from_slice(&bytes).expect("body parses as JSON");
        (status, content_type, json)
    }

    #[tokio::test]
    async fn overloaded_becomes_503_json_error() {
        let response = handle_load_shed_error(BoxError::from(Overloaded::new())).await;
        let (status, content_type, body) = body_json(response).await;

        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        // Catches the regression that motivated this test: a plain-text
        // body would fail `from_slice` above. Asserting the shape here is
        // the contract from `docs/api-contract.md` § 2 (Error response contract).
        assert_eq!(body, json!({ "error": "Service overloaded" }));
    }

    #[tokio::test]
    async fn unexpected_error_becomes_500_json_error() {
        // Any non-`Overloaded` error reaching this handler is a programming
        // bug — no other fallible layer sits above HandleErrorLayer today.
        // Map to 500 with the same message string the rest of the codebase
        // uses (`error.rs:113`) so client-side error grouping stays stable.
        #[derive(Debug)]
        struct Bogus;
        impl std::fmt::Display for Bogus {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("bogus")
            }
        }
        impl std::error::Error for Bogus {}

        let response = handle_load_shed_error(BoxError::from(Bogus)).await;
        let (status, content_type, body) = body_json(response).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(content_type.as_deref(), Some("application/json"));
        assert_eq!(body, json!({ "error": "Internal server error" }));
    }
}

//! Integration tests for the request-shape Tower middleware (tokio.md § 12).
//!
//! These exercise the *layers themselves* against minimal routers — not the
//! production router from `main.rs` — which keeps the tests deterministic and
//! fast. The production wiring (limits sourced from `Config`, ordering, the
//! `ConnectInfo` make-service) is verified by build + manual `curl` per the
//! Issue's Verification section; what's left to assert in code is the
//! layer-by-layer behavior at the boundary value.

// Tests legitimately want to panic — per rust.md § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use axum::{Router, body::Bytes, http::StatusCode, routing::post};
use axum_test::TestServer;
use tower_http::limit::RequestBodyLimitLayer;

/// Consume the body so the `RequestBodyLimitLayer` triggers on the
/// streaming-read path (axum-test sends bodies with chunked transfer encoding
/// by default, so there's no `Content-Length` for the layer to short-circuit
/// on — the limit fires when the handler tries to read past the cap).
async fn read_body(body: Bytes) -> StatusCode {
    let _ = body;
    StatusCode::OK
}

#[tokio::test]
async fn request_body_limit_rejects_oversized_with_413() {
    // 16 bytes is small enough to overflow with a trivial payload, large
    // enough that the under-limit case is non-empty.
    const LIMIT: usize = 16;

    let app = Router::new()
        .route("/echo", post(read_body))
        .layer(RequestBodyLimitLayer::new(LIMIT));
    let server = TestServer::new(app);

    let under = Bytes::from(vec![b'x'; LIMIT]);
    let response = server.post("/echo").bytes(under).await;
    response.assert_status(StatusCode::OK);

    let over = Bytes::from(vec![b'x'; LIMIT + 1]);
    let response = server.post("/echo").bytes(over).await;
    response.assert_status(StatusCode::PAYLOAD_TOO_LARGE);
}

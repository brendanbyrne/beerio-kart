//! Integration tests for the JWT bearer auth middleware.

// Tests legitimately want to panic — per rust.md § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::{Router, http::StatusCode, routing::get};
use axum_test::TestServer;
use beerio_kart::{
    ARGON2_MAX_CONCURRENT, AppState,
    config::Config,
    domain::{UserId, Username},
    middleware::auth::{AdminUser, User},
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde_json::Value;
use tokio::sync::Semaphore;
use uuid::Uuid;

const TEST_SECRET: &str = "middleware-test-secret";

/// Minimal handler that requires User.
async fn auth_handler(user: User) -> axum::Json<Value> {
    axum::Json(serde_json::json!({ "user_id": user.user_id }))
}

/// Minimal handler that requires `AdminUser`.
async fn admin_handler(admin: AdminUser) -> axum::Json<Value> {
    axum::Json(serde_json::json!({ "admin_id": admin.user_id }))
}

fn make_config(admin_user_id: Option<UserId>) -> Arc<Config> {
    Arc::new(Config {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id,
        cookie_secure: false,
        request_timeout_seconds: 30,
        request_concurrency_limit: 100,
        max_request_body_bytes: 10 * 1024 * 1024,
        rate_limit_per_minute: 60,
    })
}

async fn setup_server(admin_user_id: Option<UserId>) -> TestServer {
    let url = format!("sqlite:file:{}?mode=memory&cache=shared", Uuid::new_v4());
    let db = Database::connect(&url).await.expect("connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("pragma");
    Migrator::up(&db, None).await.expect("migrate");

    let config = make_config(admin_user_id);
    let state = AppState {
        db,
        config,
        argon2_limit: Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT)),
    };

    let app = Router::new()
        .route("/auth-only", get(auth_handler))
        .route("/admin-only", get(admin_handler))
        .with_state(state);

    TestServer::new(app)
}

fn create_access_token(user_id: &UserId, username: &str) -> String {
    let config = make_config(None);
    let username = Username::try_from(username.to_string()).expect("valid test username");
    beerio_kart::services::auth::create_access_token(user_id, &username, &config).unwrap()
}

fn create_refresh_token(user_id: &UserId) -> String {
    let config = make_config(None);
    beerio_kart::services::auth::create_refresh_token(user_id, 0, &config).unwrap()
}

// ── User extractor tests ────────────────────────────────────────

#[tokio::test]
async fn test_auth_user_missing_header_returns_401() {
    let server = setup_server(None).await;
    let response = server.get("/auth-only").await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_user_malformed_header_no_bearer_returns_401() {
    let server = setup_server(None).await;
    let response = server
        .get("/auth-only")
        .add_header("Authorization", "Token abc123")
        .await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_user_empty_token_returns_401() {
    let server = setup_server(None).await;
    let response = server
        .get("/auth-only")
        .add_header("Authorization", "Bearer ")
        .await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_user_refresh_token_as_access_returns_401() {
    let server = setup_server(None).await;
    let user_id = UserId::new_v4();
    let refresh = create_refresh_token(&user_id);
    let response = server
        .get("/auth-only")
        .add_header("Authorization", format!("Bearer {refresh}"))
        .await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_user_valid_access_token_succeeds() {
    let server = setup_server(None).await;
    let user_id = UserId::new_v4();
    let token = create_access_token(&user_id, "alice");
    let response = server
        .get("/auth-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["user_id"], user_id.to_string());
}

// ── AdminUser extractor tests ───────────────────────────────────────

#[tokio::test]
async fn test_admin_user_non_admin_returns_403() {
    let admin_id = UserId::new_v4();
    let server = setup_server(Some(admin_id)).await;
    let other = UserId::new_v4();
    let token = create_access_token(&other, "bob");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_user_correct_admin_succeeds() {
    let admin_id = UserId::new_v4();
    let server = setup_server(Some(admin_id)).await;
    let token = create_access_token(&admin_id, "admin");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["admin_id"], admin_id.to_string());
}

#[tokio::test]
async fn test_admin_user_no_admin_configured_returns_403() {
    let server = setup_server(None).await; // no ADMIN_USER_ID set
    let user_id = UserId::new_v4();
    let token = create_access_token(&user_id, "alice");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

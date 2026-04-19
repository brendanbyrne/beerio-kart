use std::sync::Arc;

use axum::{Router, http::StatusCode, routing::get};
use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde_json::Value;

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::middleware::auth::{AdminUser, AuthUser};

const TEST_SECRET: &str = "middleware-test-secret";

/// Minimal handler that requires AuthUser.
async fn auth_handler(user: AuthUser) -> axum::Json<Value> {
    axum::Json(serde_json::json!({ "user_id": user.user_id }))
}

/// Minimal handler that requires AdminUser.
async fn admin_handler(admin: AdminUser) -> axum::Json<Value> {
    axum::Json(serde_json::json!({ "admin_id": admin.user_id }))
}

fn make_config(admin_user_id: Option<&str>) -> Arc<AppConfig> {
    Arc::new(AppConfig {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id: admin_user_id.map(|s| s.to_string()),
        cookie_secure: false,
    })
}

async fn setup_server(admin_user_id: Option<&str>) -> TestServer {
    let db = Database::connect("sqlite::memory:").await.expect("connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("pragma");
    Migrator::up(&db, None).await.expect("migrate");

    let config = make_config(admin_user_id);
    let state = AppState { db, config };

    let app = Router::new()
        .route("/auth-only", get(auth_handler))
        .route("/admin-only", get(admin_handler))
        .with_state(state);

    TestServer::new(app)
}

fn create_access_token(user_id: &str, username: &str) -> String {
    let config = make_config(None);
    beerio_kart::services::auth::create_access_token(user_id, username, &config).unwrap()
}

fn create_refresh_token(user_id: &str) -> String {
    let config = make_config(None);
    beerio_kart::services::auth::create_refresh_token(user_id, 0, &config).unwrap()
}

// ── AuthUser extractor tests ────────────────────────────────────────

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
    let refresh = create_refresh_token("user-1");
    let response = server
        .get("/auth-only")
        .add_header("Authorization", format!("Bearer {refresh}"))
        .await;
    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_auth_user_valid_access_token_succeeds() {
    let server = setup_server(None).await;
    let token = create_access_token("user-1", "alice");
    let response = server
        .get("/auth-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["user_id"], "user-1");
}

// ── AdminUser extractor tests ───────────────────────────────────────

#[tokio::test]
async fn test_admin_user_non_admin_returns_403() {
    let server = setup_server(Some("admin-id")).await;
    let token = create_access_token("not-admin", "bob");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_admin_user_correct_admin_succeeds() {
    let server = setup_server(Some("admin-id")).await;
    let token = create_access_token("admin-id", "admin");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["admin_id"], "admin-id");
}

#[tokio::test]
async fn test_admin_user_no_admin_configured_returns_403() {
    let server = setup_server(None).await; // no ADMIN_USER_ID set
    let token = create_access_token("some-user", "alice");
    let response = server
        .get("/admin-only")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;
    response.assert_status(StatusCode::FORBIDDEN);
}

use std::sync::Arc;

use axum::{Router, http::StatusCode, routing::post};
use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde_json::{Value, json};

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::routes;

/// Create a fresh in-memory SQLite database with all migrations applied.
async fn setup_test_app() -> TestServer {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite");

    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("Failed to enable foreign keys");

    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    let config = Arc::new(AppConfig {
        jwt_secret: "test-secret-for-integration-tests".to_string(),
        jwt_expiry_hours: 24,
        admin_user_id: None,
    });

    let state = AppState { db, config };

    let app = Router::new()
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .with_state(state);

    TestServer::new(app)
}

#[tokio::test]
async fn test_register_success_returns_201_and_jwt() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "alice", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::CREATED);

    let body: Value = response.json();
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], "alice");
    assert!(body["user"]["id"].is_string());
    let id = body["user"]["id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok());
}

#[tokio::test]
async fn test_register_duplicate_username_returns_409() {
    let server = setup_test_app().await;

    server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "bob", "password": "password123" }))
        .await
        .assert_status(StatusCode::CREATED);

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "bob", "password": "different456" }))
        .await;

    response.assert_status(StatusCode::CONFLICT);
    let body: Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("already taken"));
}

#[tokio::test]
async fn test_register_username_too_long_returns_400() {
    let server = setup_test_app().await;

    let long_name = "a".repeat(31);
    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": long_name, "password": "password123" }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_register_empty_username_returns_400() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "  ", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_register_short_password_returns_400() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "charlie", "password": "short" }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_success_returns_200_and_jwt() {
    let server = setup_test_app().await;

    server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "dave", "password": "password123" }))
        .await
        .assert_status(StatusCode::CREATED);

    let response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "dave", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::OK);

    let body: Value = response.json();
    assert!(body["token"].is_string());
    assert_eq!(body["user"]["username"], "dave");
}

#[tokio::test]
async fn test_login_wrong_password_returns_401() {
    let server = setup_test_app().await;

    server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "eve", "password": "password123" }))
        .await
        .assert_status(StatusCode::CREATED);

    let response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "eve", "password": "wrongpassword" }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert_eq!(body["error"], "Invalid username or password");
}

#[tokio::test]
async fn test_login_nonexistent_user_returns_401() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "nobody", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert_eq!(body["error"], "Invalid username or password");
}

#[tokio::test]
async fn test_login_returns_same_user_id_as_register() {
    let server = setup_test_app().await;

    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "frank", "password": "password123" }))
        .await;
    let reg_body: Value = reg_response.json();
    let registered_id = reg_body["user"]["id"].as_str().unwrap().to_string();

    let login_response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "frank", "password": "password123" }))
        .await;
    let login_body: Value = login_response.json();
    let logged_in_id = login_body["user"]["id"].as_str().unwrap();

    assert_eq!(registered_id, logged_in_id);
}

#[tokio::test]
async fn test_logout_returns_200() {
    let server = setup_test_app().await;

    let response = server.post("/api/v1/auth/logout").await;
    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_jwt_contains_correct_sub_claim() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "grace", "password": "password123" }))
        .await;

    let body: Value = response.json();
    let token = body["token"].as_str().unwrap();
    let user_id = body["user"]["id"].as_str().unwrap();

    let config = AppConfig {
        jwt_secret: "test-secret-for-integration-tests".to_string(),
        jwt_expiry_hours: 24,
        admin_user_id: None,
    };
    let claims = beerio_kart::services::auth::validate_token(token, &config).unwrap();
    assert_eq!(claims.sub, user_id);
    assert_eq!(claims.username, "grace");
}

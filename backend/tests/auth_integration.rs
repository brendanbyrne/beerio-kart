use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    routing::{get, post, put},
};
use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde_json::{Value, json};

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::routes;

const TEST_SECRET: &str = "test-secret-for-integration-tests";

fn test_config() -> Arc<AppConfig> {
    Arc::new(AppConfig {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id: None,
        cookie_secure: false,
    })
}

/// A trivial authenticated endpoint used to verify that the AuthUser extractor works.
async fn protected_hello(user: beerio_kart::middleware::auth::AuthUser) -> axum::Json<Value> {
    axum::Json(json!({ "hello": user.username }))
}

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

    let config = test_config();
    let state = AppState { db, config };

    let app = Router::new()
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/refresh", post(routes::auth::refresh))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .route("/api/v1/auth/password", put(routes::auth::change_password))
        .route("/api/v1/protected", get(protected_hello))
        .with_state(state);

    TestServer::new(app)
}

/// Helper: register a user and return the response body.
async fn register_user(server: &TestServer, username: &str, password: &str) -> Value {
    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": username, "password": password }))
        .await;
    response.assert_status(StatusCode::CREATED);
    response.json()
}

/// Helper: extract the refresh_token cookie value from a Set-Cookie header.
fn extract_refresh_cookie(response: &axum_test::TestResponse) -> Option<String> {
    let header = response.header("set-cookie");
    let header_str = header.to_str().ok()?;
    // Cookie format: refresh_token=<value>; HttpOnly; ...
    header_str
        .split(';')
        .next()?
        .strip_prefix("refresh_token=")
        .map(|s| s.to_string())
}

// ── Existing tests (updated for new response format) ────────────────

#[tokio::test]
async fn test_register_success_returns_201_with_access_token_and_refresh_cookie() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "alice", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::CREATED);

    let body: Value = response.json();
    assert!(
        body["access_token"].is_string(),
        "should return access_token"
    );
    assert_eq!(body["user"]["username"], "alice");
    assert!(body["user"]["id"].is_string());
    let id = body["user"]["id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok());

    // Should set a refresh cookie
    let cookie = extract_refresh_cookie(&response);
    assert!(cookie.is_some(), "should set refresh_token cookie");
    assert!(!cookie.unwrap().is_empty(), "cookie should not be empty");
}

#[tokio::test]
async fn test_login_success_returns_access_token_and_refresh_cookie() {
    let server = setup_test_app().await;
    register_user(&server, "dave", "password123").await;

    let response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "dave", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::OK);

    let body: Value = response.json();
    assert!(
        body["access_token"].is_string(),
        "should return access_token"
    );
    assert_eq!(body["user"]["username"], "dave");

    let cookie = extract_refresh_cookie(&response);
    assert!(cookie.is_some(), "should set refresh_token cookie");
}

#[tokio::test]
async fn test_refresh_with_valid_cookie_returns_new_access_token_and_rotated_cookie() {
    let server = setup_test_app().await;

    // Register to get a refresh cookie
    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "carol", "password": "password123" }))
        .await;
    let cookie = extract_refresh_cookie(&reg_response).unwrap();

    // Use the refresh cookie to get a new access token
    let refresh_response = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie))
        .await;

    refresh_response.assert_status(StatusCode::OK);

    let body: Value = refresh_response.json();
    assert!(
        body["access_token"].is_string(),
        "should return new access_token"
    );

    // Should also rotate the refresh cookie
    let new_cookie = extract_refresh_cookie(&refresh_response);
    assert!(new_cookie.is_some(), "should set rotated refresh cookie");
}

#[tokio::test]
async fn test_refresh_without_cookie_returns_401() {
    let server = setup_test_app().await;

    let response = server.post("/api/v1/auth/refresh").await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("Missing"));
}

#[tokio::test]
async fn test_refresh_with_wrong_version_returns_401() {
    let server = setup_test_app().await;

    // Register to get tokens
    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "mallory", "password": "password123" }))
        .await;
    let reg_body: Value = reg_response.json();
    let access_token = reg_body["access_token"].as_str().unwrap();
    let refresh_cookie_val = extract_refresh_cookie(&reg_response).unwrap();

    // Logout bumps refresh_token_version, invalidating the cookie
    server
        .post("/api/v1/auth/logout")
        .add_header("Authorization", format!("Bearer {access_token}"))
        .await
        .assert_status(StatusCode::OK);

    // Try to use the old refresh cookie — should fail
    let response = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &refresh_cookie_val))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("revoked"));
}

#[tokio::test]
async fn test_logout_increments_version_and_clears_cookie() {
    let server = setup_test_app().await;

    // Register and get tokens
    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "logan", "password": "password123" }))
        .await;
    let reg_body: Value = reg_response.json();
    let access_token = reg_body["access_token"].as_str().unwrap();
    let refresh_cookie_val = extract_refresh_cookie(&reg_response).unwrap();

    // Logout
    let logout_response = server
        .post("/api/v1/auth/logout")
        .add_header("Authorization", format!("Bearer {access_token}"))
        .await;

    logout_response.assert_status(StatusCode::OK);

    // The logout response should clear the cookie (Max-Age=0)
    let header_val = logout_response.header("set-cookie");
    let set_cookie = header_val.to_str().unwrap();
    assert!(
        set_cookie.contains("Max-Age=0"),
        "should clear cookie with Max-Age=0"
    );

    // Old refresh cookie should now fail
    let refresh_response = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &refresh_cookie_val))
        .await;
    refresh_response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_password_change_increments_version_and_returns_new_tokens() {
    let server = setup_test_app().await;

    // Register
    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "pat", "password": "oldpass123" }))
        .await;
    let reg_body: Value = reg_response.json();
    let access_token = reg_body["access_token"].as_str().unwrap();
    let old_refresh = extract_refresh_cookie(&reg_response).unwrap();

    // Change password
    let pw_response = server
        .put("/api/v1/auth/password")
        .json(&json!({
            "current_password": "oldpass123",
            "new_password": "newpass456"
        }))
        .add_header("Authorization", format!("Bearer {access_token}"))
        .await;

    pw_response.assert_status(StatusCode::OK);
    let pw_body: Value = pw_response.json();
    assert!(
        pw_body["access_token"].is_string(),
        "should return new access_token"
    );

    // New refresh cookie should be set
    let new_refresh = extract_refresh_cookie(&pw_response);
    assert!(new_refresh.is_some(), "should set new refresh cookie");

    // Old refresh token should now fail (version was bumped)
    let old_refresh_response = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &old_refresh))
        .await;
    old_refresh_response.assert_status(StatusCode::UNAUTHORIZED);

    // Login with new password should work
    let login_response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "pat", "password": "newpass456" }))
        .await;
    login_response.assert_status(StatusCode::OK);

    // Login with old password should fail
    let old_login = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "pat", "password": "oldpass123" }))
        .await;
    old_login.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_refresh_token_in_authorization_header_rejected_by_middleware() {
    let server = setup_test_app().await;

    // Create a refresh token directly
    let config = test_config();
    let refresh_jwt =
        beerio_kart::services::auth::create_refresh_token("fake-user-id", 0, &config).unwrap();

    // Try to use it as a Bearer token on a protected endpoint
    let response = server
        .get("/api/v1/protected")
        .add_header("Authorization", format!("Bearer {refresh_jwt}"))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_access_token_with_type_refresh_rejected_by_middleware() {
    let server = setup_test_app().await;

    // Create a refresh token and try to pass it off as an access token.
    // The middleware should check token_type and reject it.
    let config = test_config();
    let refresh_jwt =
        beerio_kart::services::auth::create_refresh_token("fake-user-id", 0, &config).unwrap();

    let response = server
        .get("/api/v1/protected")
        .add_header("Authorization", format!("Bearer {refresh_jwt}"))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_expired_access_token_rejected_by_middleware() {
    use jsonwebtoken::{EncodingKey, Header, encode};
    use serde_json::json as json_val;

    let server = setup_test_app().await;

    // Create a token with exp 2 minutes in the past (beyond jsonwebtoken's
    // default 60-second leeway).
    let past = chrono::Utc::now().timestamp() - 120;
    let claims = json_val!({
        "sub": "user-1",
        "username": "test",
        "exp": past,
        "iat": past - 60,
        "token_type": "access",
    });

    let token = encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
    )
    .unwrap();

    let response = server
        .get("/api/v1/protected")
        .add_header("Authorization", format!("Bearer {token}"))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
}

// ── Preserved existing tests (updated for new API) ──────────────────

#[tokio::test]
async fn test_register_duplicate_username_returns_409() {
    let server = setup_test_app().await;
    register_user(&server, "bob", "password123").await;

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
async fn test_register_long_password_returns_400() {
    let server = setup_test_app().await;

    let long_password = "a".repeat(129);
    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "testuser", "password": long_password }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_login_wrong_password_returns_401() {
    let server = setup_test_app().await;
    register_user(&server, "eve", "password123").await;

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

    let reg_body = register_user(&server, "frank", "password123").await;
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
async fn test_register_unicode_username_within_30_chars() {
    let server = setup_test_app().await;

    let unicode_name: String = "é".repeat(30);
    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": unicode_name, "password": "password123" }))
        .await;

    response.assert_status(StatusCode::CREATED);
}

#[tokio::test]
async fn test_second_logout_with_valid_access_token_returns_200() {
    let server = setup_test_app().await;

    let reg_response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "repeat", "password": "password123" }))
        .await;
    let reg_body: Value = reg_response.json();
    let access_token = reg_body["access_token"].as_str().unwrap();

    // First logout — bumps refresh_token_version from 0 to 1
    server
        .post("/api/v1/auth/logout")
        .add_header("Authorization", format!("Bearer {access_token}"))
        .await
        .assert_status(StatusCode::OK);

    // Second logout — bumps version again (1 to 2). Not idempotent: each
    // call increments the version, invalidating any refresh tokens issued
    // between the two calls. But the endpoint itself doesn't error.
    server
        .post("/api/v1/auth/logout")
        .add_header("Authorization", format!("Bearer {access_token}"))
        .await
        .assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_login_trims_whitespace_like_register() {
    let server = setup_test_app().await;

    register_user(&server, "  padded  ", "password123").await;

    let response = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "  padded  ", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::OK);
    let body: Value = response.json();
    assert_eq!(body["user"]["username"], "padded");
}

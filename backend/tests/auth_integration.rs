//! Integration tests for the auth route handlers (register, login, refresh, logout, change-password).

// Tests legitimately want to panic — per rust.md § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::{
    Router,
    http::StatusCode,
    routing::{get, post, put},
};
use axum_test::TestServer;
use beerio_kart::{ARGON2_MAX_CONCURRENT, AppState, config::Config, db, routes};
use migration::{Migrator, MigratorTrait};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use uuid::Uuid;

const TEST_SECRET: &str = "test-secret-for-integration-tests";

fn test_config() -> Arc<Config> {
    config_with_grace(10)
}

/// A test config with an explicit refresh-token reuse-detection grace window.
/// `grace = 0` makes any presentation of an already-used token count as reuse,
/// which is what the reuse / multi-device tests rely on for determinism; the
/// default `10` exercises the within-grace reissue path.
fn config_with_grace(refresh_grace_seconds: i64) -> Arc<Config> {
    Arc::new(Config {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id: None,
        cookie_secure: false,
        refresh_grace_seconds,
        request_timeout_seconds: 30,
        request_concurrency_limit: 100,
        max_request_body_bytes: 10 * 1024 * 1024,
        rate_limit_per_minute: 60,
    })
}

/// A trivial authenticated endpoint used to verify that the User extractor works.
async fn protected_hello(user: beerio_kart::middleware::auth::User) -> axum::Json<Value> {
    axum::Json(json!({ "hello": user.username }))
}

/// Create a fresh in-memory `SQLite` database with all migrations applied.
async fn setup_test_app() -> TestServer {
    setup_test_app_with_config(test_config()).await
}

/// Like [`setup_test_app`] but with a caller-supplied config — used by the
/// reuse-detection tests to set the grace window (e.g. `config_with_grace(0)`).
async fn setup_test_app_with_config(config: Arc<Config>) -> TestServer {
    let url = format!("sqlite:file:{}?mode=memory&cache=shared", Uuid::new_v4());
    // `db::connect` enables per-pool-connection FKs — see seaorm.md § 8 / #140.
    let db = db::connect(&url)
        .await
        .expect("Failed to connect to in-memory SQLite");

    Migrator::up(&db, None)
        .await
        .expect("Failed to run migrations");

    let state = AppState {
        db,
        config,
        argon2_limit: Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT)),
    };

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

/// Helper: extract the `refresh_token` cookie value from a Set-Cookie header.
fn extract_refresh_cookie(response: &axum_test::TestResponse) -> Option<String> {
    let header = response.header("set-cookie");
    let header_str = header.to_str().ok()?;
    // Cookie format: refresh_token=<value>; HttpOnly; ...
    header_str
        .split(';')
        .next()?
        .strip_prefix("refresh_token=")
        .map(ToString::to_string)
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
    Uuid::parse_str(id).unwrap();

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

    // The rotated cookie's *value* now differs from the original. Each minted
    // refresh token carries a unique `jti` (ADR-0040), so two tokens are never
    // byte-identical — the former flaky-`assert_ne!` concern (no `jti`,
    // second-granular `exp`) is gone.
    let new_cookie =
        extract_refresh_cookie(&refresh_response).expect("should set rotated refresh cookie");
    assert_ne!(
        new_cookie, cookie,
        "the rotated cookie must differ from the original (unique jti)"
    );

    // And it's functional: the rotated cookie is itself a working refresh token.
    let second_refresh = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &new_cookie))
        .await;
    second_refresh.assert_status(StatusCode::OK);
    let body2: Value = second_refresh.json();
    assert!(
        body2["access_token"].is_string(),
        "the rotated cookie must itself be usable to refresh"
    );
}

// ── Refresh-token rotation + reuse detection (ADR-0040) ─────────────

#[tokio::test]
async fn test_refresh_reuse_detected_revokes_family() {
    // grace = 0: any presentation of an already-rotated token counts as reuse.
    let server = setup_test_app_with_config(config_with_grace(0)).await;

    let reg = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "reuser", "password": "password123" }))
        .await;
    let cookie1 = extract_refresh_cookie(&reg).unwrap();

    // First refresh rotates: cookie1 is now used; cookie2 is the live tip.
    let r1 = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie1))
        .await;
    r1.assert_status(StatusCode::OK);
    let cookie2 = extract_refresh_cookie(&r1).unwrap();

    // Replaying the original (used) cookie past the grace window is reuse.
    let reuse = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie1))
        .await;
    reuse.assert_status(StatusCode::UNAUTHORIZED);
    let reuse_body: Value = reuse.json();
    assert_eq!(reuse_body["code"], "token_reuse_detected");

    // Reuse revokes the WHOLE family — even the legitimate live successor is
    // now rejected, forcing a full re-auth (the RFC 9700 property).
    let after = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie2))
        .await;
    after.assert_status(StatusCode::UNAUTHORIZED);
    let after_body: Value = after.json();
    assert_eq!(after_body["code"], "token_invalid");
}

#[tokio::test]
async fn test_refresh_within_grace_window_reissues_instead_of_revoking() {
    // Default grace (10 s): a token presented again right after it rotated is a
    // retry / race, not theft — it reissues the family's live successor.
    let server = setup_test_app().await;

    let reg = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "racer", "password": "password123" }))
        .await;
    let cookie1 = extract_refresh_cookie(&reg).unwrap();

    let r1 = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie1))
        .await;
    r1.assert_status(StatusCode::OK);

    // Immediately replay cookie1 (well within the 10 s window).
    let retry = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &cookie1))
        .await;
    retry.assert_status(StatusCode::OK);
    let retry_body: Value = retry.json();
    assert!(
        retry_body["access_token"].is_string(),
        "a within-grace replay should reissue a working token, not 401"
    );

    // The reissued cookie is the live tip and still refreshes.
    let reissued = extract_refresh_cookie(&retry).unwrap();
    let again = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &reissued))
        .await;
    again.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn test_two_families_coexist_and_reuse_revokes_only_one() {
    // grace = 0 for deterministic reuse. Register (family A) then log in again
    // (family B) as the SAME user — two independent devices.
    let server = setup_test_app_with_config(config_with_grace(0)).await;

    let reg = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "multi", "password": "password123" }))
        .await;
    let a1 = extract_refresh_cookie(&reg).unwrap();

    let login = server
        .post("/api/v1/auth/login")
        .json(&json!({ "username": "multi", "password": "password123" }))
        .await;
    login.assert_status(StatusCode::OK);
    let b1 = extract_refresh_cookie(&login).unwrap();

    // Rotate + reuse on family A.
    server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &a1))
        .await
        .assert_status(StatusCode::OK);
    let reuse_a = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &a1))
        .await;
    reuse_a.assert_status(StatusCode::UNAUTHORIZED);
    let reuse_a_body: Value = reuse_a.json();
    assert_eq!(reuse_a_body["code"], "token_reuse_detected");

    // Family B is untouched — the other device still refreshes. (Reuse revokes
    // by family, and does not bump the global `refresh_token_version`.)
    let rb = server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &b1))
        .await;
    rb.assert_status(StatusCode::OK);
    let rb_body: Value = rb.json();
    assert!(rb_body["access_token"].is_string());
}

#[tokio::test]
async fn test_refresh_without_cookie_returns_401() {
    let server = setup_test_app().await;

    let response = server.post("/api/v1/auth/refresh").await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    let body: Value = response.json();
    assert!(body["error"].as_str().unwrap().contains("Missing"));
    assert_eq!(body["code"], "token_invalid");
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
    assert_eq!(body["code"], "token_invalid");
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

    // New refresh cookie should be set, and it starts a fresh family for the
    // current session — so it must itself be usable to refresh (ADR-0040: the
    // family clear runs before this mint, so the new cookie isn't swept away).
    let new_refresh = extract_refresh_cookie(&pw_response).expect("should set new refresh cookie");
    server
        .post("/api/v1/auth/refresh")
        .add_cookie(cookie::Cookie::new("refresh_token", &new_refresh))
        .await
        .assert_status(StatusCode::OK);

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
async fn test_refresh_token_used_as_access_token_rejected_by_middleware() {
    // A refresh token is a valid JWT signed with the same key, but it carries
    // `refresh_token_version` instead of `username`, so it can't deserialize as
    // `AccessClaims` — the middleware rejects it as `token_invalid` at decode,
    // before the explicit `token_type` guard is even reached. The security
    // property under test is that a refresh token never authenticates as an
    // access token. (Merged from the former byte-identical
    // `test_refresh_token_in_authorization_header_rejected_by_middleware` +
    // `test_access_token_with_type_refresh_rejected_by_middleware`.)
    let server = setup_test_app().await;

    let config = test_config();
    let refresh_jwt = beerio_kart::services::auth::create_refresh_token(
        &beerio_kart::domain::UserId::new_v4(),
        0,
        &Uuid::new_v4().to_string(),
        &Uuid::new_v4().to_string(),
        &config,
    )
    .unwrap();

    // Try to use it as a Bearer token on a protected endpoint.
    let response = server
        .get("/api/v1/protected")
        .add_header("Authorization", format!("Bearer {refresh_jwt}"))
        .await;

    response.assert_status(StatusCode::UNAUTHORIZED);
    // Rejected as `token_invalid` ("Invalid token") — the refresh token is
    // well-formed and correctly signed, so this is the claim-shape mismatch
    // being caught, not a signature failure.
    let body: Value = response.json();
    assert_eq!(body["code"], "token_invalid");
    assert!(body["error"].as_str().unwrap().contains("Invalid token"));
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
    // Expired (not malformed): the middleware must surface `token_expired` so
    // the frontend can refresh rather than re-prompting for credentials.
    let body: Value = response.json();
    assert_eq!(body["code"], "token_expired");
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
    assert_eq!(body["code"], "username_taken");
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
    let body: Value = response.json();
    assert_eq!(body["code"], "bad_request");
    assert!(body["error"].as_str().unwrap().contains("Username"));
}

#[tokio::test]
async fn test_register_empty_username_returns_400() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "  ", "password": "password123" }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    let body: Value = response.json();
    assert_eq!(body["code"], "bad_request");
    assert!(body["error"].as_str().unwrap().contains("Username"));
}

#[tokio::test]
async fn test_register_short_password_returns_400() {
    let server = setup_test_app().await;

    let response = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": "charlie", "password": "short" }))
        .await;

    response.assert_status(StatusCode::BAD_REQUEST);
    let body: Value = response.json();
    assert_eq!(body["code"], "bad_request");
    assert!(body["error"].as_str().unwrap().contains("Password"));
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
    let body: Value = response.json();
    assert_eq!(body["code"], "bad_request");
    assert!(body["error"].as_str().unwrap().contains("Password"));
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
    assert_eq!(body["code"], "invalid_credentials");
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
    assert_eq!(body["code"], "invalid_credentials");
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

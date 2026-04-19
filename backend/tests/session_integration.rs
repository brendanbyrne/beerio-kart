//! Integration tests for session lifecycle endpoints.

use std::sync::Arc;

use axum::{
    Router,
    http::header::{HeaderName, HeaderValue},
    routing::{get, post},
};
use axum_test::TestServer;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, Set};
use serde_json::{Value, json};

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::routes;

const TEST_SECRET: &str = "test-secret-for-session-tests";

fn test_config() -> Arc<AppConfig> {
    Arc::new(AppConfig {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id: None,
        cookie_secure: false,
    })
}

async fn setup_test_app() -> (TestServer, sea_orm::DatabaseConnection) {
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
    let state = AppState {
        db: db.clone(),
        config,
    };

    let app = Router::new()
        // Auth (needed for registration/login)
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        // Sessions
        .route(
            "/api/v1/sessions",
            get(routes::sessions::list_sessions).post(routes::sessions::create_session),
        )
        .route("/api/v1/sessions/{id}", get(routes::sessions::get_session))
        .route(
            "/api/v1/sessions/{id}/join",
            post(routes::sessions::join_session),
        )
        .route(
            "/api/v1/sessions/{id}/leave",
            post(routes::sessions::leave_session),
        )
        .with_state(state);

    (TestServer::new(app), db)
}

const AUTH_HEADER: HeaderName = HeaderName::from_static("authorization");

fn auth_value(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("Bearer {token}")).unwrap()
}

async fn register_and_get_token(server: &TestServer, username: &str) -> (String, String) {
    let res = server
        .post("/api/v1/auth/register")
        .json(&json!({ "username": username, "password": "testpass123" }))
        .await;
    res.assert_status(axum::http::StatusCode::CREATED);
    let body: Value = res.json();
    let token = body["access_token"].as_str().unwrap().to_string();
    let user_id = body["user"]["id"].as_str().unwrap().to_string();
    (token, user_id)
}

// ═══════════════════════════════════════════════════════════════════════
// Session Lifecycle
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_full_session_lifecycle() {
    let (server, _db) = setup_test_app().await;
    let (token_host, _host_id) = register_and_get_token(&server, "host").await;
    let (token_user2, _user2_id) = register_and_get_token(&server, "user2").await;

    // 1. Create session
    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .json(&json!({ "ruleset": "random" }))
        .await;
    res.assert_status(axum::http::StatusCode::CREATED);
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap();
    assert_eq!(session["ruleset"], "random");
    assert_eq!(session["status"], "active");

    // 2. user2 joins
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);

    // 3. Verify session detail
    let res = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let detail: Value = res.json();
    let participants = detail["participants"].as_array().unwrap();
    let active_count = participants
        .iter()
        .filter(|p| p["left_at"].is_null())
        .count();
    assert_eq!(active_count, 2);

    // 4. Host leaves — host transfer to user2
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);

    let res = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    let detail: Value = res.json();
    assert_eq!(detail["host_id"], _user2_id);
    assert_eq!(detail["status"], "active");

    // 5. Last participant leaves — session closes
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);

    let res = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    let detail: Value = res.json();
    assert_eq!(detail["status"], "closed");
}

#[tokio::test]
async fn test_list_sessions_returns_only_active_sorted_by_last_activity() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "host").await;

    // Create two sessions
    let res1 = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let s1: Value = res1.json();
    let s1_id = s1["id"].as_str().unwrap();

    // Leave first session (closes it since solo)
    server
        .post(&format!("/api/v1/sessions/{s1_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;

    // Create second session (stays active)
    let res2 = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let s2: Value = res2.json();
    let s2_id = s2["id"].as_str().unwrap();

    // List — should only show the active one
    let res = server
        .get("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let list: Vec<Value> = res.json();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["id"], s2_id);
}

#[tokio::test]
async fn test_get_session_returns_participants_and_race_number() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "host").await;

    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap();

    let res = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let detail: Value = res.json();

    assert!(detail["participants"].is_array());
    assert_eq!(detail["participants"].as_array().unwrap().len(), 1);
    assert_eq!(detail["race_number"], 1);
    assert!(detail["host_username"].is_string());
}

#[tokio::test]
async fn test_join_closed_session_returns_409() {
    let (server, _db) = setup_test_app().await;
    let (token_host, _) = register_and_get_token(&server, "host").await;
    let (token_user2, _) = register_and_get_token(&server, "user2").await;

    // Create and close session
    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap();

    server
        .post(&format!("/api/v1/sessions/{session_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .await;

    // Try to join closed session
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_create_session_with_invalid_ruleset_returns_400() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "host").await;

    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "invalid_thing" }))
        .await;
    res.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_join_session_twice_returns_409() {
    let (server, _db) = setup_test_app().await;
    let (token_host, _) = register_and_get_token(&server, "host").await;
    let (token_user2, _) = register_and_get_token(&server, "user2").await;

    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap();

    // First join succeeds
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);

    // Second join returns 409
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_rejoin_after_leaving() {
    let (server, _db) = setup_test_app().await;
    let (token_host, _) = register_and_get_token(&server, "host").await;
    let (token_user2, _) = register_and_get_token(&server, "user2").await;

    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap();

    // Join, leave, rejoin
    server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    server
        .post(&format!("/api/v1/sessions/{session_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_sessions_require_auth() {
    let (server, _db) = setup_test_app().await;

    let res = server.get("/api/v1/sessions").await;
    res.assert_status(axum::http::StatusCode::UNAUTHORIZED);

    let res = server
        .post("/api/v1/sessions")
        .json(&json!({ "ruleset": "random" }))
        .await;
    res.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_get_nonexistent_session_returns_404() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "host").await;

    let res = server
        .get("/api/v1/sessions/nonexistent-id")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_stale_session_cleanup() {
    let (server, db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "host").await;

    // Create session
    let res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session: Value = res.json();
    let session_id = session["id"].as_str().unwrap().to_string();

    // Manually set last_activity_at to 2 hours ago
    let two_hours_ago = (chrono::Utc::now() - chrono::Duration::hours(2)).naive_utc();
    use beerio_kart::entities::sessions;
    use sea_orm::EntityTrait;
    let session_model = sessions::Entity::find_by_id(&session_id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    let mut active: sessions::ActiveModel = session_model.into();
    active.last_activity_at = Set(two_hours_ago);
    active.update(&db).await.unwrap();

    // Run cleanup
    let closed = beerio_kart::services::sessions::close_stale_sessions(&db)
        .await
        .unwrap();
    assert_eq!(closed, 1);

    // Verify session is closed
    let res = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    let detail: Value = res.json();
    assert_eq!(detail["status"], "closed");
}

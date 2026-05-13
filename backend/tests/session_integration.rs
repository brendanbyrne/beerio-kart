//! Integration tests for session lifecycle endpoints.

// Tests legitimately want to panic — per rust.md § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Arc;

use axum::{
    Router,
    http::header::{HeaderName, HeaderValue},
    routing::{get, post},
};
use axum_test::TestServer;
use beerio_kart::{
    ARGON2_MAX_CONCURRENT, AppState,
    config::Config,
    drink_type_id::drink_type_uuid,
    entities::{bodies, characters, cups, drink_types, gliders, sessions, tracks, wheels},
    routes,
};
use chrono::Utc;
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ConnectionTrait, Database, EntityTrait, Set};
use serde_json::{Value, json};
use tokio::sync::Semaphore;
use uuid::Uuid;

const TEST_SECRET: &str = "test-secret-for-session-tests";

fn test_config() -> Arc<Config> {
    Arc::new(Config {
        jwt_secret: TEST_SECRET.to_string(),
        jwt_access_expiry_minutes: 15,
        jwt_refresh_expiry_days: 7,
        admin_user_id: None,
        cookie_secure: false,
        request_timeout_seconds: 30,
        request_concurrency_limit: 100,
        max_request_body_bytes: 10 * 1024 * 1024,
        rate_limit_per_minute: 60,
    })
}

async fn setup_test_app() -> (TestServer, sea_orm::DatabaseConnection) {
    let url = format!("sqlite:file:{}?mode=memory&cache=shared", Uuid::new_v4());
    let db = Database::connect(&url)
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
        argon2_limit: Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT)),
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
        .route(
            "/api/v1/sessions/{id}/next-track",
            post(routes::sessions::next_track),
        )
        .route(
            "/api/v1/sessions/{id}/races/{race_id}/skip",
            post(routes::sessions::skip_pending_race),
        )
        // Runs
        .route("/api/v1/runs", post(routes::runs::create_run))
        .with_state(state);

    (TestServer::new(app), db)
}

/// Seed minimal game data needed by skip + `create_run` integration tests.
/// Production seed (`seed::run`) lives in main.rs and isn't reachable from
/// integration tests; this is a stripped-down equivalent — one each of
/// cup, track, character, body, wheels, glider, drink type.
async fn seed_minimal_game_data(db: &sea_orm::DatabaseConnection) {
    cups::ActiveModel {
        id: Set(1),
        name: Set("Test Cup".to_string()),
        image_path: Set("images/cups/test.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert cup");

    tracks::ActiveModel {
        id: Set(1),
        name: Set("Test Track".to_string()),
        cup_id: Set(1),
        position: Set(1),
        image_path: Set("images/tracks/test.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert track");

    characters::ActiveModel {
        id: Set(1),
        name: Set("Mario".to_string()),
        image_path: Set("images/characters/mario.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert character");

    bodies::ActiveModel {
        id: Set(1),
        name: Set("Standard Kart".to_string()),
        image_path: Set("images/bodies/standard.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert body");

    wheels::ActiveModel {
        id: Set(1),
        name: Set("Standard".to_string()),
        image_path: Set("images/wheels/standard.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert wheels");

    gliders::ActiveModel {
        id: Set(1),
        name: Set("Super Glider".to_string()),
        image_path: Set("images/gliders/super.webp".to_string()),
    }
    .insert(db)
    .await
    .expect("insert glider");

    drink_types::ActiveModel {
        id: Set(drink_type_uuid("Test Beer").into()),
        name: Set("Test Beer".to_string()),
        alcoholic: Set(true),
        created_at: Set(Utc::now().naive_utc()),
        created_by: Set(None),
    }
    .insert(db)
    .await
    .expect("insert drink type");
}

/// Build a valid `CreateRunRequest` body for the given race ID.
fn run_request_json(session_race_id: &str) -> Value {
    let drink_id = drink_type_uuid("Test Beer");
    json!({
        "session_race_id": session_race_id,
        "track_time": 120_000,
        "lap1_time": 40_000,
        "lap2_time": 39_000,
        "lap3_time": 41_000,
        "character_id": 1,
        "body_id": 1,
        "wheel_id": 1,
        "glider_id": 1,
        "drink_type_id": drink_id,
        "disqualified": false,
    })
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
    let (token_user2, user2_id) = register_and_get_token(&server, "user2").await;

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
    assert_eq!(detail["host_id"], user2_id);
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

    // Valid UUID shape but no matching row. A non-UUID path segment now
    // produces a 400 from axum's Path extractor before reaching the handler.
    let missing = uuid::Uuid::new_v4();
    let res = server
        .get(&format!("/api/v1/sessions/{missing}"))
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

// ═══════════════════════════════════════════════════════════════════════
// Pending Races: skip endpoint + ordered-submit (PR 3D-2)
// ═══════════════════════════════════════════════════════════════════════

/// Helper: register a host, seed minimal game data, create a session, pick a
/// track. Returns (server, db, token, `user_id`, `session_id`, `race_id`).
async fn setup_with_one_race() -> (
    TestServer,
    sea_orm::DatabaseConnection,
    String,
    String,
    String,
    String,
) {
    let (server, db) = setup_test_app().await;
    seed_minimal_game_data(&db).await;
    let (token, user_id) = register_and_get_token(&server, "host").await;
    let session_res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session_id = session_res.json::<Value>()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let race_res = server
        .post(&format!("/api/v1/sessions/{session_id}/next-track"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    race_res.assert_status(axum::http::StatusCode::CREATED);
    let race_id = race_res.json::<Value>()["id"].as_str().unwrap().to_string();
    (server, db, token, user_id, session_id, race_id)
}

#[tokio::test]
async fn test_skip_endpoint_happy_path_returns_204_and_clears_pending() {
    let (server, _db, token, _user_id, session_id, race_id) = setup_with_one_race().await;

    // Pre-condition: pending list contains the race.
    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    assert_eq!(detail["your_pending"].as_array().unwrap().len(), 1);

    let res = server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NO_CONTENT);

    // Post-condition: pending list is empty (skipped race drops out).
    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    assert!(
        detail["your_pending"].as_array().unwrap().is_empty(),
        "skipped race must not appear in your_pending"
    );
}

#[tokio::test]
async fn test_skip_endpoint_idempotent() {
    let (server, _db, token, _user_id, session_id, race_id) = setup_with_one_race().await;

    let url = format!("/api/v1/sessions/{session_id}/races/{race_id}/skip");
    server
        .post(&url)
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);
    server
        .post(&url)
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_skip_endpoint_unknown_race_returns_404() {
    let (server, _db, token, _user_id, session_id, _race_id) = setup_with_one_race().await;
    let bogus = Uuid::new_v4().to_string();
    let res = server
        .post(&format!("/api/v1/sessions/{session_id}/races/{bogus}/skip"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_skip_endpoint_already_submitted_returns_409() {
    let (server, _db, token, _user_id, session_id, race_id) = setup_with_one_race().await;

    // Submit a run first.
    server
        .post("/api/v1/runs")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&run_request_json(&race_id))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // Now try to skip — must 409.
    let res = server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_create_run_blocked_by_older_pending_returns_409_with_message() {
    let (server, _db, token, _user_id, session_id, race1_id) = setup_with_one_race().await;
    // Add a second race so race1 becomes "older pending" relative to race2.
    let race2_res = server
        .post(&format!("/api/v1/sessions/{session_id}/next-track"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    let race2_id = race2_res.json::<Value>()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Try to submit race 2 while race 1 is pending → 409.
    let res = server
        .post("/api/v1/runs")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&run_request_json(&race2_id))
        .await;
    res.assert_status(axum::http::StatusCode::CONFLICT);

    let body: Value = res.json();
    let msg = body["error"].as_str().unwrap_or("");
    assert!(
        msg.contains("Must submit or skip pending race #1"),
        "expected race-#1 conflict message, got: {msg}"
    );
    let _ = race1_id; // silence unused-binding warning
}

#[tokio::test]
async fn test_create_run_succeeds_after_skipping_older_pending() {
    let (server, _db, token, _user_id, session_id, race1_id) = setup_with_one_race().await;
    let race2_res = server
        .post(&format!("/api/v1/sessions/{session_id}/next-track"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    let race2_id = race2_res.json::<Value>()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Skip race 1, then submit race 2 — should succeed.
    server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race1_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    server
        .post("/api/v1/runs")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&run_request_json(&race2_id))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    // your_pending should now be empty.
    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    assert!(
        detail["your_pending"].as_array().unwrap().is_empty(),
        "after skip + submit, your_pending should be empty"
    );
}

#[tokio::test]
async fn test_session_detail_your_pending_reflects_skip_and_submit() {
    let (server, _db, token, _user_id, session_id, race1_id) = setup_with_one_race().await;
    let race2_res = server
        .post(&format!("/api/v1/sessions/{session_id}/next-track"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    let race2_id = race2_res.json::<Value>()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Both races pending.
    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    assert_eq!(detail["your_pending"].as_array().unwrap().len(), 2);

    // Skip race 1 → only race 2 left.
    server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race1_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    let pending = detail["your_pending"].as_array().unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0]["id"].as_str().unwrap(), race2_id);

    // Submit race 2 → no pending left.
    server
        .post("/api/v1/runs")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&run_request_json(&race2_id))
        .await
        .assert_status(axum::http::StatusCode::CREATED);

    let detail: Value = server
        .get(&format!("/api/v1/sessions/{session_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .json();
    assert!(detail["your_pending"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_create_run_after_skip_returns_409() {
    // Regression for the submit-after-skip bypass — at the HTTP layer.
    let (server, _db, token, _user_id, session_id, race_id) = setup_with_one_race().await;

    server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    let res = server
        .post("/api/v1/runs")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&run_request_json(&race_id))
        .await;
    res.assert_status(axum::http::StatusCode::CONFLICT);

    let body: Value = res.json();
    let msg = body["error"].as_str().unwrap_or("");
    assert!(
        msg.contains("skipped"),
        "expected message about skipped race, got: {msg}"
    );
}

#[tokio::test]
async fn test_skip_after_leaving_returns_403() {
    // Regression for the symmetry gap with create_run — a user who has
    // left the session must not be able to skip races in it. Two-user
    // session so the session stays active when user2 leaves.
    let (server, _db, token_host, _host_id) = {
        let (server, db) = setup_test_app().await;
        seed_minimal_game_data(&db).await;
        let (token, user_id) = register_and_get_token(&server, "host").await;
        (server, db, token, user_id)
    };
    let (token_user2, _user2_id) = register_and_get_token(&server, "user2").await;

    let session_res = server
        .post("/api/v1/sessions")
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .json(&json!({ "ruleset": "random" }))
        .await;
    let session_id = session_res.json::<Value>()["id"]
        .as_str()
        .unwrap()
        .to_string();
    server
        .post(&format!("/api/v1/sessions/{session_id}/join"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    let race_res = server
        .post(&format!("/api/v1/sessions/{session_id}/next-track"))
        .add_header(AUTH_HEADER, auth_value(&token_host))
        .await;
    let race_id = race_res.json::<Value>()["id"].as_str().unwrap().to_string();

    server
        .post(&format!("/api/v1/sessions/{session_id}/leave"))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await
        .assert_status(axum::http::StatusCode::NO_CONTENT);

    // user2 has left — skipping should now be Forbidden, not allowed.
    let res = server
        .post(&format!(
            "/api/v1/sessions/{session_id}/races/{race_id}/skip"
        ))
        .add_header(AUTH_HEADER, auth_value(&token_user2))
        .await;
    res.assert_status(axum::http::StatusCode::FORBIDDEN);
}

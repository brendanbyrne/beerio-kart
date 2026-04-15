//! Integration tests for game data, user profile, and drink type endpoints.

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

/// Create a fresh in-memory SQLite database with all migrations applied and
/// static data seeded. Returns the test server and the underlying DB connection
/// for direct queries in tests.
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

    // Seed static game data
    seed_test_data(&db).await;

    let config = test_config();
    let state = AppState {
        db: db.clone(),
        config,
    };

    let app = Router::new()
        // Auth
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        // Game data
        .route(
            "/api/v1/characters",
            get(routes::game_data::list_characters),
        )
        .route("/api/v1/bodies", get(routes::game_data::list_bodies))
        .route("/api/v1/wheels", get(routes::game_data::list_wheels))
        .route("/api/v1/gliders", get(routes::game_data::list_gliders))
        .route("/api/v1/cups", get(routes::game_data::list_cups))
        .route("/api/v1/cups/{id}", get(routes::game_data::get_cup))
        .route("/api/v1/tracks", get(routes::game_data::list_tracks))
        .route("/api/v1/tracks/{id}", get(routes::game_data::get_track))
        // Users
        .route("/api/v1/users", get(routes::users::list_users))
        .route(
            "/api/v1/users/{id}",
            get(routes::users::get_user).put(routes::users::update_user),
        )
        // Drink types
        .route(
            "/api/v1/drink-types",
            get(routes::drink_types::list_drink_types).post(routes::drink_types::create_drink_type),
        )
        .route(
            "/api/v1/drink-types/{id}",
            get(routes::drink_types::get_drink_type),
        )
        .with_state(state);

    (TestServer::new(app), db)
}

/// Seed minimal static data for tests. Uses the same JSON files as production
/// seed but loads them directly.
async fn seed_test_data(db: &sea_orm::DatabaseConnection) {
    use beerio_kart::drink_type_id::drink_type_uuid;
    use beerio_kart::entities::{bodies, characters, cups, drink_types, gliders, tracks, wheels};
    use sea_orm::TransactionTrait;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct SeedItem {
        id: i32,
        name: String,
        image_path: String,
    }

    #[derive(Deserialize)]
    struct SeedTrack {
        id: i32,
        name: String,
        cup_id: i32,
        position: i32,
        image_path: String,
    }

    #[derive(Deserialize)]
    struct SeedDrinkType {
        name: String,
        alcoholic: bool,
    }

    // Seed cups
    let cups_json: Vec<SeedItem> =
        serde_json::from_str(include_str!("../../data/cups.json")).expect("cups.json");
    let txn = db.begin().await.expect("begin");
    for item in &cups_json {
        cups::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert cup");
    }
    txn.commit().await.expect("commit cups");

    // Seed characters
    let chars_json: Vec<SeedItem> =
        serde_json::from_str(include_str!("../../data/characters.json")).expect("characters.json");
    let txn = db.begin().await.expect("begin");
    for item in &chars_json {
        characters::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert character");
    }
    txn.commit().await.expect("commit characters");

    // Seed bodies
    let bodies_json: Vec<SeedItem> =
        serde_json::from_str(include_str!("../../data/bodies.json")).expect("bodies.json");
    let txn = db.begin().await.expect("begin");
    for item in &bodies_json {
        bodies::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert body");
    }
    txn.commit().await.expect("commit bodies");

    // Seed wheels
    let wheels_json: Vec<SeedItem> =
        serde_json::from_str(include_str!("../../data/wheels.json")).expect("wheels.json");
    let txn = db.begin().await.expect("begin");
    for item in &wheels_json {
        wheels::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert wheel");
    }
    txn.commit().await.expect("commit wheels");

    // Seed gliders
    let gliders_json: Vec<SeedItem> =
        serde_json::from_str(include_str!("../../data/gliders.json")).expect("gliders.json");
    let txn = db.begin().await.expect("begin");
    for item in &gliders_json {
        gliders::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert glider");
    }
    txn.commit().await.expect("commit gliders");

    // Seed tracks
    let tracks_json: Vec<SeedTrack> =
        serde_json::from_str(include_str!("../../data/tracks.json")).expect("tracks.json");
    let txn = db.begin().await.expect("begin");
    for item in &tracks_json {
        tracks::ActiveModel {
            id: Set(item.id),
            name: Set(item.name.clone()),
            cup_id: Set(item.cup_id),
            position: Set(item.position),
            image_path: Set(item.image_path.clone()),
        }
        .insert(&txn)
        .await
        .expect("insert track");
    }
    txn.commit().await.expect("commit tracks");

    // Seed drink types
    let dt_json: Vec<SeedDrinkType> =
        serde_json::from_str(include_str!("../../data/drink_types.json"))
            .expect("drink_types.json");
    let now = chrono::Utc::now().naive_utc();
    let txn = db.begin().await.expect("begin");
    for item in &dt_json {
        let id = drink_type_uuid(&item.name);
        drink_types::ActiveModel {
            id: Set(id),
            name: Set(item.name.clone()),
            alcoholic: Set(item.alcoholic),
            created_at: Set(now),
            created_by: Set(None),
        }
        .insert(&txn)
        .await
        .expect("insert drink type");
    }
    txn.commit().await.expect("commit drink_types");
}

const AUTH_HEADER: HeaderName = HeaderName::from_static("authorization");

fn auth_value(token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!("Bearer {token}")).unwrap()
}

/// Register a user and return (access_token, user_id).
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
// Game Data Endpoints
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_characters_returns_50_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/characters")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 50, "Expected 50 characters");
}

#[tokio::test]
async fn test_list_bodies_returns_41_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/bodies")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 41, "Expected 41 bodies");
}

#[tokio::test]
async fn test_list_wheels_returns_22_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/wheels")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 22, "Expected 22 wheels");
}

#[tokio::test]
async fn test_list_gliders_returns_15_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/gliders")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 15, "Expected 15 gliders");
}

#[tokio::test]
async fn test_list_cups_returns_24_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/cups")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 24, "Expected 24 cups");
}

#[tokio::test]
async fn test_list_tracks_returns_96_items() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/tracks")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 96, "Expected 96 tracks");
}

#[tokio::test]
async fn test_list_tracks_filtered_by_cup_returns_4_tracks() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/tracks?cup_id=1")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 4, "Each cup should have exactly 4 tracks");
}

#[tokio::test]
async fn test_get_cup_with_tracks() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/cups/1")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert!(body["name"].is_string());
    let tracks = body["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 4, "Cup should include its 4 tracks");
}

#[tokio::test]
async fn test_get_nonexistent_cup_returns_404() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/cups/999")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_get_track_by_id() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/tracks/1")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["id"], 1);
    assert!(body["name"].is_string());
    assert!(body["cup_id"].is_number());
}

#[tokio::test]
async fn test_game_data_requires_auth() {
    let (server, _db) = setup_test_app().await;

    // All game data endpoints should return 401 without a token
    let endpoints = [
        "/api/v1/characters",
        "/api/v1/bodies",
        "/api/v1/wheels",
        "/api/v1/gliders",
        "/api/v1/cups",
        "/api/v1/tracks",
    ];

    for endpoint in endpoints {
        let res = server.get(endpoint).await;
        res.assert_status(axum::http::StatusCode::UNAUTHORIZED);
    }
}

// ═══════════════════════════════════════════════════════════════════════
// User Profile Endpoints
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_users_returns_registered_users() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "alice").await;
    register_and_get_token(&server, "bob").await;

    let res = server
        .get("/api/v1/users")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 2);

    // Should not expose password_hash or refresh_token_version
    for user in &body {
        assert!(user.get("password_hash").is_none());
        assert!(user.get("refresh_token_version").is_none());
    }
}

#[tokio::test]
async fn test_get_user_profile() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "alice").await;

    let res = server
        .get(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["username"], "alice");
    assert!(body["preferred_drink_type"].is_null());
}

#[tokio::test]
async fn test_get_nonexistent_user_returns_404() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "alice").await;

    let res = server
        .get("/api/v1/users/nonexistent-id")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_update_user_race_setup_all_or_nothing() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "alice").await;

    // Providing only some race setup fields should fail
    let res = server
        .put(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({
            "preferred_character_id": 1,
            "preferred_body_id": 1,
        }))
        .await;
    res.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_user_race_setup_success() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "alice").await;

    let res = server
        .put(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({
            "preferred_character_id": 1,
            "preferred_body_id": 1,
            "preferred_wheel_id": 1,
            "preferred_glider_id": 1,
        }))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["preferred_character_id"], 1);
    assert_eq!(body["preferred_body_id"], 1);
    assert_eq!(body["preferred_wheel_id"], 1);
    assert_eq!(body["preferred_glider_id"], 1);
}

#[tokio::test]
async fn test_update_user_invalid_fk_returns_400() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "alice").await;

    let res = server
        .put(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({
            "preferred_character_id": 999,
            "preferred_body_id": 1,
            "preferred_wheel_id": 1,
            "preferred_glider_id": 1,
        }))
        .await;
    res.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_update_other_users_profile_returns_403() {
    let (server, _db) = setup_test_app().await;
    let (token_alice, _) = register_and_get_token(&server, "alice").await;
    let (_, bob_id) = register_and_get_token(&server, "bob").await;

    let res = server
        .put(&format!("/api/v1/users/{bob_id}"))
        .add_header(AUTH_HEADER, auth_value(&token_alice))
        .json(&json!({
            "preferred_character_id": 1,
            "preferred_body_id": 1,
            "preferred_wheel_id": 1,
            "preferred_glider_id": 1,
        }))
        .await;
    res.assert_status(axum::http::StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_update_user_preferred_drink_type() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "alice").await;

    // First create a drink type
    let dt_res = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "Test Beer", "alcoholic": true }))
        .await;
    let dt: Value = dt_res.json();
    let dt_id = dt["id"].as_str().unwrap();

    // Set it as preferred
    let res = server
        .put(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "preferred_drink_type_id": dt_id }))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["preferred_drink_type"]["id"], dt_id);
    assert_eq!(body["preferred_drink_type"]["name"], "Test Beer");

    // Clear it
    let res = server
        .put(&format!("/api/v1/users/{user_id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "preferred_drink_type_id": null }))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert!(body["preferred_drink_type"].is_null());
}

// ═══════════════════════════════════════════════════════════════════════
// Drink Type Endpoints
// ═══════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn test_list_drink_types_returns_pre_seeded() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 2, "Should have 2 pre-seeded drink types");
}

#[tokio::test]
async fn test_create_drink_type() {
    let (server, _db) = setup_test_app().await;
    let (token, user_id) = register_and_get_token(&server, "testuser").await;

    let res = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "Molson Canadian", "alcoholic": true }))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["name"], "Molson Canadian");
    assert_eq!(body["alcoholic"], true);
    assert_eq!(body["created_by"], user_id);
}

#[tokio::test]
async fn test_create_drink_type_deduplicates_case_insensitively() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    // Create one
    let res1 = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "Molson Canadian", "alcoholic": true }))
        .await;
    let body1: Value = res1.json();

    // Try to create with different casing — should return the existing one
    let res2 = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "MOLSON CANADIAN", "alcoholic": true }))
        .await;
    res2.assert_status(axum::http::StatusCode::OK);
    let body2: Value = res2.json();

    assert_eq!(
        body1["id"], body2["id"],
        "Same UUID for same name (case-insensitive)"
    );
    assert_eq!(
        body2["name"], "Molson Canadian",
        "Preserves original casing"
    );
}

#[tokio::test]
async fn test_create_drink_type_empty_name_returns_400() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "  ", "alcoholic": false }))
        .await;
    res.assert_status(axum::http::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_get_drink_type_by_id() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    // Create one first
    let create_res = server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "Test IPA", "alcoholic": true }))
        .await;
    let created: Value = create_res.json();
    let id = created["id"].as_str().unwrap();

    // Fetch by ID
    let res = server
        .get(&format!("/api/v1/drink-types/{id}"))
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Value = res.json();
    assert_eq!(body["name"], "Test IPA");
}

#[tokio::test]
async fn test_get_nonexistent_drink_type_returns_404() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    let res = server
        .get("/api/v1/drink-types/nonexistent-uuid")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_filter_drink_types_by_alcoholic() {
    let (server, _db) = setup_test_app().await;
    let (token, _) = register_and_get_token(&server, "testuser").await;

    // Add a non-alcoholic drink
    server
        .post("/api/v1/drink-types")
        .add_header(AUTH_HEADER, auth_value(&token))
        .json(&json!({ "name": "LaCroix", "alcoholic": false }))
        .await;

    // Filter alcoholic=true (pre-seeded: Labatt Blue + Modelo = 2)
    let res = server
        .get("/api/v1/drink-types?alcoholic=true")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 2, "Should have 2 alcoholic drink types");

    // Filter alcoholic=false
    let res = server
        .get("/api/v1/drink-types?alcoholic=false")
        .add_header(AUTH_HEADER, auth_value(&token))
        .await;
    res.assert_status(axum::http::StatusCode::OK);
    let body: Vec<Value> = res.json();
    assert_eq!(body.len(), 1, "Should have 1 non-alcoholic drink type");
    assert_eq!(body[0]["name"], "LaCroix");
}

#[tokio::test]
async fn test_drink_types_require_auth() {
    let (server, _db) = setup_test_app().await;

    let res = server.get("/api/v1/drink-types").await;
    res.assert_status(axum::http::StatusCode::UNAUTHORIZED);

    let res = server
        .post("/api/v1/drink-types")
        .json(&json!({ "name": "test", "alcoholic": true }))
        .await;
    res.assert_status(axum::http::StatusCode::UNAUTHORIZED);
}

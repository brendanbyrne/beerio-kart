mod seed;

use std::time::Duration;

use axum::{
    Json, Router,
    routing::{get, post, put},
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde::Serialize;
use tower_http::services::{ServeDir, ServeFile};
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::routes;
use beerio_kart::services;

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

#[tokio::main]
async fn main() {
    // Load .env file if present (non-fatal if missing)
    dotenvy::dotenv().ok();

    // Initialize structured logging. Defaults to `info` level; override with
    // the RUST_LOG env var (e.g., RUST_LOG=debug or RUST_LOG=beerio_kart=debug).
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,sqlx=warn,sea_orm_migration=warn")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:../data/db/beerio-kart.db?mode=rwc".to_string());

    // Load config from env vars (panics if JWT_SECRET is missing)
    let config = AppConfig::from_env();

    // Connect to the database. The ?mode=rwc flag creates the file if it
    // doesn't exist yet. SeaORM wraps sqlx under the hood.
    let db = Database::connect(&database_url)
        .await
        .expect("Failed to connect to database");

    // SQLite doesn't enforce foreign keys by default — this PRAGMA enables it
    // per connection. Without it, you can insert rows referencing nonexistent
    // foreign keys and SQLite will silently accept them.
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("Failed to enable foreign key enforcement");

    // Run all pending migrations. On a fresh database this creates every table.
    Migrator::up(&db, None)
        .await
        .expect("Failed to run database migrations");

    // Seed static game data (characters, tracks, cups, etc.) from JSON files.
    // Only inserts into empty tables — safe to call on every startup.
    tracing::info!("Seeding static data...");
    seed::run(&db).await.expect("Failed to seed database");
    tracing::info!("Seeding complete");

    let state = AppState { db, config };

    // Clone the DB connection for the background cleanup task before `state`
    // is moved into the router.
    let cleanup_db = state.db.clone();

    // STATIC_DIR defaults to ../frontend/dist for local dev (running from backend/).
    // In Docker, set to /app/static where the built frontend is copied.
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".to_string());

    let app = Router::new()
        .route("/api/v1/hello", get(hello))
        // Auth
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/refresh", post(routes::auth::refresh))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .route("/api/v1/auth/password", put(routes::auth::change_password))
        // Game data (pre-seeded, read-only)
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
        // Sessions
        .route(
            "/api/v1/sessions",
            get(routes::sessions::list_sessions).post(routes::sessions::create_session),
        )
        .route("/api/v1/sessions/mine", get(routes::sessions::my_session))
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
            "/api/v1/sessions/{id}/skip-turn",
            post(routes::sessions::skip_turn),
        )
        .route(
            "/api/v1/sessions/{id}/races",
            get(routes::sessions::list_races),
        )
        // Runs — /defaults before /{id} so literal matches first
        .route(
            "/api/v1/runs",
            get(routes::runs::list_runs).post(routes::runs::create_run),
        )
        .route("/api/v1/runs/defaults", get(routes::runs::get_defaults))
        .route(
            "/api/v1/runs/{id}",
            get(routes::runs::get_run).delete(routes::runs::delete_run),
        )
        .layer(TraceLayer::new_for_http())
        .with_state(state)
        // Serve frontend static files. If no API route or static file matches,
        // fall back to index.html so React Router can handle client-side routing.
        // Using .fallback() instead of .not_found_service() returns 200 (not 404).
        .fallback_service(
            ServeDir::new(&static_dir)
                .fallback(ServeFile::new(format!("{}/index.html", static_dir))),
        );

    // Spawn background task to close stale sessions (no activity for 1 hour).
    // Runs every 5 minutes.
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(300)).await;
            match services::sessions::close_stale_sessions(&cleanup_db).await {
                Ok(0) => {}
                Ok(n) => tracing::info!("Closed {n} stale session(s)"),
                Err(_) => tracing::error!("Stale session cleanup failed"),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Beerio Kart!".to_string(),
    })
}

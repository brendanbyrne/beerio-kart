mod seed;

use axum::{
    Json, Router,
    routing::{get, post},
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database};
use serde::Serialize;
use tower_http::trace::TraceLayer;
use tracing_subscriber::EnvFilter;

use beerio_kart::AppState;
use beerio_kart::config::AppConfig;
use beerio_kart::routes;

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
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:../data/beerio-kart.db?mode=rwc".to_string());

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

    let app = Router::new()
        .route("/api/v1/hello", get(hello))
        .route("/api/v1/auth/register", post(routes::auth::register))
        .route("/api/v1/auth/login", post(routes::auth::login))
        .route("/api/v1/auth/logout", post(routes::auth::logout))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    tracing::info!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Beerio Kart!".to_string(),
    })
}

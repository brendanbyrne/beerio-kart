#[allow(unused)]
mod entities;

use axum::{Json, Router, routing::get};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
use serde::Serialize;

#[derive(Serialize)]
struct HelloResponse {
    message: String,
}

#[tokio::main]
async fn main() {
    // Load .env file if present (non-fatal if missing)
    dotenvy::dotenv().ok();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:../data/beerio-kart.db?mode=rwc".to_string());

    // Connect to the database. The ?mode=rwc flag creates the file if it
    // doesn't exist yet. SeaORM wraps sqlx under the hood.
    let db: DatabaseConnection = Database::connect(&database_url)
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

    let app = Router::new()
        .route("/api/v1/hello", get(hello))
        .with_state(db);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Listening on http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}

async fn hello() -> Json<HelloResponse> {
    Json(HelloResponse {
        message: "Hello from Beerio Kart!".to_string(),
    })
}

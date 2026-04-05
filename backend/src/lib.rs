pub mod config;
pub mod drink_type_id;
#[allow(unused_imports)]
pub mod entities;
pub mod error;
pub mod middleware;
pub mod routes;
pub mod services;

use std::sync::Arc;

use config::AppConfig;
use sea_orm::DatabaseConnection;

/// Shared application state available to all Axum handlers via `State<AppState>`.
///
/// `AppState` is cheaply cloneable — `DatabaseConnection` and `Arc<AppConfig>`
/// are both reference-counted internally.
#[derive(Clone)]
pub struct AppState {
    pub db: DatabaseConnection,
    pub config: Arc<AppConfig>,
}

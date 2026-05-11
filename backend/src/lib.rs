//! Beerio Kart backend — Axum HTTP API on top of `SeaORM` + `SQLite`.
//!
//! The composition root and shared application state live here; the
//! per-area modules ([`routes`], [`services`], [`entities`], [`middleware`])
//! are wired together by `main.rs`. See [`AppState`] for the handler-visible
//! state and [`ARGON2_MAX_CONCURRENT`] for the password-hash concurrency cap.
//!
//! Architecture, naming, and error conventions live in `docs/design.md` and
//! `docs/coding-standards/`; backend-specific conventions (schema-changes
//! prelaunch, testing, ORM usage) live in `backend/CLAUDE.md`.

// Tests legitimately want to panic — per rust.md § 8.
#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;
pub mod db;
pub mod domain;
pub mod drink_type_id;
#[allow(unused_imports)]
pub mod entities;
pub mod error;
pub mod middleware;
pub mod routes;
pub mod services;

#[cfg(test)]
pub mod test_helpers;

use std::sync::Arc;

use config::Config;
use sea_orm::DatabaseConnection;
use tokio::sync::Semaphore;

/// Default cap on concurrent Argon2 hashes/verifies in flight at once.
///
/// Argon2id with default cost takes ~50–200 ms of CPU + memory. Without a
/// bound, a login storm can saturate Tokio's blocking pool (default 512
/// threads) and starve unrelated `spawn_blocking` work. 16 is a balance
/// between absorbing a burst and leaving CPU/memory headroom for everything
/// else — see `coding-standards/tokio.md` § 12.
pub const ARGON2_MAX_CONCURRENT: usize = 16;

/// Shared application state available to all Axum handlers via `State<AppState>`.
///
/// `AppState` is cheaply cloneable — `DatabaseConnection`, `Arc<Config>`,
/// and `Arc<Semaphore>` are all reference-counted internally.
#[derive(Clone)]
pub struct AppState {
    /// `SeaORM` connection pool. Cheaply cloneable — internally an `Arc`.
    pub db: DatabaseConnection,
    /// Loaded configuration (JWT secret, cookie flags, token expiries).
    /// Wrapped in `Arc` so clones share one allocation.
    pub config: Arc<Config>,
    /// Caps concurrent Argon2 hash/verify operations across all handlers.
    /// See `services::auth::{hash_password, verify_password}`.
    pub argon2_limit: Arc<Semaphore>,
}

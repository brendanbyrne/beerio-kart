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
// A test named for a specific failure must pin that failure, not just "it
// errored": `assert!(r.is_ok())` / `assert!(r.is_err())` discard both the value
// and the error, so a wrong-but-Ok value or a failure for the wrong reason
// passes. Forbid them in favour of `.unwrap()` / `.unwrap_err()` (or matching
// the specific `Error` variant / `code`). Opt-in (clippy `restriction` group),
// so it must be declared here. See `docs/coding-standards/testing.md`. (#217)
#![warn(clippy::assertions_on_result_states)]

/// Loaded server configuration (JWT secrets, cookie flags, request limits).
pub mod config;
/// SeaORM connection-pool setup with per-connection SQLite PRAGMAs.
pub mod db;
/// Domain types — IDs, validated strings, numeric newtypes, enums.
pub mod domain;
/// Newtype wrapper for drink-type identifiers (currently a plain string).
pub mod drink_type_id;
// Hand-written SeaORM entities (ADR 0023): every `pub` item in the
// submodules is a column declaration or relation marker whose name IS
// the documentation. `rust.md` § 6 doesn't earn its keep here; opt out
// at the module boundary so we don't need a `#![allow]` in each file.
#[allow(unused_imports, missing_docs)]
pub mod entities;
/// Unified application error type implementing [`axum::response::IntoResponse`].
pub mod error;
/// Project-local request extractors (typed Path/Json) emitting the standard
/// [error envelope](error::Error) on rejection.
pub mod extract;
/// Axum middleware — JWT-based auth extractor and request-limit handlers.
pub mod middleware;
/// HTTP route handlers grouped by resource.
pub mod routes;
/// Business-logic service layer — orchestrates entities, enforces rules.
pub mod services;
/// Graceful-shutdown wiring: signal handling, task supervision, and drain.
pub mod shutdown;
/// Per-call timeout helpers wrapping `SeaORM` futures with `Error::Timeout`.
pub mod timeout;

/// Test fixtures and helpers shared across integration tests.
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

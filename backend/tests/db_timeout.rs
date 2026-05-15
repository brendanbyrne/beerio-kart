//! Verification test for PR-F4 — per-call DB timeouts under `SQLite` contention.
//!
//! Forces a held write lock from one pool and confirms that a writer on a
//! second pool, wrapped in `timeout::db_query`, returns `Error::Timeout`
//! before `SQLite`'s pool-level `busy_timeout` (5 s in `db::connect`) would.
//! This is the behavioural test for `tokio.md` § 12's "stuck call can't
//! pin a worker forever" claim.
//!
//! Lives in `tests/` rather than as a `#[cfg(test)] mod tests` block in
//! `timeout.rs` because it needs real file-system contention against a
//! migrated database — not a synthetic future. In-memory `cache=shared`
//! databases don't reproduce the file-lock serialization that exposes the
//! contention.

// Tests legitimately want to panic — per rust.md § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::{Duration, Instant};

use axum::response::IntoResponse;
use beerio_kart::{
    db,
    entities::users,
    error::{Error, ErrorCode},
    timeout::{QUERY_BUDGET, db_query},
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, Set, TransactionTrait};
use uuid::Uuid;

#[tokio::test]
async fn test_db_query_returns_timeout_when_blocked_on_sqlite_write_lock() {
    // Temp file path. A real file is required: in-memory `cache=shared` DBs
    // don't serialize writes via a file lock the way SQLite-on-disk does, so
    // the contention this test exists to verify wouldn't actually occur.
    let path = std::env::temp_dir().join(format!("beerio-kart-pr-f4-{}.db", Uuid::new_v4()));
    let url = format!("sqlite:{}?mode=rwc", path.display());

    let db_holder = db::connect(&url).await.expect("connect holder pool");
    Migrator::up(&db_holder, None).await.expect("migrate");

    // Two pools, same DB file. We need separate pools (not two checkouts
    // from the same pool) so the holder's txn doesn't starve the contender
    // at the pool layer — the wait we're testing is at the SQLite file lock,
    // not at the connection-pool acquire.
    let db_contender = db::connect(&url).await.expect("connect contender pool");

    // Take the file's write lock by beginning a txn and issuing a write that
    // won't be committed/rolled back until after the contender's timeout
    // fires. SQLite acquires the write lock on the first write, not on
    // BEGIN, so the INSERT below is what actually contends.
    let txn = db_holder.begin().await.expect("begin holder txn");
    db_query(make_user("holder").insert(&txn))
        .await
        .expect("holder insert");

    // Second writer. SQLite serializes writers — this must wait for `txn`'s
    // commit or rollback. Wrapped in `db_query`, the wait should terminate
    // at `QUERY_BUDGET` (2 s) rather than the pool's `busy_timeout` (5 s).
    let start = Instant::now();
    let result: Result<_, Error> = db_query(make_user("contender").insert(&db_contender)).await;
    let elapsed = start.elapsed();

    // Release the held lock so background completion of the contender's
    // blocking task (sqlx's `spawn_blocking` runs to completion regardless
    // of whether our wrapper future was dropped) can drain cleanly before
    // we tear down the pools and remove the file.
    let _ = txn.rollback().await;
    drop(db_holder);
    drop(db_contender);
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_file(path.with_extension("db-shm"));
    let _ = std::fs::remove_file(path.with_extension("db-wal"));

    // Variant + budget: a stuck DB call must surface as `Error::Timeout`
    // (not `Error::Internal` from a SQLITE_BUSY mapped through `DbErr`),
    // and the budget carried in the variant must be the helper's constant.
    // `code()` must return `GatewayTimeout` so the wire envelope's `code`
    // field is correct — IntoResponse below sanity-checks the serialization.
    let err = match result {
        Err(Error::Timeout { budget }) => {
            assert_eq!(budget, QUERY_BUDGET);
            Error::Timeout { budget }
        }
        other => panic!("expected Error::Timeout, got {other:?}"),
    };
    assert_eq!(err.code(), ErrorCode::GatewayTimeout);

    // Wire-shape sanity: the response body emits `code: "gateway_timeout"`
    // alongside the user-facing message. Guards against a regression where
    // the variant is right but the IntoResponse arm drops the code field.
    let response = err.into_response();
    assert_eq!(response.status(), axum::http::StatusCode::GATEWAY_TIMEOUT);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("collect body");
    let body: serde_json::Value = serde_json::from_slice(&bytes).expect("body is JSON");
    assert_eq!(body["code"], "gateway_timeout");
    assert_eq!(body["error"], "Request timed out");

    // Wall-clock check: the timeout fired close to the budget, not past
    // the pool's 5 s busy_timeout. The variant check above is the
    // structural claim; this is the regression guard. The +1500 ms
    // tolerance leaves room for scheduler jitter on WSL2 / CI runners
    // without admitting a regression that lets writes block past 3.5 s.
    assert!(
        elapsed < QUERY_BUDGET + Duration::from_millis(1500),
        "elapsed {elapsed:?} exceeded budget {QUERY_BUDGET:?} by too much"
    );
}

/// Build a unique `users::ActiveModel`. Each call generates a fresh UUID for
/// `id` and `username` so concurrent inserts in the test don't trip the
/// uniqueness constraint on `users.username`.
fn make_user(prefix: &str) -> users::ActiveModel {
    users::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        username: Set(format!("{prefix}-{}", Uuid::new_v4())),
        email: Set(None),
        password_hash: Set("placeholder".to_string()),
        preferred_character_id: Set(None),
        preferred_body_id: Set(None),
        preferred_wheel_id: Set(None),
        preferred_glider_id: Set(None),
        preferred_drink_type_id: Set(None),
        refresh_token_version: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    }
}

//! Per-call timeout helpers for database operations.
//!
//! Wraps a `SeaORM` future in `tokio::time::timeout` so a stuck query can't
//! pin a worker indefinitely. Elapsed budgets map to [`Error::Timeout`]
//! (504 Gateway Timeout); the inner error type's `From<E>` impl handles the
//! non-timeout path, so `?` at the call site still produces a single unified
//! [`Error`]. See `docs/coding-standards/tokio.md` § 12.
//!
//! Two public entry points by budget:
//! - [`db_query`] — 2 s, for single-statement reads/writes.
//! - [`db_txn`]   — 5 s, for transactions and their `begin` / `commit` calls.
//!
//! ```ignore
//! use beerio_kart::timeout::db_query;
//!
//! let row = db_query(users::Entity::find_by_id(id).one(db)).await?;
//! ```

use std::{future::Future, time::Duration};

use crate::error::Error;

/// Default budget for a single-statement database call.
pub const QUERY_BUDGET: Duration = Duration::from_secs(2);

/// Default budget for a transaction body, including `begin` and `commit`.
///
/// Wider than [`QUERY_BUDGET`] because a transaction reasonably bundles
/// multiple statements; tightening this is a future operational decision once
/// real load data exists.
pub const TXN_BUDGET: Duration = Duration::from_secs(5);

/// Wrap a single-statement DB future in a 2 s timeout.
///
/// On elapsed budget the wrapped future is dropped — `SeaORM`'s cancellation
/// passes through to the underlying `sqlx` future — and [`Error::Timeout`] is
/// returned. On a non-timeout inner error, the `Error: From<E>` impl converts
/// the inner error into an `Error` variant.
///
/// # Errors
///
/// Returns [`Error::Timeout`] with `budget = QUERY_BUDGET` if the wrapped
/// future does not complete within the budget; otherwise propagates the inner
/// error converted via `Error::from(E)`.
pub async fn db_query<F, T, E>(fut: F) -> Result<T, Error>
where
    F: Future<Output = Result<T, E>>,
    Error: From<E>,
{
    with_budget(QUERY_BUDGET, fut).await
}

/// Wrap a transaction-scoped DB future in a 5 s timeout. Use for `db.begin()`,
/// `txn.commit()`, and any sequence the caller treats as a single logical
/// transaction.
///
/// # Errors
///
/// Returns [`Error::Timeout`] with `budget = TXN_BUDGET` if the wrapped future
/// does not complete within the budget; otherwise propagates the inner error
/// converted via `Error::from(E)`.
pub async fn db_txn<F, T, E>(fut: F) -> Result<T, Error>
where
    F: Future<Output = Result<T, E>>,
    Error: From<E>,
{
    with_budget(TXN_BUDGET, fut).await
}

async fn with_budget<F, T, E>(budget: Duration, fut: F) -> Result<T, Error>
where
    F: Future<Output = Result<T, E>>,
    Error: From<E>,
{
    match tokio::time::timeout(budget, fut).await {
        Ok(inner) => inner.map_err(Error::from),
        Err(_elapsed) => Err(Error::Timeout { budget }),
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::DbErr;

    use super::*;

    #[tokio::test]
    async fn test_db_query_passes_ok_result_through() {
        let result: Result<i32, Error> = db_query(async { Ok::<i32, DbErr>(42) }).await;
        assert_eq!(result.expect("Ok branch"), 42);
    }

    #[tokio::test]
    async fn test_db_query_converts_inner_err_via_from_impl() {
        // `DbErr::RecordNotFound` maps to `Error::NotFound` via the existing
        // `From<DbErr> for Error` impl. The helper must route inner errors
        // through that conversion, not swallow them as Timeout.
        let result: Result<(), Error> =
            db_query(async { Err::<(), DbErr>(DbErr::RecordNotFound("user not found".into())) })
                .await;
        match result {
            Err(Error::NotFound(msg)) => assert_eq!(msg, "user not found"),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_db_query_returns_timeout_with_query_budget_when_elapsed() {
        // `start_paused = true` lets the runtime auto-advance time when all
        // tasks are blocked on sleep, so the 2 s budget elapses instantly.
        let result: Result<(), Error> = db_query(async {
            tokio::time::sleep(Duration::from_mins(1)).await;
            Ok::<(), DbErr>(())
        })
        .await;
        match result {
            Err(Error::Timeout { budget }) => assert_eq!(budget, QUERY_BUDGET),
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_db_txn_returns_timeout_with_txn_budget_when_elapsed() {
        let result: Result<(), Error> = db_txn(async {
            tokio::time::sleep(Duration::from_mins(1)).await;
            Ok::<(), DbErr>(())
        })
        .await;
        match result {
            Err(Error::Timeout { budget }) => assert_eq!(budget, TXN_BUDGET),
            other => panic!("expected Timeout, got {other:?}"),
        }
    }

    #[tokio::test(start_paused = true)]
    async fn test_db_query_inner_completes_before_budget_returns_ok() {
        // Inner future yields once then returns Ok within the budget — the
        // timeout must not fire and the value must come through unchanged.
        let result: Result<i32, Error> = db_query(async {
            tokio::time::sleep(Duration::from_millis(500)).await;
            Ok::<i32, DbErr>(7)
        })
        .await;
        assert_eq!(result.expect("Ok branch"), 7);
    }
}

//! Database connection setup.
//!
//! Per `seaorm.md` § 8: `SQLite` enforces `journal_mode` at the database-file
//! level (sticky) but `busy_timeout`, `synchronous`, and `foreign_keys` are
//! per-connection — they reset on every new connection in the pool. The
//! pre-PR-B2 startup pattern of `Database::connect(url)` followed by a single
//! `PRAGMA foreign_keys = ON` only configured the connection that served that
//! statement; subsequent pool connections opened later had FKs disabled. This
//! module fixes that by building the `SQLx` pool with `SqliteConnectOptions`
//! (which applies the per-connection PRAGMAs at connection setup time) and
//! wrapping it with `SqlxSqliteConnector`.

use std::{str::FromStr, time::Duration};

use sea_orm::{DatabaseConnection, DbErr, RuntimeErr, SqlxSqliteConnector};
use sqlx::{
    ConnectOptions,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};

/// Connect to the `SQLite` database with per-connection PRAGMAs applied.
///
/// Returns a `DatabaseConnection` whose every pool connection has WAL mode,
/// synchronous=Normal, `busy_timeout=5s`, and foreign keys enforced.
///
/// # Errors
///
/// Returns `DbErr` if `url` fails to parse as a `SQLite` connection string,
/// the pool can't be created (e.g., file permission errors for a file-backed
/// `SQLite`), or the initial connection handshake fails.
pub async fn connect(url: &str) -> Result<DatabaseConnection, DbErr> {
    let sqlx_opts = SqliteConnectOptions::from_str(url)
        .map_err(|e| DbErr::Conn(RuntimeErr::SqlxError(e)))?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true)
        .log_statements(log::LevelFilter::Debug);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Some(Duration::from_secs(60)))
        .connect_with(sqlx_opts)
        .await
        .map_err(|e| DbErr::Conn(RuntimeErr::SqlxError(e)))?;

    Ok(SqlxSqliteConnector::from_sqlx_sqlite_pool(pool))
}

#[cfg(test)]
mod tests {
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{
        ActiveModelTrait, ActiveValue::NotSet, EntityTrait, FromQueryResult, PaginatorTrait, Set,
        Statement, TransactionTrait,
    };
    use uuid::Uuid;

    use super::*;
    use crate::entities::users;

    /// Drives a connection through the new `connect()` path against an in-memory
    /// `SQLite` database. `cache=shared` makes the same DB visible to every pool
    /// connection (otherwise each connection gets its own ephemeral DB and
    /// migrations applied on one wouldn't be visible on the next).
    async fn shared_memory_db() -> DatabaseConnection {
        // Each test gets a unique URL so its named-memory DB doesn't collide
        // with parallel tests in the same process.
        let url = format!("sqlite:file:{}?mode=memory&cache=shared", Uuid::new_v4());
        let db = connect(&url).await.expect("connect");
        Migrator::up(&db, None).await.expect("migrations");
        db
    }

    #[tokio::test]
    async fn test_connect_enforces_foreign_keys_on_inserts() {
        // The behavioral test: with FKs enforced per-connection, an insert that
        // references a non-existent character must fail. Pre-PR-B2 this would
        // only fail if the query happened to land on the connection that ran
        // the startup PRAGMA — flaky and pool-size-dependent.
        let db = shared_memory_db().await;
        let result = users::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            username: Set("alice".to_string()),
            email: Set(None),
            password_hash: Set("placeholder".to_string()),
            preferred_character_id: Set(Some(99_999)),
            preferred_body_id: Set(None),
            preferred_wheel_id: Set(None),
            preferred_glider_id: Set(None),
            preferred_drink_type_id: Set(None),
            refresh_token_version: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await;
        assert!(
            result.is_err(),
            "insert with bogus FK should fail when foreign_keys=ON"
        );
    }

    #[tokio::test]
    async fn test_connect_enforces_foreign_keys_on_non_first_pool_connection() {
        // The behavioural test for Issue #140: with `foreign_keys = ON`
        // applied per-connection via `SqliteConnectOptions::foreign_keys(true)`
        // at pool-open time, every pool connection enforces FKs — not just
        // the one that handled a one-shot startup PRAGMA.
        //
        // Pin connection #1 with a transaction so the subsequent `&db` insert
        // *must* go through a freshly-opened pool connection. With the
        // pre-#140 setup (`Database::connect(url)` + `execute_unprepared
        // ("PRAGMA foreign_keys = ON")`), only the one connection that
        // happened to serve the startup PRAGMA had FKs enabled. Connection
        // #2+ would happily insert this bogus FK row, and the test would
        // fail. Under `db::connect()`, every connection has FKs at birth and
        // the violation surfaces here.
        let db = shared_memory_db().await;

        let pin = db.begin().await.expect("pin connection #1");

        let result = users::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            username: Set("alice".to_string()),
            email: Set(None),
            password_hash: Set("placeholder".to_string()),
            preferred_character_id: Set(Some(99_999)),
            preferred_body_id: Set(None),
            preferred_wheel_id: Set(None),
            preferred_glider_id: Set(None),
            preferred_drink_type_id: Set(None),
            refresh_token_version: Set(0),
            created_at: NotSet,
            updated_at: NotSet,
        }
        .insert(&db)
        .await;

        // Release the pin before the assertion so we don't leak the txn on
        // a panic path.
        let _ = pin.rollback().await;

        assert!(
            result.is_err(),
            "insert with bogus FK must fail on non-#1 pool connection too"
        );
    }

    #[tokio::test]
    async fn test_shared_cache_schema_visible_across_pool_connections() {
        // Two concurrent transactions force the pool to hand out two distinct
        // connections (each `begin()` pins one for the lifetime of the txn).
        // Both must be able to SELECT from `users` — proving that the
        // migration ran on connection #1 is visible to connection #2, i.e.
        // the `cache=shared` URL is working. Without it (or with a plain
        // `sqlite::memory:` URL), the second connection would open a fresh,
        // empty in-memory DB and the query would fail with "no such table".
        let db = shared_memory_db().await;

        let txn_a = db.begin().await.expect("begin txn a");
        let txn_b = db.begin().await.expect("begin txn b");

        let count_a = users::Entity::find()
            .count(&txn_a)
            .await
            .expect("count via txn a");
        let count_b = users::Entity::find()
            .count(&txn_b)
            .await
            .expect("count via txn b");

        assert_eq!(count_a, 0);
        assert_eq!(count_b, 0);

        txn_a.commit().await.expect("commit a");
        txn_b.commit().await.expect("commit b");
    }

    #[tokio::test]
    async fn test_connect_uses_wal_journal_mode() {
        // `cache=shared` memory databases report `memory` for journal_mode,
        // not `wal`. Use a temp file path to verify WAL is actually applied.
        let path = std::env::temp_dir().join(format!("beerio-kart-test-{}.db", Uuid::new_v4()));
        let url = format!("sqlite:{}", path.display());
        let db = connect(&url).await.expect("connect");

        // PRAGMA journal_mode returns the mode name as text.
        let stmt = Statement::from_string(
            sea_orm::DatabaseBackend::Sqlite,
            "PRAGMA journal_mode".to_string(),
        );
        let row = sea_orm::JsonValue::find_by_statement(stmt)
            .one(&db)
            .await
            .expect("pragma query")
            .expect("pragma row");
        assert_eq!(row["journal_mode"], "wal");

        // Cleanup. Best-effort; not asserting since the connection holds it.
        drop(db);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
    }
}

# SeaORM — Coding Standards

> **Scope.** SeaORM 1.x with SQLite for the `beerio-kart` backend. Features: `["sqlx-sqlite", "runtime-tokio-rustls", "macros"]`. Workspace member crate `migration` for migrations.
> **Format.** *Rule / Why / Example / Source*.
> **Companions.** `rust.md`, `tokio.md`, `../api-contract.md`, `../compliance-plan.md`.

## Index

1. [ActiveModel vs Model](#1-activemodel-vs-model)
2. [Find / query patterns](#2-find--query-patterns)
3. [Transactions](#3-transactions)
4. [`ConnectionTrait` abstraction](#4-connectiontrait-abstraction)
5. [Migrations](#5-migrations)
6. [Entity organization](#6-entity-organization)
7. [Error handling](#7-error-handling)
8. [Connection pool & SQLite performance](#8-connection-pool--sqlite-performance)
9. [Testing](#9-testing)
10. [Raw SQL escape hatch](#10-raw-sql-escape-hatch)
11. [Relations](#11-relations)
12. [Pitfalls (consolidated)](#12-pitfalls-consolidated)

---

## 1. ActiveModel vs Model

`Model` is a plain struct returned from `SELECT`. `ActiveModel` is the mutable form that drives `INSERT`/`UPDATE`; each field is wrapped in `ActiveValue<T>` with three states: `Set(v)`, `Unchanged(v)`, `NotSet`. Only `Set` columns appear in the generated SQL.

- **Rule:** Use `ActiveModel` for inserts and updates. Always.
  - **Why:** `Model` has no concept of "this column wasn't touched" — you'd send every column on every UPDATE, which fights with DB defaults and clobbers concurrent writers.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/basic-crud/active-model/>

- **Rule:** Distinguish `NotSet` from `Set(None)` deliberately.
  - **Why:** `NotSet` omits the column (so DB defaults / auto-increment fire). `Set(None)` writes a literal `NULL`. They are not interchangeable.
  - **Source:** <https://github.com/SeaQL/sea-orm/discussions/770>

- **Rule:** For partial updates, query the row, convert into `ActiveModel`, mutate the fields you intend to change, call `.update(db)`.
  - **Why:** Querying first gives you `Unchanged(...)` for every field. Only the fields you reassign with `Set(...)` end up in the `UPDATE`. You don't accidentally overwrite columns you didn't touch.
  - **Example:**
    ```rust
    let mut am: sessions::ActiveModel = sessions::Entity::find_by_id(id.to_string())
        .one(db).await?
        .ok_or_else(|| AppError::NotFound("session".into()))?
        .into();
    am.status = Set(SessionStatus::Closed.into());
    am.last_activity_at = Set(Utc::now().to_rfc3339());
    am.update(db).await?;
    ```
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/basic-crud/update/>

- **Rule:** For set-based updates, use `Entity::update_many().filter(...).col_expr(...)` — not fetch + save in a loop.
  - **Why:** A single `UPDATE ... WHERE` is one round-trip and atomic; the loop is N round-trips and a race condition. Particularly important for SQLite where every write serializes globally.
  - **Example (the planned 1-hour stale-session cleanup):**
    ```rust
    sessions::Entity::update_many()
        .col_expr(sessions::Column::Status, Expr::value("closed"))
        .filter(sessions::Column::LastActivityAt.lt(threshold_iso))
        .filter(sessions::Column::Status.eq("active"))
        .exec(db).await?;
    ```

- **Rule:** Implement `ActiveModelBehavior::before_save` for `created_at` / `updated_at`. Don't sprinkle `Set(now)` across every service.
  - **Why:** `before_save` fires on insert and update with an `insert: bool` flag. Centralizing it removes "the developer forgot to bump `updated_at`" entirely. Keep the impl in a sibling file (e.g. `entities/users_behavior.rs`) so codegen doesn't clobber it.
  - **Example:**
    ```rust
    #[async_trait]
    impl ActiveModelBehavior for users::ActiveModel {
        async fn before_save<C: ConnectionTrait>(mut self, _db: &C, insert: bool)
            -> Result<Self, DbErr>
        {
            let now = Utc::now().to_rfc3339();
            if insert { self.created_at = Set(now.clone()); }
            self.updated_at = Set(now);
            Ok(self)
        }
    }
    ```
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/entity/trait.ActiveModelBehavior.html>

- **Rule:** Use `DeriveIntoActiveModel` on request DTOs so route handlers go directly from `Json<CreateRunRequest>` to `ActiveModel`.
  - **Why:** Cuts boilerplate; `Option<T>` fields naturally become `NotSet` when absent, which is exactly what you want for partial updates.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/advanced-query/custom-active-model/>

## 2. Find / query patterns

- **Rule:** Use `Entity::find_by_id(pk)` for primary-key lookups; reserve `find()` + `filter()` for everything else.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/basic-crud/select/>

- **Rule:** Pick the terminator that matches cardinality. `.one()` → `Option<Model>`. `.all()` → `Vec<Model>` only when bounded by an upstream `LIMIT` or business invariant. `.paginate(db, n)` for unbounded lists. `.stream(db)` for lazy iteration.
  - **Why:** `.all()` on `runs` will happily load every run in the database. Don't.
  - **Example:**
    ```rust
    let mut pages = runs::Entity::find()
        .filter(runs::Column::UserId.eq(user_id.to_string()))
        .order_by_desc(runs::Column::CreatedAt)
        .paginate(db, 50);
    while let Some(page) = pages.fetch_and_next().await? { /* ... */ }
    ```
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/struct.Paginator.html>

- **Rule:** For "list-with-children" reads, prefer `LoaderTrait::load_many` (two queries, no row duplication) over `find_with_related` (one big JOIN that explodes parent rows N times).
  - **Why:** SeaORM eager loading via JOIN duplicates the parent row once per child. With a session that has 8 participants and 12 races, the loader transfers far less data than the JOIN.
  - **Example:**
    ```rust
    let sessions = sessions::Entity::find().all(db).await?;
    let participants: Vec<Vec<session_participants::Model>> =
        sessions.load_many(session_participants::Entity, db).await?;
    ```
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/relation/data-loader/>

- **Rule:** Compose multi-clause `WHERE`s with `Condition::all()` / `Condition::any()` when mixing AND and OR. Don't chain `.filter()` for that case.
  - **Why:** Chained `.filter()` is always AND. Mixing it with a `Condition::any()` argument has produced order-of-operations bugs (sea-query #414).
  - **Example:**
    ```rust
    let cond = Condition::all()
        .add(runs::Column::UserId.eq(user_id.to_string()))
        .add(Condition::any()
            .add(runs::Column::Disqualified.eq(false))
            .add(runs::Column::PhotoPath.is_not_null()));
    runs::Entity::find().filter(cond).all(db).await?;
    ```
  - **Source:** <https://github.com/SeaQL/sea-query/issues/414>

- **Rule:** When you need a subset of columns or an aggregate, define a struct with `#[derive(FromQueryResult)]` and use `select_only().column(...)`.
  - **Why:** Avoids returning the full row when you need two fields; gives you a typed result.
  - **Example:** `services/sessions.rs::ActiveParticipantRow` already follows this pattern. Copy it.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/advanced-query/custom-select/>

- **Rule:** For paginated list endpoints (../design.md mandates cursor pagination on `GET /runs`), use `Entity::find().cursor_by(Column::CreatedAt)` rather than `.paginate(db, n).fetch_page(p)`.
  - **Why:** Page-offset pagination duplicates/skips rows when the underlying set changes. Cursor pagination is stable and O(log n).
  - **Source:** <https://github.com/SeaQL/sea-orm/pull/822>

## 3. Transactions

- **Rule:** Wrap any handler that performs more than one write in a transaction. No exceptions.
  - **Why:** A partial failure halfway through "create session, add participant, insert race" leaves orphaned rows. SQLite's default isolation does not save you.
  - **Example:**
    ```rust
    db.transaction::<_, SessionDetail, AppError>(|txn| Box::pin(async move {
        let session = create_session_row(txn, &input).await?;
        add_participant(txn, &session.id, &user.id).await?;
        Ok(load_session_detail(txn, &session.id).await?)
    })).await.map_err(Into::into)
    ```
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/advanced-query/transaction/>

- **Rule:** Prefer the closure API (`db.transaction(|txn| ...)`) over manual `begin/commit/rollback`. The boxed-pin syntax is awkward; that ugliness is the cost of the right behavior. Don't try to wriggle out of it with manual `begin` + `commit`.
  - **Why:** Manual transactions leak when you `?`-bail out before commit. The closure form rolls back automatically on `Err`.
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/struct.DatabaseTransaction.html>

- **Rule:** Never hold a `DatabaseTransaction` across an HTTP request boundary or across long-running async work that doesn't need the DB.
  - **Why:** Transactions take a connection out of the pool. On SQLite, an open write transaction blocks all other writers until commit or busy-timeout. Minimum scope possible is the right scope.
  - **Source:** <https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/>

- **Rule:** Don't put CPU-bound work (Argon2, image resize) inside a transaction.
  - **Why:** The transaction holds the SQLite write lock for the entire `.await`. A 100ms hash inside a transaction is a 100ms global write stall.

## 4. `ConnectionTrait` abstraction

- **Rule:** Service functions take `&impl ConnectionTrait` (or generic `<C: ConnectionTrait>`), not `&DatabaseConnection`.
  - **Why:** Lets the same function be called both standalone and inside a transaction, without two copies. The current `services/sessions.rs` already does this — it's the right pattern; codify it.
  - **Example:**
    ```rust
    pub async fn add_participant(
        db: &impl ConnectionTrait,
        session_id: &str,
        user_id: &str,
    ) -> Result<session_participants::Model, AppError> { /* ... */ }
    ```
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/trait.ConnectionTrait.html>

- **Rule:** If a service streams rows, bound on `ConnectionTrait + StreamTrait`.
  - **Why:** `.stream()` requires `StreamTrait`; bare `ConnectionTrait` doesn't include it.
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/trait.StreamTrait.html>

- **Rule:** Top-level orchestration (route handlers) is the only place `&DatabaseConnection` should appear, and only to start a transaction or pass into a service.
  - **Why:** Concentrates pool / txn decisions at the request boundary; everything below is agnostic and testable.

- **Rule:** Don't store a `DatabaseTransaction` in `axum::Extension` or `State`.
  - **Why:** Axum state needs `Clone` and is shared across requests; a transaction is per-request. A `RwLock<DatabaseTransaction>` in state serializes the entire app.
  - **Source:** <https://github.com/SeaQL/sea-orm/issues/2162>

## 5. Migrations

**"Launch" definition.** "Launch" in the migration policy means *the moment we want to preserve database data between deployments*. As of May 2026, we have no real data and the schema is still in flux — we're prelaunch. Once we deploy a version where dropping the DB to apply schema changes would lose data we care about, we flip to append-only migrations and the consolidated migration becomes immutable history. CLAUDE.md will be updated at that time.

- **Rule:** While prelaunch (data is throwaway), edit the single consolidated migration file in place. Never append a new migration file. Reset the dev DB after schema edits.
  - **Why:** Pre-real-data, the audit history of a migration doesn't earn its keep. One file with the current schema is simpler than N append-only files when there's no data to preserve.
  - **Source:** Project policy (`.claude/CLAUDE.md` schema-changes section).

- **Rule:** Once we cross the launch threshold (data must persist), schema changes become append-only — every change is a new migration file, the consolidated initial migration is immutable.
  - **Why:** From that moment forward, migrations are forensic history. Editing one is editing the past.

- **Rule:** Write migrations using SeaQuery's `Table::create()` / `Index::create()`, not raw SQL strings.
  - **Why:** SeaQuery generates the right dialect for each backend (SQLite now, Postgres later, per ../design.md).
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/migration/writing-migration/>

- **Rule:** Don't use `Schema::create_table_from_entity(...)` in migration code.
  - **Why:** It uses the *current* entity definitions, so when you change an entity later, the historical migration silently changes its meaning. Hand-written SeaQuery migrations are immutable history (post-launch), or at least an explicit single source of truth (prelaunch).
  - **Source:** <https://github.com/SeaQL/sea-orm/discussions/325>

- **Rule:** Implement both `up` and `down`, even prelaunch.
  - **Why:** `down` costs almost nothing to write at create-time and gives you a working `migrate down` for local experimentation. SeaORM enforces it on the trait.

- **Rule:** Avoid `ALTER COLUMN` / `DROP COLUMN` patterns on SQLite. If you must, do "create new table → copy → drop old → rename" inside a transaction.
  - **Why:** SQLite's `ALTER TABLE` is severely limited; type changes aren't supported. SeaORM has open issues where multi-step ALTER migrations on SQLite fail.
  - **Source:** <https://sqlite.org/lang_altertable.html>, <https://github.com/SeaQL/sea-orm/issues/2303>

## 6. Entity organization

- **Rule:** Treat the generated `entities/*.rs` files as build output. No hand-edits.
  - **Why:** `sea-orm-cli generate entity` clobbers them on regen. Custom logic on a generated file is a time bomb.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/generate-entity/entity-first/>

- **Rule:** Put domain logic in sibling modules (`services/`, `domain/`), not on entity types.
  - **Why:** Keeps generated code generated and domain code domain. Free functions taking `&impl ConnectionTrait` and `&Model` references compose better than methods.

- **Rule:** Implement `ActiveModelBehavior` in a sibling file under `entities/` (e.g. `entities/users_behavior.rs`) wired in through `entities/mod.rs`. Do **not** edit the generated `users.rs`.
  - **Why:** This is the one trait you legitimately need to implement on a generated type. Putting it in a sibling preserves "regenerate freely."

- **Rule:** Add `Serialize` / `Deserialize` via `sea-orm-cli generate entity --with-serde both`, not by hand. The flag persists across regens; hand-added derives don't.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/generate-entity/sea-orm-cli/>

- **Rule:** Newtype boundary. Entities use codegen-default primitives (`String` for UUID columns, `i32` for INTEGER columns). Conversion to/from domain newtypes (`UserId`, `RaceTimeMs`, etc. — see `rust.md` § 2) happens at the entity↔service boundary inside the service layer. Don't try to teach codegen to use newtypes directly; the entity boundary is the explicit conversion point.
  - **Example:**
    ```rust
    // In services/users.rs:
    let model = users::Entity::find_by_id(user_id.to_string()).one(db).await?
        .ok_or_else(|| AppError::NotFound("user".into()))?;
    let user = User {
        id: UserId::try_from(model.id.as_str())?,
        username: Username::try_from(model.username.as_str())?,
        // ...
    };
    ```
  - **Why:** Codegen can't be reliably taught to use third-party newtypes. The boundary conversion is one place to audit, easy to test, and the cost (one block of `try_from`s per entity) is small.

## 7. Error handling

`DbErr` variants worth distinguishing:
- `RecordNotFound(String)` — surfaced from `.update()` / `.delete_by_id()` / similar when the row didn't exist. **Note:** `Entity::find().one(db)` returns `Ok(None)`, *not* this error.
- `RecordNotInserted` / `RecordNotUpdated` — zero rows affected.
- `Exec`, `Query`, `Conn` — wrapping the inner SQLx error.
- `Custom`, `Type`, `Json`, `Migration`.

- **Rule:** Don't blanket-convert `DbErr` to `AppError::Internal`. Inspect the variant first.
  - **Why:** A blanket `From<DbErr> for AppError::Internal` collapses every database error into a 500. That hides 404s (`RecordNotFound`) and 409s (UNIQUE / FK violations).
  - **Example:**
    ```rust
    impl From<sea_orm::DbErr> for AppError {
        fn from(e: sea_orm::DbErr) -> Self {
            match &e {
                DbErr::RecordNotFound(msg) => AppError::NotFound(msg.clone()),
                _ => match e.sql_err() {
                    Some(SqlErr::UniqueConstraintViolation(m)) => AppError::Conflict(m),
                    Some(SqlErr::ForeignKeyConstraintViolation(m)) => AppError::BadRequest(m),
                    _ => AppError::Internal(format!("Database error: {e}")),
                },
            }
        }
    }
    ```
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/advanced-query/error-handling/>

- **Rule:** For "not found" lookups, `find_by_id().one(db).await?.ok_or_else(|| AppError::NotFound(...))`. Never `.unwrap()`.
  - **Why:** `.one()` returns `Ok(None)` for "no row", which makes 404 a control-flow result, not a panic.

- **Rule:** Use `DbErr::sql_err()` rather than string-matching the inner SQLx error.
  - **Why:** SeaORM normalizes UNIQUE / FK violations across backends. String-matching breaks when SQLite versions or backends change.

- **Rule:** Never `unwrap_or_default()` an `Option<Model>`.
  - **Why:** A defaulted `Model` (zeroed/empty fields) silently replaces "not found", producing wrong answers downstream.

## 8. Connection pool & SQLite performance

- **Rule:** Configure `ConnectOptions` explicitly. Don't call `Database::connect(url)` with defaults.
  - **Example:**
    ```rust
    let mut opt = ConnectOptions::new(database_url);
    opt.max_connections(5)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(60))
        .sqlx_logging(true)
        .sqlx_logging_level(log::LevelFilter::Debug); // not Info
    ```
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/struct.ConnectOptions.html>

- **Rule:** Keep `max_connections` small for SQLite (≤ ~5–10).
  - **Why:** SQLite serializes writers — `max_connections=100` doesn't give you 100 concurrent writes, just 100 connections fighting for one write lock and timing out.
  - **Source:** <https://emschwartz.me/psa-your-sqlite-connection-pool-might-be-ruining-your-write-performance/>

- **Rule:** Apply per-connection PRAGMAs by building the SQLx pool with `SqliteConnectOptions` and wrapping via `SqlxSqliteConnector::from_sqlx_sqlite_pool`.
  - **Why:** `journal_mode=WAL` is sticky to the database file, but `busy_timeout`, `synchronous`, and `foreign_keys` are *per-connection* and reset on every new connection. Running `PRAGMA foreign_keys = ON` once at startup only affects the one connection that served that statement — newly opened pool connections don't have FKs enforced.
  - **Example:**
    ```rust
    use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions, SqliteJournalMode, SqliteSynchronous};
    let sqlx_opts = SqliteConnectOptions::from_str(&url)?
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(Duration::from_secs(5))
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(sqlx_opts).await?;
    let db = SqlxSqliteConnector::from_sqlx_sqlite_pool(pool);
    ```
  - **Source:** <https://docs.rs/sea-orm/latest/sea_orm/struct.SqlxSqliteConnector.html>, <https://docs.rs/sqlx/latest/sqlx/sqlite/struct.SqliteConnectOptions.html>

- **Rule:** Set `sqlx_logging_level(LevelFilter::Debug)`, not the default `Info`.
  - **Why:** Default `Info` logs every SQL statement. That's noise in prod and a privacy risk if values include user data.

## 9. Testing

- **Rule:** Default to in-memory SQLite for service / integration tests. Use `sqlite::memory:?cache=shared` so multiple pool connections share one in-memory database.
  - **Why:** Each connection to `sqlite::memory:` *without* `cache=shared` gets a fresh, separate database. With a pool size > 1, the second SeaORM call may land on a connection that has zero tables, producing confusing "no such table" errors. `?cache=shared` makes the pool-wide DB single.
  - **Example:**
    ```rust
    let db = Database::connect("sqlite::memory:?cache=shared").await?;
    Migrator::up(&db, None).await?;
    // seed and exercise services
    ```
  - **Source:** <https://www.sqlite.org/inmemorydb.html#sharedmemdb>

- **Rule:** Each integration test gets its own in-memory database (one per `#[tokio::test]`). For shared-cache mode, parameterize the URL with a unique cache name per test if cross-test isolation matters.
  - **Why:** Sharing a connection means tests order-depend on each other.

- **Rule:** Reach for `MockDatabase` only for genuine unit tests of pure logic — never for integration tests that should hit real SQLite.
  - **Why:** `MockDatabase` returns whatever you tell it to. A buggy query passes the test by accident. In-memory SQLite catches SQL bugs, constraint violations, and type mismatches.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/write-test/sqlite/>

- **Rule:** Don't unit-test query construction (the SQL string). Test behavior — given seeded data, the service returns the right result.
  - **Why:** Mock-checking that "the SQL contains 'WHERE foo = ?'" reproduces SeaORM's output, not your logic.

## 10. Raw SQL escape hatch

- **Rule:** Drop to `find_by_statement(Statement::from_sql_and_values(...))` only when the builder genuinely can't express the query — multi-table JOINs with non-trivial conditions, window functions, recursive CTEs.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/basic-crud/raw-sql/>

- **Rule:** When you do go to raw SQL, **always parameterize**. Never `format!()` user input into the SQL string.
  - **Why:** SQL injection is the same vulnerability whether you use SeaORM or hand-rolled. `from_sql_and_values` binds the values as parameters; `format!` interpolates them as literals.
  - **Example:**
    ```rust
    // CORRECT
    Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT * FROM runs WHERE user_id = $1 AND track_time < $2",
        [user_id.to_string().into(), max_time.into()],
    )
    // WRONG — DO NOT DO THIS
    Statement::from_string(
        db.get_database_backend(),
        format!("SELECT * FROM runs WHERE user_id = '{}'", user_id),
    )
    ```

- **Rule:** Raw queries return into `#[derive(FromQueryResult)]` structs, not `Model`.
  - **Why:** A raw join doesn't shape into a single entity. A result struct documents which columns the query produces and gives compile-time type safety.
  - **Example:** `services/sessions.rs::ActiveParticipantRow` is the model to copy.

## 11. Relations

- **Rule:** Trust codegen for vanilla one-to-many / many-to-one relations. The CLI infers them from foreign keys.

- **Rule:** Define many-to-many relations manually (in a sibling module) by implementing `Related` on both sides through the junction entity.
  - **Why:** Codegen produces parent ↔ junction and junction ↔ child but not the direct `Related<Child> for Parent` impl.
  - **Source:** <https://www.sea-ql.org/SeaORM/docs/relation/many-to-many/>

- **Rule:** When two FKs link the same pair of tables, give them distinct `Relation` variants and reference them by name.
  - **Why:** Codegen historically produces ambiguous relations when there are multiple FKs between the same two tables (#405). Without distinct names, "find related users" doesn't know which FK to follow.
  - **Note:** A live instance of this case (`sessions.created_by` and `sessions.host_id` both → `users.id`) is being eliminated by dropping `created_by`. After that change lands, the multi-FK case is hypothetical for current schema but the rule still applies for any future table that reuses a target.
  - **Source:** <https://github.com/SeaQL/sea-orm/issues/405>

## 12. Pitfalls (consolidated)

A single-screen review checklist:

- Don't `unwrap()` on `Option<Model>` from `.one()`.
- Don't blanket-`From<DbErr> for AppError::Internal` (see § 7).
- Don't run a one-time PRAGMA against `db` and assume it sticks (see § 8).
- Don't hold transactions across HTTP boundaries or long awaits.
- Don't put a `DatabaseConnection` behind a `Mutex` / `RwLock` in app state.
- Don't use `find_with_related` for large parent lists — use `LoaderTrait`.
- Don't mix builder `Condition` with chained `.filter()` when expressing OR.
- Don't use `Schema::create_table_from_entity` in migrations.
- Don't use `sqlite::memory:` without `?cache=shared` in pooled tests.
- Don't auto-derive `Default` on an `ActiveModel` and expect timestamps to populate — `Default` produces `NotSet` for all fields. Set timestamps via `before_save` (§ 1).

---

## Document history

- 2026-05-02 — Initial draft as part of `docs/rust-coding-standards.md`.
- 2026-05-02 — Split into `docs/coding-standards/seaorm.md`. Added explicit "launch" definition to § 5. Updated § 9 to use `?cache=shared`. Added entity↔domain newtype boundary rule to § 6. Noted upcoming `sessions.created_by` removal in § 11.

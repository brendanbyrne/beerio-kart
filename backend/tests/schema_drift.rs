//! Schema-drift verification: prove that every entity in
//! `backend/src/entities/` can load from a freshly-migrated database.
//!
//! Implements PR-X2 / § 7 of
//! `docs/designs/archive/2026-05-02-entity-codegen-strategy.md`. Now that entities
//! are committed source (per `coding-standards/seaorm.md` § 6), codegen no
//! longer acts as an implicit drift-checker between migration and entity.
//! This test replaces that signal with a single CI-time guarantee: for every
//! entity, force `SeaORM` to issue a `SELECT` covering all declared columns
//! against the schema the migration produces.
//!
//! # What this catches
//!
//! - **Entity declares a column the migration didn't create** → the issued
//!   `SELECT` references a non-existent column and `SQLite` returns an error
//!   at execution time. Verified by manual injection during PR-X2 dev:
//!   adding a phantom `bogus_drift_column` to `run_flags` produced a clean
//!   `no such column: run_flags.bogus_drift_column` failure.
//! - **Entity points at a wrong table name** → same shape: `no such table:
//!   …` from `SQLite`.
//!
//! # What this does NOT catch
//!
//! - **Migration adds a column the entity doesn't declare.** The entity's
//!   `SELECT` only names the columns it knows about; extra columns in the
//!   table are silently ignored. Catching that direction requires a
//!   different mechanism (e.g. introspecting `PRAGMA table_info` and
//!   diffing against the entity's `Column` enum) which is out of scope for
//!   this test.
//! - **Type mismatch on a column that exists in both.** `SQLite` uses column
//!   *affinity*, not strict typing, so a `TEXT`-declared column happily
//!   accepts an integer value at the SQL layer. With `LIMIT 0` no rows are
//!   returned, so `SeaORM`'s per-row decode (where a Rust-level type
//!   mismatch would surface) never runs. Catching type drift requires
//!   seeding at least one row — out of scope for the bootstrap version of
//!   this test. Verified by manual injection: changing `run_flags.reason`
//!   from `String` to `i32` did **not** trip this test.
//! - **Relation-cardinality mismatches** (`has_one` vs `has_many`, missing
//!   `Related` impl, ambiguous M2M). These are structural, not column-shape,
//!   and rely on review attention plus the atomic-PR rule from CLAUDE.md.
//! - **Hand-corrected attributes** like the absent `unique` on
//!   `session_participants.user_id`. The entity deliberately omits `unique`
//!   so `SeaORM` doesn't infer `has_one` on the `users` ↔
//!   `session_participants` relation (see that entity's doc-comment for
//!   the full reasoning, including the two distinct migration constraints
//!   that justify it). Intentional deviation, not drift; review catches it.
//! - **Migration-only constraints** like `CHECK` clauses, partial indexes,
//!   or compound `UNIQUE`s that the entity doesn't model. The entity is a
//!   column-shape mirror, not a constraint mirror.
//!
//! # How to read a failure
//!
//! A failure here means the migration and the entity disagree on column
//! shape. Either the migration changed without a matching entity update, or
//! the entity added a column that wasn't applied to the migration. Open the
//! migration file (`backend/migration/src/m20260101_000001_initial_schema.rs`)
//! and the failing entity side-by-side and reconcile. CLAUDE.md's
//! "Schema changes (prelaunch)" rule requires the migration and the entity
//! to land in the same PR, so the fix is always atomic.

// Test legitimately wants to panic on drift — per `rust.md` § 8.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use beerio_kart::entities::{
    bodies, characters, cups, drink_types, gliders, notifications, run_flags, runs,
    session_participants, session_race_participations, session_races, sessions, tracks, users,
    wheels,
};
use migration::{Migrator, MigratorTrait};
use sea_orm::{Database, EntityTrait, QuerySelect};
use uuid::Uuid;

#[tokio::test]
async fn each_entity_can_load_from_a_fresh_migrated_db() {
    // Per-test unique in-memory database so this test never shares cache
    // state with concurrent tests; `cache=shared` is required so that any
    // future pool with > 1 connection in this test would still see one DB.
    // (See `seaorm.md` § 9.)
    let cache_name = Uuid::new_v4();
    let url = format!("sqlite:file:{cache_name}?mode=memory&cache=shared");
    let db = Database::connect(&url).await.unwrap();
    Migrator::up(&db, None).await.unwrap();

    // One line per entity. `find().limit(0)` issues a real `SELECT` that
    // names every declared column against the migrated schema, with zero
    // rows returned so we don't need any seed data. The check is structural
    // (column / table existence) — see the module-level doc-comment for the
    // `LIMIT 0` trade-off vs. type-decode coverage.
    bodies::Entity::find().limit(0).all(&db).await.unwrap();
    characters::Entity::find().limit(0).all(&db).await.unwrap();
    cups::Entity::find().limit(0).all(&db).await.unwrap();
    drink_types::Entity::find().limit(0).all(&db).await.unwrap();
    gliders::Entity::find().limit(0).all(&db).await.unwrap();
    notifications::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .unwrap();
    run_flags::Entity::find().limit(0).all(&db).await.unwrap();
    runs::Entity::find().limit(0).all(&db).await.unwrap();
    session_participants::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .unwrap();
    session_race_participations::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .unwrap();
    session_races::Entity::find()
        .limit(0)
        .all(&db)
        .await
        .unwrap();
    sessions::Entity::find().limit(0).all(&db).await.unwrap();
    tracks::Entity::find().limit(0).all(&db).await.unwrap();
    users::Entity::find().limit(0).all(&db).await.unwrap();
    wheels::Entity::find().limit(0).all(&db).await.unwrap();
}

//! Consolidated migration crate for Beerio Kart.
//!
//! Prelaunch convention (see `backend/CLAUDE.md` § Schema changes): all
//! schema lives in one consolidated migration file, edited in place rather
//! than appended to. [`Migrator`] is the entry point —
//! `Migrator::up(&db, None).await` applies the schema to a fresh database.

pub use sea_orm_migration::prelude::*;

/// Consolidated initial-schema migration — table, FK, and index DDL.
pub mod m20260101_000001_initial_schema;

/// Runs the consolidated initial-schema migration.
pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![Box::new(m20260101_000001_initial_schema::Migration)]
    }
}

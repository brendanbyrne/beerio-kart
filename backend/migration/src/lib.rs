pub use sea_orm_migration::prelude::*;

pub mod m20260330_000001_create_base_tables;
pub mod m20260330_000002_create_tracks;
pub mod m20260330_000003_create_drink_types;
pub mod m20260330_000004_create_users;
pub mod m20260330_000005_add_drink_types_created_by;
pub mod m20260330_000006_create_runs;
pub mod m20260330_000007_create_run_flags;
pub mod m20260330_000008_create_sessions;
pub mod m20260330_000009_create_session_participants;
pub mod m20260330_000010_create_session_races;
pub mod m20260330_000011_recreate_runs;
pub mod m20260330_000012_add_runs_unique_constraint;

pub struct Migrator;

#[async_trait::async_trait]
impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn MigrationTrait>> {
        vec![
            Box::new(m20260330_000001_create_base_tables::Migration),
            Box::new(m20260330_000002_create_tracks::Migration),
            Box::new(m20260330_000003_create_drink_types::Migration),
            Box::new(m20260330_000004_create_users::Migration),
            Box::new(m20260330_000005_add_drink_types_created_by::Migration),
            Box::new(m20260330_000006_create_runs::Migration),
            Box::new(m20260330_000007_create_run_flags::Migration),
            Box::new(m20260330_000008_create_sessions::Migration),
            Box::new(m20260330_000009_create_session_participants::Migration),
            Box::new(m20260330_000010_create_session_races::Migration),
            Box::new(m20260330_000011_recreate_runs::Migration),
            Box::new(m20260330_000012_add_runs_unique_constraint::Migration),
        ]
    }
}

use sea_orm_migration::prelude::*;

/// Drops and recreates the runs table with new columns (session_race_id,
/// disqualified) and run_flags. Uses raw SQL for SQLite STRICT mode.
/// Approved by Brendan — no production data to preserve.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conn = manager.get_connection();

        // Drop run_flags first (FK dependency on runs)
        conn.execute_unprepared("DROP TABLE IF EXISTS run_flags")
            .await?;

        // Drop old runs table
        conn.execute_unprepared("DROP TABLE IF EXISTS runs").await?;

        // Recreate runs with new columns and STRICT mode
        conn.execute_unprepared(
            "CREATE TABLE runs (
                id TEXT NOT NULL PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                session_race_id TEXT NOT NULL REFERENCES session_races(id),
                track_id INTEGER NOT NULL REFERENCES tracks(id),
                character_id INTEGER NOT NULL REFERENCES characters(id),
                body_id INTEGER NOT NULL REFERENCES bodies(id),
                wheel_id INTEGER NOT NULL REFERENCES wheels(id),
                glider_id INTEGER NOT NULL REFERENCES gliders(id),
                track_time INTEGER NOT NULL,
                lap1_time INTEGER NOT NULL,
                lap2_time INTEGER NOT NULL,
                lap3_time INTEGER NOT NULL,
                drink_type_id TEXT NOT NULL REFERENCES drink_types(id),
                disqualified INTEGER NOT NULL DEFAULT 0,
                photo_path TEXT,
                created_at TEXT NOT NULL,
                notes TEXT
            ) STRICT",
        )
        .await?;

        // Recreate run_flags with STRICT mode
        conn.execute_unprepared(
            "CREATE TABLE run_flags (
                id TEXT NOT NULL PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id),
                reason TEXT NOT NULL,
                note TEXT,
                hide_while_pending INTEGER NOT NULL DEFAULT 0,
                auto_generated INTEGER NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL,
                resolved_at TEXT
            ) STRICT",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Down migration not needed — this is a destructive wipe approved
        // during pre-production. The original migrations (006, 007) handle
        // the original schema.
        Ok(())
    }
}

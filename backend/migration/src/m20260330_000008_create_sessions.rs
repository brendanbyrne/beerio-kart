use sea_orm_migration::prelude::*;

/// Creates the sessions table — the organizational unit for group play.
/// Uses raw SQL for SQLite STRICT mode (SeaORM's builder doesn't support it).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT NOT NULL PRIMARY KEY,
                    created_by TEXT NOT NULL REFERENCES users(id),
                    host_id TEXT NOT NULL REFERENCES users(id),
                    ruleset TEXT NOT NULL,
                    least_played_drink_category TEXT,
                    status TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    last_activity_at TEXT NOT NULL
                ) STRICT",
            )
            .await?;

        // Index for list_active_sessions and close_stale_sessions queries
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_sessions_status_last_activity
                 ON sessions(status, last_activity_at)",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS sessions")
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Sessions {
    Table,
    Id,
    CreatedBy,
    HostId,
    Ruleset,
    LeastPlayedDrinkCategory,
    Status,
    CreatedAt,
    LastActivityAt,
}

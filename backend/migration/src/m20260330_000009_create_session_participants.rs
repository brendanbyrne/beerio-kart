use sea_orm_migration::prelude::*;

/// Creates the session_participants table — tracks who is in a session
/// and when they joined/left. Uses raw SQL for SQLite-specific features.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS session_participants (
                    id TEXT NOT NULL PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    user_id TEXT NOT NULL REFERENCES users(id),
                    joined_at datetime_text NOT NULL,
                    left_at datetime_text
                )",
            )
            .await?;

        // Index for the most common query pattern: filter by session + active status
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE INDEX idx_session_participants_session_active
                 ON session_participants(session_id, left_at)",
            )
            .await?;

        // A user can only be active in one session at a time.
        // Partial unique index: only applies to rows where left_at IS NULL.
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE UNIQUE INDEX idx_session_participants_one_active_session
                 ON session_participants(user_id) WHERE left_at IS NULL",
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS session_participants")
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum SessionParticipants {
    Table,
    Id,
    SessionId,
    UserId,
    JoinedAt,
    LeftAt,
}

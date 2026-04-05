use sea_orm_migration::prelude::*;

/// Creates the session_participants table — tracks who is in a session
/// and when they joined/left. Uses raw SQL for SQLite STRICT mode.
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
                    joined_at TEXT NOT NULL,
                    left_at TEXT
                ) STRICT",
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

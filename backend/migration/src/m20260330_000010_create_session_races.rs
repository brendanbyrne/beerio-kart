use sea_orm_migration::prelude::*;

/// Creates the session_races table — each race within a session.
/// Uses raw SQL for SQLite STRICT mode and composite unique index.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "CREATE TABLE IF NOT EXISTS session_races (
                    id TEXT NOT NULL PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    race_number INTEGER NOT NULL,
                    track_id INTEGER NOT NULL REFERENCES tracks(id),
                    chosen_by TEXT REFERENCES users(id),
                    created_at TEXT NOT NULL,
                    UNIQUE(session_id, race_number)
                ) STRICT",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE IF EXISTS session_races")
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum SessionRaces {
    Table,
    Id,
    SessionId,
    RaceNumber,
    TrackId,
    ChosenBy,
    CreatedAt,
}

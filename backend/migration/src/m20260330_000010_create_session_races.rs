use sea_orm_migration::prelude::*;

use crate::m20260330_000002_create_tracks::Tracks;
use crate::m20260330_000004_create_users::Users;
use crate::m20260330_000008_create_sessions::Sessions;

/// Creates the session_races table — each race within a session.
/// Tracks the sequence of tracks raced and who chose them.
/// Composite unique on (session_id, race_number).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SessionRaces::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SessionRaces::Id)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SessionRaces::SessionId).text().not_null())
                    .col(
                        ColumnDef::new(SessionRaces::RaceNumber)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SessionRaces::TrackId).integer().not_null())
                    .col(ColumnDef::new(SessionRaces::ChosenBy).text())
                    .col(ColumnDef::new(SessionRaces::CreatedAt).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(SessionRaces::Table, SessionRaces::SessionId)
                            .to(Sessions::Table, Sessions::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(SessionRaces::Table, SessionRaces::TrackId)
                            .to(Tracks::Table, Tracks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(SessionRaces::Table, SessionRaces::ChosenBy)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Composite unique index: no duplicate race numbers within a session
        manager
            .create_index(
                Index::create()
                    .name("idx_session_races_session_race_number")
                    .table(SessionRaces::Table)
                    .col(SessionRaces::SessionId)
                    .col(SessionRaces::RaceNumber)
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SessionRaces::Table).to_owned())
            .await
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

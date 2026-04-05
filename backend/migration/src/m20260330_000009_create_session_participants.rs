use sea_orm_migration::prelude::*;

use crate::m20260330_000004_create_users::Users;
use crate::m20260330_000008_create_sessions::Sessions;

/// Creates the session_participants table — tracks who is in a session
/// and when they joined/left. A user can rejoin (creating a new row).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SessionParticipants::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SessionParticipants::Id)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SessionParticipants::SessionId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionParticipants::UserId)
                            .text()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SessionParticipants::JoinedAt)
                            .text()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SessionParticipants::LeftAt).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(SessionParticipants::Table, SessionParticipants::SessionId)
                            .to(Sessions::Table, Sessions::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(SessionParticipants::Table, SessionParticipants::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SessionParticipants::Table).to_owned())
            .await
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

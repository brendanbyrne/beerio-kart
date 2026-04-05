use sea_orm_migration::prelude::*;

use crate::m20260330_000004_create_users::Users;

/// Creates the sessions table — the organizational unit for group play.
/// A session is like a lobby where players join, race tracks, and leave.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Sessions::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(Sessions::CreatedBy).text().not_null())
                    .col(ColumnDef::new(Sessions::HostId).text().not_null())
                    .col(ColumnDef::new(Sessions::Ruleset).text().not_null())
                    .col(ColumnDef::new(Sessions::LeastPlayedDrinkCategory).text())
                    .col(ColumnDef::new(Sessions::Status).text().not_null())
                    .col(ColumnDef::new(Sessions::CreatedAt).text().not_null())
                    .col(ColumnDef::new(Sessions::LastActivityAt).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Sessions::Table, Sessions::CreatedBy)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Sessions::Table, Sessions::HostId)
                            .to(Users::Table, Users::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Sessions::Table).to_owned())
            .await
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

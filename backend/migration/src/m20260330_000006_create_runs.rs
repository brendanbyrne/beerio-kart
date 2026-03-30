use sea_orm_migration::prelude::*;

use crate::m20260330_000001_create_base_tables::{Bodies, Characters, Gliders, Wheels};
use crate::m20260330_000002_create_tracks::Tracks;
use crate::m20260330_000003_create_drink_types::DrinkTypes;
use crate::m20260330_000004_create_users::Users;

/// Creates the runs table — the core table. One row per player per race attempt.
/// FKs to users, tracks, characters, bodies, wheels, gliders, drink_types.
/// Times stored as INTEGER milliseconds.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Runs::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Runs::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(Runs::UserId).text().not_null())
                    .col(ColumnDef::new(Runs::TrackId).integer().not_null())
                    .col(ColumnDef::new(Runs::CharacterId).integer().not_null())
                    .col(ColumnDef::new(Runs::BodyId).integer().not_null())
                    .col(ColumnDef::new(Runs::WheelsId).integer().not_null())
                    .col(ColumnDef::new(Runs::GliderId).integer().not_null())
                    .col(ColumnDef::new(Runs::TrackTime).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap1Time).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap2Time).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap3Time).integer().not_null())
                    .col(ColumnDef::new(Runs::DrinkTypeId).text().not_null())
                    .col(ColumnDef::new(Runs::PhotoPath).text())
                    .col(ColumnDef::new(Runs::CreatedAt).text().not_null())
                    .col(ColumnDef::new(Runs::Notes).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::UserId)
                            .to(Users::Table, Users::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::TrackId)
                            .to(Tracks::Table, Tracks::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::CharacterId)
                            .to(Characters::Table, Characters::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::BodyId)
                            .to(Bodies::Table, Bodies::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::WheelsId)
                            .to(Wheels::Table, Wheels::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::GliderId)
                            .to(Gliders::Table, Gliders::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Runs::Table, Runs::DrinkTypeId)
                            .to(DrinkTypes::Table, DrinkTypes::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Runs::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Runs {
    Table,
    Id,
    UserId,
    TrackId,
    CharacterId,
    BodyId,
    WheelsId,
    GliderId,
    TrackTime,
    Lap1Time,
    Lap2Time,
    Lap3Time,
    DrinkTypeId,
    PhotoPath,
    CreatedAt,
    Notes,
}

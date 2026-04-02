use sea_orm_migration::prelude::*;

use crate::m20260330_000001_create_base_tables::{Bodies, Characters, Gliders, Wheels};
use crate::m20260330_000003_create_drink_types::DrinkTypes;

/// Creates the users table with FKs to characters, bodies, wheels, gliders,
/// and drink_types. All preferred_* columns are nullable (new user hasn't
/// picked yet).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).text().not_null().primary_key())
                    .col(
                        ColumnDef::new(Users::Username)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Users::Email).text().unique_key())
                    .col(ColumnDef::new(Users::PasswordHash).text().not_null())
                    .col(ColumnDef::new(Users::PreferredCharacterId).integer())
                    .col(ColumnDef::new(Users::PreferredBodyId).integer())
                    .col(ColumnDef::new(Users::PreferredWheelId).integer())
                    .col(ColumnDef::new(Users::PreferredGliderId).integer())
                    .col(ColumnDef::new(Users::PreferredDrinkTypeId).text())
                    .col(
                        ColumnDef::new(Users::RefreshTokenVersion)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Users::CreatedAt).text().not_null())
                    .col(ColumnDef::new(Users::UpdatedAt).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Users::Table, Users::PreferredCharacterId)
                            .to(Characters::Table, Characters::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Users::Table, Users::PreferredBodyId)
                            .to(Bodies::Table, Bodies::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Users::Table, Users::PreferredWheelId)
                            .to(Wheels::Table, Wheels::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Users::Table, Users::PreferredGliderId)
                            .to(Gliders::Table, Gliders::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Users::Table, Users::PreferredDrinkTypeId)
                            .to(DrinkTypes::Table, DrinkTypes::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Users {
    Table,
    Id,
    Username,
    Email,
    PasswordHash,
    PreferredCharacterId,
    PreferredBodyId,
    PreferredWheelId,
    PreferredGliderId,
    PreferredDrinkTypeId,
    RefreshTokenVersion,
    CreatedAt,
    UpdatedAt,
}

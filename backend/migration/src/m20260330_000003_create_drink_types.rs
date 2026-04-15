use sea_orm_migration::prelude::*;

/// Creates the drink_types table WITHOUT the created_by column.
/// The created_by FK points to users, but users hasn't been created yet
/// (and users has a FK back to drink_types). We break the cycle by adding
/// created_by in a later migration after users exists.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(DrinkTypes::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DrinkTypes::Id)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(DrinkTypes::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(DrinkTypes::Alcoholic).boolean().not_null())
                    .col(ColumnDef::new(DrinkTypes::CreatedAt).date_time().not_null())
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DrinkTypes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum DrinkTypes {
    Table,
    Id,
    Name,
    Alcoholic,
    CreatedAt,
}

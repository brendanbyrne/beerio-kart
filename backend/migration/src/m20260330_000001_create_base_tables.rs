use sea_orm_migration::prelude::*;

/// Creates the five pre-seeded lookup tables that have no foreign keys:
/// characters, bodies, wheels, gliders, cups.
/// All use INTEGER primary keys since they hold static game data.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Characters
        manager
            .create_table(
                Table::create()
                    .table(Characters::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Characters::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Characters::Name)
                            .text()
                            .not_null()
                            .unique_key(),
                    )
                    .col(ColumnDef::new(Characters::ImagePath).text().not_null())
                    .to_owned(),
            )
            .await?;

        // Bodies
        manager
            .create_table(
                Table::create()
                    .table(Bodies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Bodies::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Bodies::Name).text().not_null().unique_key())
                    .col(ColumnDef::new(Bodies::ImagePath).text().not_null())
                    .to_owned(),
            )
            .await?;

        // Wheels
        manager
            .create_table(
                Table::create()
                    .table(Wheels::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Wheels::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Wheels::Name).text().not_null().unique_key())
                    .col(ColumnDef::new(Wheels::ImagePath).text().not_null())
                    .to_owned(),
            )
            .await?;

        // Gliders
        manager
            .create_table(
                Table::create()
                    .table(Gliders::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Gliders::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Gliders::Name).text().not_null().unique_key())
                    .col(ColumnDef::new(Gliders::ImagePath).text().not_null())
                    .to_owned(),
            )
            .await?;

        // Cups
        manager
            .create_table(
                Table::create()
                    .table(Cups::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Cups::Id).integer().not_null().primary_key())
                    .col(ColumnDef::new(Cups::Name).text().not_null().unique_key())
                    .col(ColumnDef::new(Cups::ImagePath).text().not_null())
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Cups::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Gliders::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Wheels::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Bodies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Characters::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
pub enum Characters {
    Table,
    Id,
    Name,
    ImagePath,
}

#[derive(DeriveIden)]
pub enum Bodies {
    Table,
    Id,
    Name,
    ImagePath,
}

#[derive(DeriveIden)]
pub enum Wheels {
    Table,
    Id,
    Name,
    ImagePath,
}

#[derive(DeriveIden)]
pub enum Gliders {
    Table,
    Id,
    Name,
    ImagePath,
}

#[derive(DeriveIden)]
pub enum Cups {
    Table,
    Id,
    Name,
    ImagePath,
}

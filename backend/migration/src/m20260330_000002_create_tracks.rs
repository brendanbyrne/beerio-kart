use sea_orm_migration::prelude::*;

use crate::m20260330_000001_create_base_tables::Cups;

/// Creates the tracks table with a foreign key to cups
/// and a composite unique constraint on (cup_id, position).
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Tracks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Tracks::Id)
                            .integer()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Tracks::Name).text().not_null().unique_key())
                    .col(ColumnDef::new(Tracks::CupId).integer().not_null())
                    .col(ColumnDef::new(Tracks::Position).integer().not_null())
                    .col(ColumnDef::new(Tracks::ImagePath).text().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Tracks::Table, Tracks::CupId)
                            .to(Cups::Table, Cups::Id),
                    )
                    .to_owned(),
            )
            .await?;

        // Composite unique: no two tracks in the same cup position
        manager
            .create_index(
                Index::create()
                    .name("idx_tracks_cup_position")
                    .table(Tracks::Table)
                    .col(Tracks::CupId)
                    .col(Tracks::Position)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Tracks::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
pub enum Tracks {
    Table,
    Id,
    Name,
    CupId,
    Position,
    ImagePath,
}

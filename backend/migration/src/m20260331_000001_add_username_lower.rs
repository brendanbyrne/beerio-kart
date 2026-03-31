use sea_orm_migration::prelude::*;

use crate::m20260330_000004_create_users::Users;

/// Adds a `username_lower` column for case-insensitive username uniqueness.
/// The original `username` keeps its inline UNIQUE constraint (exact-match
/// uniqueness), while `username_lower` gets its own unique index for
/// case-insensitive lookups. Both constraints are harmless to keep —
/// `username_lower` is the one that matters for deduplication.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Add username_lower column
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .add_column(
                        ColumnDef::new(Users::UsernameLower)
                            .text()
                            .not_null()
                            // Default to empty string so existing rows don't fail.
                            // The backfill below will set the real values.
                            .default(""),
                    )
                    .to_owned(),
            )
            .await?;

        // Backfill existing rows: set username_lower = lower(username)
        manager
            .get_connection()
            .execute_unprepared("UPDATE users SET username_lower = lower(username)")
            .await?;

        // SQLite doesn't support adding UNIQUE via ALTER TABLE, so create an index
        manager
            .create_index(
                Index::create()
                    .name("idx_users_username_lower_unique")
                    .table(Users::Table)
                    .col(Users::UsernameLower)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop the username_lower unique index
        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_username_lower_unique")
                    .table(Users::Table)
                    .to_owned(),
            )
            .await?;

        // Remove column
        manager
            .alter_table(
                Table::alter()
                    .table(Users::Table)
                    .drop_column(Users::UsernameLower)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

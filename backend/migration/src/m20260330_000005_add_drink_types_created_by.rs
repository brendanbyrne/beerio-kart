use sea_orm_migration::prelude::*;

/// Adds the created_by column to drink_types, completing the circular
/// dependency: users.preferred_drink_type_id → drink_types and
/// drink_types.created_by → users. We broke the cycle by creating
/// drink_types first (without created_by), then users, then adding
/// this column.
///
/// Uses raw SQL because SQLite's ALTER TABLE ADD COLUMN supports
/// inline REFERENCES but SeaORM's schema builder doesn't reliably
/// emit it for ALTER TABLE on SQLite.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE drink_types ADD COLUMN created_by TEXT REFERENCES users(id)",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // SQLite 3.35.0+ supports DROP COLUMN
        manager
            .get_connection()
            .execute_unprepared("ALTER TABLE drink_types DROP COLUMN created_by")
            .await?;
        Ok(())
    }
}

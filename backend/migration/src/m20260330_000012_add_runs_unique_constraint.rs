use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Prevent duplicate run submissions: one run per user per race
        manager
            .create_index(
                Index::create()
                    .name("idx_runs_session_race_user_unique")
                    .table(Alias::new("runs"))
                    .col(Alias::new("session_race_id"))
                    .col(Alias::new("user_id"))
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_runs_session_race_user_unique")
                    .to_owned(),
            )
            .await
    }
}

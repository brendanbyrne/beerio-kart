use sea_orm_migration::prelude::*;

use crate::m20260330_000001_create_base_tables::{Bodies, Characters, Gliders, Wheels};
use crate::m20260330_000002_create_tracks::Tracks;
use crate::m20260330_000003_create_drink_types::DrinkTypes;
use crate::m20260330_000004_create_users::Users;
use crate::m20260330_000006_create_runs::Runs;
use crate::m20260330_000010_create_session_races::SessionRaces;

/// Drops and recreates the runs table with new columns: session_race_id and
/// disqualified. Also drops and recreates run_flags since it has a FK to runs.
/// Approved by Brendan — no production data to preserve.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop run_flags first (FK dependency on runs)
        manager
            .drop_table(Table::drop().table(RunFlags::Table).if_exists().to_owned())
            .await?;

        // Drop old runs table
        manager
            .drop_table(Table::drop().table(Runs::Table).if_exists().to_owned())
            .await?;

        // Recreate runs with new columns
        manager
            .create_table(
                Table::create()
                    .table(Runs::Table)
                    .col(ColumnDef::new(Runs::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(Runs::UserId).text().not_null())
                    .col(ColumnDef::new(Runs::SessionRaceId).text().not_null())
                    .col(ColumnDef::new(Runs::TrackId).integer().not_null())
                    .col(ColumnDef::new(Runs::CharacterId).integer().not_null())
                    .col(ColumnDef::new(Runs::BodyId).integer().not_null())
                    .col(ColumnDef::new(Runs::WheelId).integer().not_null())
                    .col(ColumnDef::new(Runs::GliderId).integer().not_null())
                    .col(ColumnDef::new(Runs::TrackTime).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap1Time).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap2Time).integer().not_null())
                    .col(ColumnDef::new(Runs::Lap3Time).integer().not_null())
                    .col(ColumnDef::new(Runs::DrinkTypeId).text().not_null())
                    .col(
                        ColumnDef::new(Runs::Disqualified)
                            .integer()
                            .not_null()
                            .default(0),
                    )
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
                            .from(Runs::Table, Runs::SessionRaceId)
                            .to(SessionRaces::Table, SessionRaces::Id),
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
                            .from(Runs::Table, Runs::WheelId)
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
            .await?;

        // Recreate run_flags
        manager
            .create_table(
                Table::create()
                    .table(RunFlags::Table)
                    .col(ColumnDef::new(RunFlags::Id).text().not_null().primary_key())
                    .col(ColumnDef::new(RunFlags::RunId).text().not_null())
                    .col(ColumnDef::new(RunFlags::Reason).text().not_null())
                    .col(ColumnDef::new(RunFlags::Note).text())
                    .col(
                        ColumnDef::new(RunFlags::HideWhilePending)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(RunFlags::AutoGenerated)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(ColumnDef::new(RunFlags::CreatedAt).text().not_null())
                    .col(ColumnDef::new(RunFlags::ResolvedAt).text())
                    .foreign_key(
                        ForeignKey::create()
                            .from(RunFlags::Table, RunFlags::RunId)
                            .to(Runs::Table, Runs::Id),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Down migration not needed — this is a destructive wipe approved
        // during pre-production. The original migrations (006, 007) handle
        // the original schema.
        Ok(())
    }
}

// Re-use Runs iden from migration 006 (it has the new SessionRaceId and
// Disqualified variants added there for this migration to reference).

#[derive(DeriveIden)]
enum RunFlags {
    Table,
    Id,
    RunId,
    Reason,
    Note,
    HideWhilePending,
    AutoGenerated,
    CreatedAt,
    ResolvedAt,
}

// PR-A1 scaffolding: rust-2018 idioms cleanup deferred to Phase H per
// rust.md § 8 ("config and clearing land in separate PRs").
#![allow(elided_lifetimes_in_paths)]
// The `Iden` enums below are mechanical SeaORM DDL scaffolding — each
// variant is a column name, so the variant name itself is the
// documentation. `rust.md` § 6 doesn't earn its keep here; opt out.
#![allow(missing_docs)]

use sea_orm_migration::prelude::*;

/// Consolidated initial schema for the prelaunch period.
///
/// Per CLAUDE.md → "Schema changes (prelaunch)", all schema lives in this single
/// file. New tables, columns, indexes, and constraints edit this migration
/// in place rather than appending a new one. Resetting the dev DB recreates
/// the schema from scratch on next boot.
///
/// Notes on construction:
/// - Tables are created in dependency order so foreign keys resolve.
/// - There is a circular FK between `users.preferred_drink_type_id` and
///   `drink_types.created_by`. We break the cycle by creating `drink_types`
///   first without `created_by`, creating `users`, then adding `created_by`
///   via raw `ALTER TABLE` (`SQLite` supports inline REFERENCES on ADD COLUMN
///   but `SeaORM`'s builder doesn't reliably emit it).
/// - Timestamp columns use `SeaORM`'s `.date_time()` which emits the literal
///   SQL type `datetime_text`. `SeaORM`'s codegen recognizes that type and
///   maps the column to `chrono::NaiveDateTime` when entities are regenerated.
///   The session/race/run tables are written as raw SQL (matching the prior
///   pattern) and use the same `datetime_text` type literally for consistency.
/// - The partial unique index on `session_participants(user_id) WHERE left_at
///   IS NULL` is created via raw SQL because `SeaORM`'s builder doesn't support
///   partial-index `WHERE` clauses.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // ---- Lookup tables (no FKs) ----

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

        // ---- Tracks (FK to cups) ----

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

        // ---- Drink types (created without created_by — see header comment) ----

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
            .await?;

        // ---- Users (FKs to lookup tables and drink_types) ----

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
                    .col(ColumnDef::new(Users::CreatedAt).date_time().not_null())
                    .col(ColumnDef::new(Users::UpdatedAt).date_time().not_null())
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
            .await?;

        // Now that users exists, close the cycle by adding drink_types.created_by.
        manager
            .get_connection()
            .execute_unprepared(
                "ALTER TABLE drink_types ADD COLUMN created_by TEXT REFERENCES users(id)",
            )
            .await?;

        // ---- Sessions (raw SQL, matching prior pattern) ----

        let conn = manager.get_connection();

        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT NOT NULL PRIMARY KEY,
                    host_id TEXT NOT NULL REFERENCES users(id),
                    ruleset TEXT NOT NULL,
                    least_played_drink_category TEXT,
                    status TEXT NOT NULL,
                    created_at datetime_text NOT NULL
                )",
        )
        .await?;

        // Supports the race-derived stale-session sweeper (ADR-0035):
        // `close_stale_sessions` filters `status = 'active' AND created_at <
        // cutoff`. The session has no maintained activity column anymore —
        // liveness is derived from `session_races.created_at`.
        conn.execute_unprepared(
            "CREATE INDEX idx_sessions_status_created_at
                 ON sessions(status, created_at)",
        )
        .await?;

        // ---- Session participants ----
        //
        // One row per (session, user). Leave/rejoin mutates this row rather
        // than appending a new one — `joined_at` is the start of the current
        // presence segment, `left_at` is null while present. Per-race presence
        // is captured by `session_race_participations` (below) at race-creation
        // time, not derived from this table.

        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS session_participants (
                    id TEXT NOT NULL PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    user_id TEXT NOT NULL REFERENCES users(id),
                    joined_at datetime_text NOT NULL,
                    left_at datetime_text,
                    UNIQUE(session_id, user_id)
                )",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE INDEX idx_session_participants_session_active
                 ON session_participants(session_id, left_at)",
        )
        .await?;

        // Partial unique index: a user can only be active in one session
        // at a time. Raw SQL because SeaORM's builder doesn't support
        // partial-index WHERE clauses.
        conn.execute_unprepared(
            "CREATE UNIQUE INDEX idx_session_participants_one_active_session
                 ON session_participants(user_id) WHERE left_at IS NULL",
        )
        .await?;

        // ---- Session races ----

        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS session_races (
                    id TEXT NOT NULL PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(id),
                    race_number INTEGER NOT NULL,
                    track_id INTEGER NOT NULL REFERENCES tracks(id),
                    chosen_by TEXT REFERENCES users(id),
                    created_at datetime_text NOT NULL,
                    UNIQUE(session_id, race_number)
                )",
        )
        .await?;

        // Backs the race-derived liveness subqueries (ADR-0035): the
        // `EXISTS` in `get_active_session_id` and the `NOT EXISTS` in
        // `close_stale_sessions` both correlate on
        // `session_id = ? AND created_at >= ?`. The `UNIQUE(session_id,
        // race_number)` index above can seek `session_id` but leaves the
        // `created_at` filter as a residual scan; this index covers both.
        // Also backs the `list_active_sessions` JOIN on `session_id`.
        conn.execute_unprepared(
            "CREATE INDEX idx_session_races_session_created
                 ON session_races(session_id, created_at)",
        )
        .await?;

        // ---- Session race participations ----
        //
        // One row per (race, user) for every user present at race-creation
        // time. The row is the proof of "this user was present when this race
        // was created" — pending-state derivation reads from here, not from a
        // walk of `session_participants` history. `skipped_at` flips when the
        // user explicitly forfeits the race.
        //
        // ON DELETE CASCADE on session_race_id is required: `skip_turn`
        // deletes-and-replaces the current race, and the old participations
        // need to evaporate atomically with the race delete.
        //
        // user_id intentionally does NOT cascade — docs/design.md guarantees these
        // rows are never deleted (they're the audit trail of "who was present
        // when"). If a user-deletion path is ever added, it must explicitly
        // decide what to do with these rows rather than letting them silently
        // vanish.

        conn.execute_unprepared(
            "CREATE TABLE IF NOT EXISTS session_race_participations (
                    session_race_id TEXT NOT NULL REFERENCES session_races(id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(id),
                    created_at datetime_text NOT NULL,
                    skipped_at datetime_text,
                    PRIMARY KEY (session_race_id, user_id)
                )",
        )
        .await?;

        conn.execute_unprepared(
            "CREATE INDEX idx_session_race_participations_user
                 ON session_race_participations(user_id)",
        )
        .await?;

        // ---- Runs ----

        conn.execute_unprepared(
            "CREATE TABLE runs (
                id TEXT NOT NULL PRIMARY KEY,
                user_id TEXT NOT NULL REFERENCES users(id),
                session_race_id TEXT NOT NULL REFERENCES session_races(id),
                track_id INTEGER NOT NULL REFERENCES tracks(id),
                character_id INTEGER NOT NULL REFERENCES characters(id),
                body_id INTEGER NOT NULL REFERENCES bodies(id),
                wheel_id INTEGER NOT NULL REFERENCES wheels(id),
                glider_id INTEGER NOT NULL REFERENCES gliders(id),
                track_time INTEGER NOT NULL,
                lap1_time INTEGER NOT NULL,
                lap2_time INTEGER NOT NULL,
                lap3_time INTEGER NOT NULL,
                drink_type_id TEXT NOT NULL REFERENCES drink_types(id),
                disqualified BOOLEAN NOT NULL DEFAULT 0,
                photo_path TEXT,
                created_at datetime_text NOT NULL,
                notes TEXT
            )",
        )
        .await?;

        // ---- Run flags ----

        conn.execute_unprepared(
            "CREATE TABLE run_flags (
                id TEXT NOT NULL PRIMARY KEY,
                run_id TEXT NOT NULL REFERENCES runs(id),
                reason TEXT NOT NULL,
                note TEXT,
                hide_while_pending BOOLEAN NOT NULL DEFAULT 0,
                auto_generated BOOLEAN NOT NULL DEFAULT 0,
                created_at datetime_text NOT NULL,
                resolved_at datetime_text
            )",
        )
        .await?;

        // Prevent duplicate run submissions: one run per user per race.
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
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse FK dependency order so FKs don't block drops.
        let conn = manager.get_connection();

        conn.execute_unprepared("DROP TABLE IF EXISTS run_flags")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS runs").await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS session_race_participations")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS session_races")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS session_participants")
            .await?;
        conn.execute_unprepared("DROP TABLE IF EXISTS sessions")
            .await?;

        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DrinkTypes::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Tracks::Table).to_owned())
            .await?;
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

#[derive(DeriveIden)]
pub enum Tracks {
    Table,
    Id,
    Name,
    CupId,
    Position,
    ImagePath,
}

#[derive(DeriveIden)]
pub enum DrinkTypes {
    Table,
    Id,
    Name,
    Alcoholic,
    CreatedAt,
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

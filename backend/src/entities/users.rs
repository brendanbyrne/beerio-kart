//! Hand-written entity for `users`.
//!
//! Two relations to `session_races` are conceptually possible:
//! 1. **Participation** (M2M) — a user races in many `session_races` via the
//!    `session_race_participations` junction. This is what `Related<session_races>`
//!    resolves to below.
//! 2. **Choice** (1:N via `session_races.chosen_by`) — the user who picked the
//!    track for a given race. The FK column still exists, but no caller queries
//!    that direction through the entity layer, so no `Relation` variant is
//!    declared here. Add one (e.g. `ChosenSessionRaces`) if a future caller
//!    needs `find_related` on the choice direction; the M2M would then need
//!    to keep its name distinct from any direct variant pointing at the same
//!    target — see `seaorm.md` § 11.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text", unique)]
    pub username: String,
    #[sea_orm(column_type = "Text", nullable, unique)]
    pub email: Option<String>,
    #[sea_orm(column_type = "Text")]
    pub password_hash: String,
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
    #[sea_orm(column_type = "Text", nullable)]
    pub preferred_drink_type_id: Option<String>,
    pub refresh_token_version: i32,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::bodies::Entity",
        from = "Column::PreferredBodyId",
        to = "super::bodies::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Bodies,
    #[sea_orm(
        belongs_to = "super::characters::Entity",
        from = "Column::PreferredCharacterId",
        to = "super::characters::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Characters,
    #[sea_orm(
        belongs_to = "super::drink_types::Entity",
        from = "Column::PreferredDrinkTypeId",
        to = "super::drink_types::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    DrinkTypes,
    #[sea_orm(
        belongs_to = "super::gliders::Entity",
        from = "Column::PreferredGliderId",
        to = "super::gliders::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Gliders,
    #[sea_orm(has_many = "super::runs::Entity")]
    Runs,
    // `has_many` (not `has_one`): a user accumulates one active and many
    // historical participation rows over the lifetime of a session — the
    // partial unique index on `user_id WHERE left_at IS NULL` only constrains
    // active rows, not the full set.
    #[sea_orm(has_many = "super::session_participants::Entity")]
    SessionParticipants,
    #[sea_orm(has_many = "super::session_race_participations::Entity")]
    SessionRaceParticipations,
    #[sea_orm(has_many = "super::sessions::Entity")]
    Sessions,
    #[sea_orm(
        belongs_to = "super::wheels::Entity",
        from = "Column::PreferredWheelId",
        to = "super::wheels::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Wheels,
}

impl Related<super::bodies::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bodies.def()
    }
}

impl Related<super::characters::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Characters.def()
    }
}

impl Related<super::drink_types::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::DrinkTypes.def()
    }
}

impl Related<super::gliders::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Gliders.def()
    }
}

impl Related<super::runs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Runs.def()
    }
}

impl Related<super::session_participants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionParticipants.def()
    }
}

impl Related<super::session_race_participations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionRaceParticipations.def()
    }
}

impl Related<super::sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

impl Related<super::wheels::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Wheels.def()
    }
}

// Many-to-many: users ↔ session_races through `session_race_participations`.
// `to()` is the junction → far-side leg (`SessionRaces`); `via()` is the
// reverse of the near-side leg (junction's `Users` belongs_to, reversed so
// SeaORM walks user → junction).
impl Related<super::session_races::Entity> for Entity {
    fn to() -> RelationDef {
        super::session_race_participations::Relation::SessionRaces.def()
    }
    fn via() -> Option<RelationDef> {
        Some(
            super::session_race_participations::Relation::Users
                .def()
                .rev(),
        )
    }
}

impl ActiveModelBehavior for ActiveModel {}

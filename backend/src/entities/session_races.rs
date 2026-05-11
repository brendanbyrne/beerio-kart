//! Hand-written entity for `session_races`.
//!
//! `chosen_by` is the user who picked this race's track. It exists as a
//! nullable FK column (preserved for audit/UX), but no caller currently
//! traverses that direction through `SeaORM`, so no `Relation` variant is
//! declared for it. Adding one (e.g. `Chooser` with `belongs_to = users`)
//! would require keeping its name distinct from any direct variant on the
//! `users` side pointing here — see `seaorm.md` § 11.
//!
//! `Related<users>` resolves to the M2M "users who participated in this
//! race" through `session_race_participations`, *not* to the chooser.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "session_races")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub session_id: String,
    pub race_number: i32,
    pub track_id: i32,
    #[sea_orm(column_type = "Text", nullable)]
    pub chosen_by: Option<String>,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::runs::Entity")]
    Runs,
    #[sea_orm(has_many = "super::session_race_participations::Entity")]
    SessionRaceParticipations,
    #[sea_orm(
        belongs_to = "super::sessions::Entity",
        from = "Column::SessionId",
        to = "super::sessions::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Sessions,
    #[sea_orm(
        belongs_to = "super::tracks::Entity",
        from = "Column::TrackId",
        to = "super::tracks::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Tracks,
}

impl Related<super::runs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Runs.def()
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

impl Related<super::tracks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tracks.def()
    }
}

// Many-to-many: session_races ↔ users through `session_race_participations`.
// `to()` is the junction → far-side leg (`Users`); `via()` is the reverse of
// the near-side leg (junction's `SessionRaces` belongs_to, reversed so
// SeaORM walks session_race → junction).
impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        super::session_race_participations::Relation::Users.def()
    }
    fn via() -> Option<RelationDef> {
        Some(
            super::session_race_participations::Relation::SessionRaces
                .def()
                .rev(),
        )
    }
}

impl ActiveModelBehavior for ActiveModel {}

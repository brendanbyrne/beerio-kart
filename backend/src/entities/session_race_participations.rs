//! Hand-written entity for `session_race_participations`.
//!
//! Composite primary key `(session_race_id, user_id)` — one row per (race, user)
//! captured at race-creation time. Existence proves presence; `skipped_at`
//! flips when the user explicitly forfeits the race.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "session_race_participations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub session_race_id: String,
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub user_id: String,
    pub created_at: DateTime,
    pub skipped_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::session_races::Entity",
        from = "Column::SessionRaceId",
        to = "super::session_races::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    SessionRaces,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::session_races::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionRaces.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

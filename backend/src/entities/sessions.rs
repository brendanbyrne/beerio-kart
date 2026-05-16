use sea_orm::entity::prelude::*;

use crate::domain::enums::{DrinkCategory, SessionRuleset, SessionStatus};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub host_id: String,
    pub ruleset: SessionRuleset,
    pub least_played_drink_category: Option<DrinkCategory>,
    pub status: SessionStatus,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::session_participants::Entity")]
    SessionParticipants,
    #[sea_orm(has_many = "super::session_races::Entity")]
    SessionRaces,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::HostId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::session_participants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::SessionParticipants.def()
    }
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

// `ActiveModelBehavior` for this entity lives in the sibling
// `sessions_behavior.rs` (it stamps `created_at`).

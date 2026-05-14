use sea_orm::entity::prelude::*;

use crate::domain::enums::RunFlagReason;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "run_flags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub run_id: String,
    pub reason: RunFlagReason,
    #[sea_orm(column_type = "Text", nullable)]
    pub note: Option<String>,
    pub hide_while_pending: bool,
    pub auto_generated: bool,
    pub created_at: DateTime,
    pub resolved_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::runs::Entity",
        from = "Column::RunId",
        to = "super::runs::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Runs,
}

impl Related<super::runs::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Runs.def()
    }
}

// `ActiveModelBehavior` for this entity lives in the sibling
// `run_flags_behavior.rs` (it stamps `created_at`).

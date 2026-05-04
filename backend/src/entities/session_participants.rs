//! Hand-written entity for `session_participants`.
//!
//! `user_id` deliberately carries no `unique` attribute. The migration
//! enforces uniqueness only over *active* participation rows
//! (`CREATE UNIQUE INDEX ... ON session_participants(user_id) WHERE left_at IS NULL`),
//! which lets a user re-join the same session — each rejoin produces a new
//! row, with the previous one stamped via `left_at`. Marking the column
//! `unique` here would imply a full constraint and would also coerce the
//! `users` ↔ `session_participants` relation to `has_one`, breaking
//! historical-row loading.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "session_participants")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub session_id: String,
    #[sea_orm(column_type = "Text")]
    pub user_id: String,
    pub joined_at: DateTime,
    pub left_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sessions::Entity",
        from = "Column::SessionId",
        to = "super::sessions::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Sessions,
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "NoAction"
    )]
    Users,
}

impl Related<super::sessions::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Sessions.def()
    }
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}

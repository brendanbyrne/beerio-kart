//! Hand-written entity for `session_participants`.
//!
//! `user_id` deliberately carries no `unique` attribute. The migration
//! defines two separate uniqueness constraints on this table:
//!
//! - `UNIQUE(session_id, user_id)` — at most one row per (session, user).
//!   Rejoining the same session mutates the existing row (clears `left_at`)
//!   rather than inserting a new one; see `services/sessions.rs::join_session`.
//! - `CREATE UNIQUE INDEX ... ON session_participants(user_id) WHERE left_at IS NULL`
//!   — a user can be active in at most one session at a time globally.
//!
//! Neither makes `user_id` standalone-unique on the table. `has_many` on the
//! `users` ↔ `session_participants` relation reflects that a user accumulates
//! one row per session they've ever joined (many rows across many sessions),
//! even though within any single session there's only ever one row per user.
//! Marking `user_id` `unique` here would coerce SeaORM into `has_one` and
//! break loading of a user's full participation history.

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

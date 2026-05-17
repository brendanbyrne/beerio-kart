//! Hand-written entity for `notifications` (ADR-0038).
//!
//! Per-user inbox of asynchronous events. `kind` is the discriminator
//! (`snake_case`, lifted out of the JSON for indexing); `payload` is the
//! kind-specific structured body, stored as JSON TEXT — the serde-tagged
//! [`NotificationPayload`] enum on the Rust side. `read_at` is NULL until
//! the user dismisses the notification.
//!
//! [`NotificationPayload`]: crate::services::notifications::NotificationPayload

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "notifications")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub user_id: String,
    #[sea_orm(column_type = "Text")]
    pub kind: String,
    #[sea_orm(column_type = "Text")]
    pub payload: String,
    pub created_at: DateTime,
    pub read_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::UserId",
        to = "super::users::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Users,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

// `ActiveModelBehavior` for this entity lives in the sibling
// `notifications_behavior.rs` (it stamps `created_at`).

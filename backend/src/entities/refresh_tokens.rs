//! Hand-written entity for `refresh_tokens` (ADR-0040).
//!
//! One row per issued refresh token, keyed by its `jti` (`id`). Implements
//! refresh-token rotation with reuse detection: each login starts a
//! `family_id`; each refresh mints a successor row in the same family and
//! stamps the predecessor's `used_at`. `used_at IS NULL` is the live tip of a
//! family; a set `used_at` means the token was rotated away from. Presenting a
//! used token past the grace window is reuse and revokes the whole family.

use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "refresh_tokens")]
pub struct Model {
    /// The token's `jti` (UUID as TEXT, ADR-0027).
    #[sea_orm(primary_key, auto_increment = false, column_type = "Text")]
    pub id: String,
    #[sea_orm(column_type = "Text")]
    pub user_id: String,
    /// Identifies the chain of tokens descended from one login.
    #[sea_orm(column_type = "Text")]
    pub family_id: String,
    /// `None` = live tip; `Some` = the instant this token was rotated away from.
    pub used_at: Option<DateTime>,
    pub expires_at: DateTime,
    pub created_at: DateTime,
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
// `refresh_tokens_behavior.rs` (it stamps `created_at`).

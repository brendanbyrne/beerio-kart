//! `ActiveModelBehavior` for [`super::sessions`].
//!
//! Stamps `created_at` on insert. The session carries no maintained activity
//! column — liveness is derived from `session_races.created_at` (ADR-0035),
//! so there is nothing for `before_save` to advance on update.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelBehavior, ConnectionTrait, DbErr, Set};

use super::sessions::ActiveModel;

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        if insert {
            self.created_at = Set(Utc::now().naive_utc());
        }
        Ok(self)
    }
}

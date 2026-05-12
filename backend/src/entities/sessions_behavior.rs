//! `ActiveModelBehavior` for [`super::sessions`].
//!
//! Stamps `created_at` on insert. `last_activity_at` is *not* touched here —
//! it's an application-managed activity timestamp, advanced explicitly by
//! `services::helpers::touch_session` when something happens in the session
//! (a join, a race transition, etc.). Treating it as a generic `updated_at`
//! would advance it on every write, which would defeat the stale-session
//! cleanup loop. See `docs/coding-standards/seaorm.md` § 1.

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

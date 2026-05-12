//! `ActiveModelBehavior` for [`super::session_race_participations`]. Stamps
//! `created_at` on insert. `skipped_at` is set explicitly by the skip path,
//! not here. See `docs/coding-standards/seaorm.md` § 1.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelBehavior, ConnectionTrait, DbErr, Set};

use super::session_race_participations::ActiveModel;

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

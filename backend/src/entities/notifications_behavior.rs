//! `ActiveModelBehavior` for [`super::notifications`]. Stamps `created_at` on
//! insert. `read_at` is set explicitly by the mark-as-read path, not here.
//! See `docs/coding-standards/seaorm.md` § 1.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelBehavior, ConnectionTrait, DbErr, Set};

use super::notifications::ActiveModel;

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

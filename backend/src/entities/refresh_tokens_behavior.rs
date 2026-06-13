//! `ActiveModelBehavior` for [`super::refresh_tokens`]. Stamps `created_at` on
//! insert. `used_at` and `expires_at` are set explicitly by the rotation path,
//! not here. See `docs/coding-standards/seaorm.md` § 1.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelBehavior, ConnectionTrait, DbErr, Set};

use super::refresh_tokens::ActiveModel;

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

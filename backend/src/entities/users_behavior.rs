//! `ActiveModelBehavior` for [`super::users`].
//!
//! Centralizes `created_at` / `updated_at` maintenance so service code never
//! sets these timestamps by hand. `before_save` runs on insert and update;
//! `created_at` is only stamped on insert, `updated_at` on both. See
//! `docs/coding-standards/seaorm.md` § 1.

use async_trait::async_trait;
use chrono::Utc;
use sea_orm::{ActiveModelBehavior, ConnectionTrait, DbErr, Set};

use super::users::ActiveModel;

#[async_trait]
impl ActiveModelBehavior for ActiveModel {
    async fn before_save<C>(mut self, _db: &C, insert: bool) -> Result<Self, DbErr>
    where
        C: ConnectionTrait,
    {
        let now = Utc::now().naive_utc();
        if insert {
            self.created_at = Set(now);
        }
        self.updated_at = Set(now);
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    //! Verifies `before_save` for `users`. The other `*_behavior.rs` files
    //! share the same shape (stamp `created_at` on insert), differing only in
    //! which column they touch — exercising the pattern once here is enough.

    use std::time::Duration;

    use sea_orm::{ActiveModelTrait, EntityTrait, IntoActiveModel, Set};

    use crate::{
        entities::users,
        test_helpers::{create_user, setup_db},
    };

    #[tokio::test]
    async fn test_before_save_stamps_created_at_and_updated_at_on_insert() {
        // `create_user` constructs a `users::ActiveModel` with `created_at`
        // and `updated_at` left as `NotSet`; `before_save` must populate both.
        let db = setup_db().await;
        let user_id = create_user(&db, "alice").await;

        let user = users::Entity::find_by_id(user_id.as_str())
            .one(&db)
            .await
            .expect("query user")
            .expect("user exists");

        // On a fresh insert the two timestamps were stamped in the same
        // `before_save` call, so they must match exactly.
        assert_eq!(user.created_at, user.updated_at);
        let now = chrono::Utc::now().naive_utc();
        let drift = (now - user.created_at).num_seconds().abs();
        assert!(drift < 5, "stamped time should be ~now (drift {drift}s)");
    }

    #[tokio::test]
    async fn test_before_save_advances_updated_at_on_update() {
        let db = setup_db().await;
        let user_id = create_user(&db, "bob").await;

        let user_t0 = users::Entity::find_by_id(user_id.as_str())
            .one(&db)
            .await
            .expect("query user")
            .expect("user exists");
        let created_t0 = user_t0.created_at;
        let updated_t0 = user_t0.updated_at;

        // Sleep past `NaiveDateTime`'s microsecond resolution so the second
        // `before_save` call lands on a strictly later timestamp.
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Mutate any non-timestamp column. `before_save` runs on update too.
        let mut active: users::ActiveModel = user_t0.into_active_model();
        active.refresh_token_version = Set(1);
        active.update(&db).await.expect("update user");

        let user_t1 = users::Entity::find_by_id(user_id.as_str())
            .one(&db)
            .await
            .expect("query user")
            .expect("user exists");

        assert_eq!(
            user_t1.created_at, created_t0,
            "created_at must not change on update"
        );
        assert!(
            user_t1.updated_at > updated_t0,
            "updated_at must advance on update (t0={updated_t0}, t1={})",
            user_t1.updated_at
        );
    }
}

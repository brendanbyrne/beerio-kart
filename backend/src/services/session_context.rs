//! Loaded-and-validated session bundle.
//!
//! Aggregates the helpers from `services::helpers` so service functions that
//! start by loading an active session, checking host/participant, and touching
//! activity can read top-to-bottom without repeating the ceremony.

use sea_orm::ConnectionTrait;

use crate::entities::{session_participants, sessions};
use crate::error::AppError;
use crate::services::helpers;

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub session: sessions::Model,
}

impl SessionContext {
    /// Load the session by ID and require it to be in the `Active` state.
    pub async fn load_active<C: ConnectionTrait>(
        db: &C,
        session_id: &str,
    ) -> Result<Self, AppError> {
        let session = helpers::load_active_session(db, session_id).await?;
        Ok(Self { session })
    }

    /// Require that `user_id` is the host of this session.
    pub fn require_host(&self, user_id: &str) -> Result<(), AppError> {
        if self.session.host_id != user_id {
            return Err(AppError::Forbidden("Only the host can do that".into()));
        }
        Ok(())
    }

    /// Require that `user_id` is an active participant and return their row.
    pub async fn require_participant<C: ConnectionTrait>(
        &self,
        db: &C,
        user_id: &str,
    ) -> Result<session_participants::Model, AppError> {
        helpers::require_active_participant(db, &self.session.id, user_id).await
    }

    /// Bump `last_activity_at` to now in both the DB and this struct.
    /// Delegates the UPDATE to `helpers::touch_session`, then refreshes
    /// the in-memory field so callers that read it post-touch see the
    /// updated value.
    pub async fn touch<C: ConnectionTrait>(&mut self, db: &C) -> Result<(), AppError> {
        helpers::touch_session(db, &self.session.id).await?;
        self.session.last_activity_at = chrono::Utc::now().naive_utc();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::EntityTrait;

    use crate::test_helpers::{create_user, insert_participant, insert_session, setup_db};

    #[tokio::test]
    async fn load_active_populates_session() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;

        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();
        assert_eq!(ctx.session.id, session_id);
        assert_eq!(ctx.session.host_id, host);
    }

    #[tokio::test]
    async fn load_active_missing_propagates_not_found() {
        let db = setup_db().await;
        let err = SessionContext::load_active(&db, "missing")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn load_active_closed_propagates_conflict() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "closed").await;

        let err = SessionContext::load_active(&db, &session_id)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
    }

    #[tokio::test]
    async fn require_host_matches() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        ctx.require_host(&host).expect("host matches");
    }

    #[tokio::test]
    async fn require_host_mismatch_is_forbidden() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let other = create_user(&db, "other").await;
        let session_id = insert_session(&db, &host, "active").await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        let err = ctx.require_host(&other).unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn require_participant_happy_and_forbidden_paths() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;
        insert_participant(&db, &session_id, &host, None).await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        // Happy path: host is a participant.
        let row = ctx.require_participant(&db, &host).await.unwrap();
        assert_eq!(row.user_id, host);

        // Forbidden path: another user isn't.
        let outsider = create_user(&db, "outsider").await;
        let err = ctx.require_participant(&db, &outsider).await.unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn touch_bumps_last_activity_at_in_db_and_struct() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;
        let mut ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        let before = ctx.session.last_activity_at;
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        ctx.touch(&db).await.unwrap();

        // In-memory field is updated.
        assert!(
            ctx.session.last_activity_at > before,
            "struct field should advance: before={before}, after={}",
            ctx.session.last_activity_at
        );

        // DB is also updated.
        let db_value = sessions::Entity::find_by_id(&session_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .last_activity_at;
        assert!(
            db_value > before,
            "DB should advance: before={before}, after={db_value}"
        );
    }
}

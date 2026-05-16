//! Loaded-and-validated session bundle.
//!
//! Aggregates the helpers from `services::helpers` so service functions that
//! start by loading an active session and checking host/participant can read
//! top-to-bottom without repeating the ceremony.

use sea_orm::ConnectionTrait;

use crate::{
    domain::{SessionId, UserId},
    entities::{session_participants, sessions},
    error::Error,
    services::helpers,
};

/// A loaded session plus its IDs in typed form.
///
/// `session_id` mirrors `session.id` and `host_id` mirrors `session.host_id`
/// — both parsed once in [`load_active`](SessionContext::load_active) so
/// helper methods can borrow them instead of reparsing on every call. The
/// entity-side `String` fields stay authoritative for serialization; the
/// typed copies are what callers reach for when they need a `&SessionId` /
/// `&UserId`.
#[derive(Debug, Clone)]
pub struct SessionContext {
    /// The loaded session row, authoritative for all field access.
    pub session: sessions::Model,
    /// Typed copy of `session.id` for borrow-friendly access.
    pub session_id: SessionId,
    /// Typed copy of `session.host_id` for borrow-friendly access.
    pub host_id: UserId,
}

impl SessionContext {
    /// Load the session by ID and require it to be in the `Active` state.
    ///
    /// # Errors
    ///
    /// Propagates the errors of [`helpers::load_active_session`]: `NotFound`
    /// if the session doesn't exist, `Conflict` if it's not active.
    /// Returns `Internal` if the stored `session.id` or `session.host_id` is
    /// not a valid UUID — both are FK-protected, so this only fires on data
    /// corruption.
    #[tracing::instrument(level = "debug", skip(db), fields(session_id = %session_id))]
    pub async fn load_active<C: ConnectionTrait>(
        db: &C,
        session_id: &SessionId,
    ) -> Result<Self, Error> {
        let session = helpers::load_active_session(db, session_id).await?;
        let session_id = SessionId::from_db(&session.id)?;
        let host_id = UserId::from_db(&session.host_id)?;
        Ok(Self {
            session,
            session_id,
            host_id,
        })
    }

    /// Require that `user_id` is the host of this session.
    ///
    /// # Errors
    ///
    /// Returns `Forbidden` if `user_id` is not the host.
    pub fn require_host(&self, user_id: &UserId) -> Result<(), Error> {
        if self.host_id != *user_id {
            return Err(Error::forbidden("Only the host can do that"));
        }
        Ok(())
    }

    /// Require that `user_id` is an active participant and return their row.
    ///
    /// # Errors
    ///
    /// Propagates the errors of [`helpers::require_active_participant`]:
    /// `Forbidden` if the user is not an active participant of this session.
    #[tracing::instrument(
        level = "debug",
        skip(self, db),
        fields(session_id = %self.session_id, user_id = %user_id),
    )]
    pub async fn require_participant<C: ConnectionTrait>(
        &self,
        db: &C,
        user_id: &UserId,
    ) -> Result<session_participants::Model, Error> {
        helpers::require_active_participant(db, &self.session_id, user_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::enums::SessionStatus,
        test_helpers::{create_user, insert_participant, insert_session, setup_db},
    };

    #[tokio::test]
    async fn load_active_populates_session() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;

        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();
        assert_eq!(ctx.session.id, session_id.to_string());
        assert_eq!(ctx.session.host_id, host.to_string());
    }

    #[tokio::test]
    async fn load_active_missing_propagates_not_found() {
        let db = setup_db().await;
        let err = SessionContext::load_active(&db, &SessionId::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[tokio::test]
    async fn load_active_closed_propagates_conflict() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, SessionStatus::Closed).await;

        let err = SessionContext::load_active(&db, &session_id)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Conflict { .. }));
    }

    #[tokio::test]
    async fn require_host_matches() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        ctx.require_host(&host).expect("host matches");
    }

    #[tokio::test]
    async fn require_host_mismatch_is_forbidden() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let other = create_user(&db, "other").await;
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        let err = ctx.require_host(&other).unwrap_err();
        assert!(matches!(err, Error::Forbidden { .. }));
    }

    #[tokio::test]
    async fn require_participant_happy_and_forbidden_paths() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host, None).await;
        let ctx = SessionContext::load_active(&db, &session_id).await.unwrap();

        // Happy path: host is a participant.
        let row = ctx.require_participant(&db, &host).await.unwrap();
        assert_eq!(row.user_id, host.to_string());

        // Forbidden path: another user isn't.
        let outsider = create_user(&db, "outsider").await;
        let err = ctx.require_participant(&db, &outsider).await.unwrap_err();
        assert!(matches!(err, Error::Forbidden { .. }));
    }
}

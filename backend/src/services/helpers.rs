//! Small, reusable service-layer helpers.
//!
//! Building blocks for the larger service functions in `sessions` and `runs`.
//! Each helper is independently tested.

use chrono::Utc;
use rand::seq::SliceRandom;
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, EntityTrait, PrimaryKeyTrait, QueryFilter,
    sea_query::Expr,
};

use crate::domain::enums::SessionStatus;
use crate::entities::{session_participants, sessions, tracks};
use crate::error::AppError;

/// Load a session by ID and require that it is in the `Active` state.
///
/// - `NotFound` if no row with that ID exists.
/// - `Conflict` if the session exists but is closed.
pub async fn load_active_session<C: ConnectionTrait>(
    db: &C,
    session_id: &str,
) -> Result<sessions::Model, AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".into()))?;
    if session.status != SessionStatus::Active.as_str() {
        return Err(AppError::Conflict("Session is not active".into()));
    }
    Ok(session)
}

/// Require that `user_id` is an active (not-yet-left) participant in the
/// session. Returns the participant row on success.
///
/// `Forbidden` if the user has no active participant row for this session.
pub async fn require_active_participant<C: ConnectionTrait>(
    db: &C,
    session_id: &str,
    user_id: &str,
) -> Result<session_participants::Model, AppError> {
    session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
        .one(db)
        .await?
        .ok_or_else(|| AppError::Forbidden("Not a participant in this session".into()))
}

/// Bump the session's `last_activity_at` column to now. Single `UPDATE`,
/// no prior read required.
pub async fn touch_session<C: ConnectionTrait>(db: &C, session_id: &str) -> Result<(), AppError> {
    let now = Utc::now().naive_utc();
    sessions::Entity::update_many()
        .col_expr(sessions::Column::LastActivityAt, Expr::value(now))
        .filter(sessions::Column::Id.eq(session_id))
        .exec(db)
        .await?;
    Ok(())
}

/// Assert that a row with the given primary key exists in entity `E`.
/// Returns `BadRequest` with a formatted message on miss (e.g., "Invalid
/// character_id").
///
/// The generic bounds mirror `EntityTrait::find_by_id` — the primary key's
/// `ValueType` is what the caller must pass in.
pub async fn require_exists<E, C>(
    db: &C,
    id: <<E as EntityTrait>::PrimaryKey as PrimaryKeyTrait>::ValueType,
    entity_label: &str,
) -> Result<(), AppError>
where
    E: EntityTrait,
    C: ConnectionTrait,
{
    if E::find_by_id(id).one(db).await?.is_none() {
        return Err(AppError::BadRequest(format!("Invalid {entity_label}_id")));
    }
    Ok(())
}

/// Pick a random track, excluding IDs in `exclude`. If every track is in the
/// exclusion set (pool exhausted), reset the pool — but keep any IDs in
/// `always_exclude` filtered out even after the reset.
///
/// Two-tier exclusion supports both callers:
/// - `next_track`: `exclude = &used_ids, always_exclude = &[]` — full reset.
/// - `skip_turn`:  `exclude = &used_ids, always_exclude = &[skipped_id]` —
///   the skipped track stays excluded even through a reset.
///
/// Returns `Internal` only if the `tracks` table is empty (seed error).
pub async fn pick_random_track<C: ConnectionTrait>(
    db: &C,
    exclude: &[i32],
    always_exclude: &[i32],
) -> Result<tracks::Model, AppError> {
    let all_tracks = tracks::Entity::find().all(db).await?;
    if all_tracks.is_empty() {
        return Err(AppError::Internal("No tracks configured".into()));
    }

    let available: Vec<&tracks::Model> = all_tracks
        .iter()
        .filter(|t| !exclude.contains(&t.id) && !always_exclude.contains(&t.id))
        .collect();

    let pool: Vec<&tracks::Model> = if available.is_empty() {
        tracing::info!(
            excluded = exclude.len(),
            always_excluded = always_exclude.len(),
            "Track pool exhausted — resetting (always_exclude still applied)",
        );
        all_tracks
            .iter()
            .filter(|t| !always_exclude.contains(&t.id))
            .collect()
    } else {
        available
    };

    // `rand::thread_rng()` is !Send; keep it confined to this sync scope so
    // the returned `Model` can cross `.await` boundaries in callers.
    let chosen = {
        let mut rng = rand::thread_rng();
        pool.choose(&mut rng).copied().cloned()
    };

    chosen.ok_or_else(|| AppError::Internal("pick_random_track: empty pool after reset".into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::{ActiveModelTrait, Set};

    use crate::entities::{characters, cups, tracks};
    use crate::test_helpers::{
        create_user, insert_participant, insert_session, seed_tracks_for_test, setup_db,
    };

    // --- load_active_session ---

    #[tokio::test]
    async fn load_active_session_returns_active() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;

        let model = load_active_session(&db, &session_id).await.unwrap();
        assert_eq!(model.id, session_id);
        assert_eq!(model.status, "active");
    }

    #[tokio::test]
    async fn load_active_session_missing_is_not_found() {
        let db = setup_db().await;
        let err = load_active_session(&db, "does-not-exist")
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::NotFound(_)));
    }

    #[tokio::test]
    async fn load_active_session_closed_is_conflict() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "closed").await;

        let err = load_active_session(&db, &session_id).await.unwrap_err();
        assert!(matches!(err, AppError::Conflict(_)));
    }

    // --- require_active_participant ---

    #[tokio::test]
    async fn require_active_participant_returns_row_when_active() {
        let db = setup_db().await;
        let user = create_user(&db, "u1").await;
        let session_id = insert_session(&db, &user, "active").await;
        insert_participant(&db, &session_id, &user, None).await;

        let row = require_active_participant(&db, &session_id, &user)
            .await
            .unwrap();
        assert_eq!(row.user_id, user);
        assert!(row.left_at.is_none());
    }

    #[tokio::test]
    async fn require_active_participant_no_row_is_forbidden() {
        let db = setup_db().await;
        let user = create_user(&db, "u1").await;
        let session_id = insert_session(&db, &user, "active").await;

        let err = require_active_participant(&db, &session_id, &user)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    #[tokio::test]
    async fn require_active_participant_left_is_forbidden() {
        let db = setup_db().await;
        let user = create_user(&db, "u1").await;
        let session_id = insert_session(&db, &user, "active").await;
        insert_participant(&db, &session_id, &user, Some(Utc::now().naive_utc())).await;

        let err = require_active_participant(&db, &session_id, &user)
            .await
            .unwrap_err();
        assert!(matches!(err, AppError::Forbidden(_)));
    }

    // --- touch_session ---

    #[tokio::test]
    async fn touch_session_updates_last_activity_at() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;

        let before = sessions::Entity::find_by_id(&session_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .last_activity_at;

        // Sleep a millisecond so the new timestamp is strictly later. SQLite's
        // DateTime has microsecond precision; 1ms of real wall-clock time is
        // comfortably over the resolution boundary.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        touch_session(&db, &session_id).await.unwrap();

        let after = sessions::Entity::find_by_id(&session_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .last_activity_at;

        assert!(
            after > before,
            "expected last_activity_at to advance: before={before}, after={after}"
        );
    }

    // --- require_exists ---

    #[tokio::test]
    async fn require_exists_ok_when_row_present() {
        let db = setup_db().await;
        // Seed one cup; characters/etc. are empty.
        cups::ActiveModel {
            id: Set(7),
            name: Set("Seeded".to_string()),
            image_path: Set("x".to_string()),
        }
        .insert(&db)
        .await
        .unwrap();

        require_exists::<cups::Entity, _>(&db, 7, "cup")
            .await
            .expect("cup 7 exists");
    }

    #[tokio::test]
    async fn require_exists_bad_request_when_missing() {
        let db = setup_db().await;
        let err = require_exists::<characters::Entity, _>(&db, 999, "character")
            .await
            .unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert_eq!(msg, "Invalid character_id"),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    // --- pick_random_track ---

    #[tokio::test]
    async fn pick_random_track_returns_from_available() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;

        // Exclude tracks 1,2,3 — only 4,5,6 are available.
        let chosen = pick_random_track(&db, &[1, 2, 3], &[]).await.unwrap();
        assert!([4, 5, 6].contains(&chosen.id));
    }

    #[tokio::test]
    async fn pick_random_track_resets_when_pool_exhausted() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;

        // Exclude everything — should still return one of the seeded tracks.
        let chosen = pick_random_track(&db, &[1, 2, 3, 4, 5, 6], &[])
            .await
            .unwrap();
        assert!([1, 2, 3, 4, 5, 6].contains(&chosen.id));
    }

    #[tokio::test]
    async fn pick_random_track_always_exclude_survives_reset() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await; // 6 tracks: IDs 1-6

        // Exhaust the pool (exclude all 6) but always_exclude track 3.
        // On reset, track 3 stays excluded — result must not be 3.
        for _ in 0..20 {
            let chosen = pick_random_track(&db, &[1, 2, 3, 4, 5, 6], &[3])
                .await
                .unwrap();
            assert_ne!(
                chosen.id, 3,
                "always_exclude track must stay excluded through pool reset"
            );
        }
    }

    #[tokio::test]
    async fn pick_random_track_empty_table_is_internal() {
        let db = setup_db().await;
        // No tracks seeded.
        let err = pick_random_track(&db, &[], &[]).await.unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    #[tokio::test]
    async fn pick_random_track_uses_all_tracks_when_exclude_empty() {
        let db = setup_db().await;
        // Seed exactly one track so the choice is deterministic.
        cups::ActiveModel {
            id: Set(1),
            name: Set("Only Cup".to_string()),
            image_path: Set("x".to_string()),
        }
        .insert(&db)
        .await
        .unwrap();
        tracks::ActiveModel {
            id: Set(42),
            name: Set("Only Track".to_string()),
            cup_id: Set(1),
            position: Set(1),
            image_path: Set("x".to_string()),
        }
        .insert(&db)
        .await
        .unwrap();

        let chosen = pick_random_track(&db, &[], &[]).await.unwrap();
        assert_eq!(chosen.id, 42);
    }
}

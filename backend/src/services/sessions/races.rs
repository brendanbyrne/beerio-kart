//! Race orchestration within a session.
//!
//! Picks the next track, re-rolls the current one, and derives / mutates the
//! per-user pending-race surface. Read-side aggregation (current race in
//! session detail, race history) lives in [`super::detail`]; the
//! [`SessionRaceInfo`] DTO returned by the mutations is shared from
//! [`super::types`].
//!
//! [`SessionRaceInfo`]: super::types::SessionRaceInfo

use chrono::{NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, ModelTrait, PaginatorTrait, QueryFilter,
    QueryOrder, Set, TransactionTrait,
};

use super::types::{REJOIN_GRACE_MINUTES, SessionRaceInfo};
use crate::{
    domain::{ImagePath, SessionId, SessionRaceId, UserId},
    entities::{cups, runs, session_race_participations, session_races},
    error::Error,
    services::helpers,
};

/// Row shape for the pending-races query.
#[derive(Debug, FromQueryResult)]
struct PendingRaceRow {
    id: String,
    race_number: i32,
    track_id: i32,
    track_name: String,
    cup_name: String,
    image_path: String,
    created_at: NaiveDateTime,
}

/// Return the requesting user's pending races within a session, oldest first.
///
/// A race is **pending** for user `U` iff all of the following hold (per
/// docs/design.md "Pending Race Tracking"):
///
/// 1. A `session_race_participations` row exists for `(SR.id, U.id)` —
///    proves `U` was present when `SR` was created.
/// 2. `skipped_at IS NULL` on that row — `U` hasn't explicitly forfeited.
/// 3. No `runs` row exists for `(SR.id, U.id)` — `U` hasn't submitted.
/// 4. `U` is currently within grace — `session_participants.left_at IS NULL`
///    OR `NOW() - left_at <= REJOIN_GRACE_MINUTES`.
/// 5. `SR.created_at >= session_participants.joined_at` — excludes pre-gap
///    pending after a long-gap rejoin reset `joined_at`.
/// 6. `sessions.status = 'active'` — closed sessions accept no further
///    submissions, so any "pending" entries on them are phantom (the user
///    has no API path to resolve them). Required in addition to the grace
///    clause: a session can close while users are still inside their
///    5-minute grace window (e.g. via `close_stale_sessions`, or as part of
///    the host-leaves-last cascade).
///
/// Returns empty if the user is not a participant, all races are submitted/
/// skipped, the session is closed, or the user is past the grace window.
///
/// **Lazy check note:** The grace-period predicate is computed at query time
/// from `NOW()` and stored timestamps. No background task touches
/// `session_race_participations` rows — they remain in the DB for history
/// regardless of accessibility.
///
/// **`submissions` is intentionally empty.** Pending races report only the
/// race shell here; per-race submissions belong to the current/history views,
/// not the pending list. This avoids an N+1 query and keeps the polling path
/// fast.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures on the JOIN query.
#[tracing::instrument(
    skip(db),
    fields(session_id = %session_id, user_id = %user_id),
)]
pub async fn get_pending_races(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<Vec<SessionRaceInfo>, Error> {
    let now = Utc::now().naive_utc();
    let grace_cutoff = now - chrono::Duration::minutes(REJOIN_GRACE_MINUTES);

    let rows = PendingRaceRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sr.id, sr.race_number, sr.track_id,
               t.name AS track_name, c.name AS cup_name,
               t.image_path, sr.created_at
        FROM session_race_participations srp
        JOIN session_races sr ON srp.session_race_id = sr.id
        JOIN sessions s ON s.id = sr.session_id
        JOIN tracks t ON sr.track_id = t.id
        JOIN cups c ON t.cup_id = c.id
        JOIN session_participants sp
          ON sp.session_id = sr.session_id AND sp.user_id = srp.user_id
        WHERE sr.session_id = $1
          AND srp.user_id = $2
          AND s.status = 'active'
          AND srp.skipped_at IS NULL
          AND sr.created_at >= sp.joined_at
          AND (sp.left_at IS NULL OR sp.left_at >= $3)
          AND NOT EXISTS (
              SELECT 1 FROM runs r
              WHERE r.session_race_id = sr.id AND r.user_id = srp.user_id
          )
        ORDER BY sr.race_number ASC
        "#,
        [session_id.into(), user_id.into(), grace_cutoff.into()],
    ))
    .all(db)
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(SessionRaceInfo {
                id: SessionRaceId::from_db(&r.id)?,
                race_number: r.race_number,
                track_id: r.track_id,
                track_name: r.track_name,
                cup_name: r.cup_name,
                image_path: ImagePath::from_db(r.image_path, "tracks.image_path")?,
                created_at: r.created_at.and_utc(),
                submissions: Vec::new(),
            })
        })
        .collect()
}

/// Mark a pending race as skipped for the requesting user.
///
/// "Skip" means the user explicitly forfeits this race — they're not
/// submitting a time and want to be unblocked from submitting newer races.
/// The pending derivation in `get_pending_races` excludes rows where
/// `skipped_at IS NOT NULL`, so the race drops out of the user's pending
/// list immediately on success.
///
/// **Idempotent.** Calling skip twice on the same `(race, user)` returns
/// success both times; the timestamp is set on the first call only and
/// not updated on subsequent calls.
///
/// **Errors:**
/// - `Conflict` if the session is closed (via `load_active_session`).
/// - `NotFound("Race not found in this session")` if the race ID doesn't
///   belong to this session — covers both unknown race IDs and race IDs
///   from other sessions.
/// - `NotFound("Pending race not found")` if the user has no
///   `session_race_participations` row for this race (they were absent at
///   creation time, or the race is bogus). This is a single error class
///   for "not pending for you," not exposing whether the row exists for
///   another user.
/// - `Conflict("Already submitted")` if the user has a `runs` row for this
///   race — submitting and skipping are mutually exclusive (in both
///   directions; see also the matching skip-then-submit guard in
///   `services::runs::create_run`).
/// - `Forbidden` if the user is not currently an active participant of the
///   session. A user who left (even within their grace window) must rejoin
///   before they can act on pending races; otherwise the symmetry with
///   `create_run` breaks (which also requires an active participant).
///
/// # Errors
///
/// Returns the variants enumerated in the doc above, plus `NotFound` if the
/// race or session-race-participation row doesn't exist, and `Internal` for
/// unexpected DB failures.
#[tracing::instrument(
    skip(db),
    fields(
        session_id = %session_id,
        session_race_id = %session_race_id,
        user_id = %user_id,
    ),
)]
pub async fn skip_pending_race(
    db: &DatabaseConnection,
    session_id: &SessionId,
    session_race_id: &SessionRaceId,
    user_id: &UserId,
) -> Result<(), Error> {
    helpers::load_active_session(db, session_id)
        .await
        .map_err(|e| match e {
            Error::Conflict { .. } => Error::conflict("Cannot skip in a closed session"),
            other => other,
        })?;
    // Symmetry with `create_run`: the user must currently be in the session
    // to act on pending races. Within-grace users (`left_at IS NOT NULL`)
    // still see their pending races via `get_pending_races` clause 4, but
    // they need to rejoin before they can submit or skip.
    helpers::require_active_participant(db, session_id, user_id).await?;

    // Verify the race belongs to this session. Looking up by ID first and
    // then matching session_id keeps the error message consistent — both
    // "unknown race" and "race in another session" surface the same 404.
    let race = session_races::Entity::find_by_id(session_race_id)
        .one(db)
        .await?;
    let session_id_str = session_id.to_string();
    let Some(race) = race.filter(|r| r.session_id == session_id_str) else {
        return Err(Error::NotFound(
            "Race not found in this session".to_string(),
        ));
    };

    let participation = session_race_participations::Entity::find()
        .filter(
            Condition::all()
                .add(session_race_participations::Column::SessionRaceId.eq(&race.id))
                .add(session_race_participations::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Pending race not found".to_string()))?;

    // Reject if user already submitted a run for this race.
    let existing_run = runs::Entity::find()
        .filter(
            Condition::all()
                .add(runs::Column::SessionRaceId.eq(&race.id))
                .add(runs::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await?;
    if existing_run.is_some() {
        return Err(Error::conflict("Already submitted"));
    }

    // Idempotent: already skipped → return success without changing
    // skipped_at. This avoids both spurious updates and visible change in
    // the timestamp for repeated skip calls.
    if participation.skipped_at.is_some() {
        return Ok(());
    }

    let now = Utc::now().naive_utc();
    let txn = db.begin().await?;

    let mut active: session_race_participations::ActiveModel = participation.into();
    active.skipped_at = Set(Some(now));
    active.update(&txn).await?;

    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    Ok(())
}

/// Pick the next track for a session. Host-only.
/// Randomly selects from tracks not yet used in this session.
/// If all tracks have been used, resets the pool.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Conflict` if the
/// session is closed; `Forbidden` if `user_id` isn't the host; `Internal`
/// for unexpected DB failures or an empty `tracks` table.
#[tracing::instrument(skip(db), fields(session_id = %session_id, user_id = %user_id))]
pub async fn next_track(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<SessionRaceInfo, Error> {
    use crate::services::session_context::SessionContext;

    let ctx = SessionContext::load_active(db, session_id).await?;
    ctx.require_host(user_id)?;

    // Get already-used track IDs
    let used_races = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .all(db)
        .await?;
    let race_count = i32::try_from(used_races.len()).map_err(|_| {
        Error::Internal(anyhow::anyhow!(
            "session has more races than i32 can represent: {}",
            used_races.len()
        ))
    })?;
    let used_track_ids: Vec<i32> = used_races.iter().map(|r| r.track_id).collect();

    let chosen = helpers::pick_random_track(db, &used_track_ids, &[]).await?;

    let race_id = SessionRaceId::new_v4();
    let new_race_number = race_count + 1;

    let txn = db.begin().await?;

    // Capture the inserted row so we can echo its `before_save`-stamped
    // `created_at` back in the response.
    let inserted = session_races::ActiveModel {
        id: Set((&race_id).into()),
        session_id: Set(session_id.into()),
        race_number: Set(new_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: NotSet,
    }
    .insert(&txn)
    .await?;

    helpers::insert_race_participations(&txn, session_id, &race_id).await?;
    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    // Look up cup name for the response (FK-protected — missing is corruption)
    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            Error::Internal(anyhow::anyhow!(
                "Cup not found for cup_id {}",
                chosen.cup_id
            ))
        })?
        .name;

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: new_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: ImagePath::from_db(chosen.image_path.clone(), "tracks.image_path")?,
        created_at: inserted.created_at.and_utc(),
        submissions: Vec::new(),
    })
}

/// Re-roll the current track.
///
/// Any participant can trigger this (per docs/design.md — "any participant
/// can pass the chooser's turn"). Only valid if the most recent race has no
/// runs submitted. Deletes the current race and picks a new one in a single
/// transaction, excluding the skipped track from the pool so it can't come
/// back.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Conflict` if the
/// session is closed; `BadRequest` if there is no track to skip; `Conflict`
/// if any runs were already submitted for the current race; `Internal` for
/// unexpected DB failures.
#[tracing::instrument(
    skip(db, user_id),
    fields(session_id = %session_id, user_id = %user_id),
)]
pub async fn skip_turn(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<SessionRaceInfo, Error> {
    helpers::load_active_session(db, session_id).await?;

    // Find the most recent race
    let current_race = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .order_by_desc(session_races::Column::RaceNumber)
        .one(db)
        .await?
        .ok_or_else(|| Error::bad_request("No track to skip"))?;

    // Verify no runs exist for this race
    let run_count = runs::Entity::find()
        .filter(runs::Column::SessionRaceId.eq(&current_race.id))
        .count(db)
        .await?;

    if run_count > 0 {
        return Err(Error::bad_request("Can't skip — runs already submitted"));
    }

    let skipped_track_id = current_race.track_id;
    let keep_race_number = current_race.race_number;

    // Build the exclusion list: all tracks used in this session (including
    // the one being skipped, which next_track wouldn't see after deletion).
    let used_races = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .all(db)
        .await?;
    let exclude_ids: Vec<i32> = used_races.iter().map(|r| r.track_id).collect();

    let chosen = helpers::pick_random_track(db, &exclude_ids, &[skipped_track_id]).await?;

    let race_id = SessionRaceId::new_v4();

    // Delete old race + insert new one + snapshot present users in a single
    // transaction. The old race's `session_race_participations` rows cascade
    // away with the delete; the new race gets a fresh snapshot of who's
    // currently present.
    let txn = db.begin().await?;

    current_race.delete(&txn).await?;

    // Capture the inserted row so we can echo its `before_save`-stamped
    // `created_at` back in the response.
    let inserted = session_races::ActiveModel {
        id: Set((&race_id).into()),
        session_id: Set(session_id.into()),
        race_number: Set(keep_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: NotSet,
    }
    .insert(&txn)
    .await?;

    helpers::insert_race_participations(&txn, session_id, &race_id).await?;
    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            Error::Internal(anyhow::anyhow!(
                "Cup not found for cup_id {}",
                chosen.cup_id
            ))
        })?
        .name;

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: keep_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: ImagePath::from_db(chosen.image_path.clone(), "tracks.image_path")?,
        created_at: inserted.created_at.and_utc(),
        submissions: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::{
        domain::enums::SessionStatus,
        entities::sessions,
        services::sessions::{close_stale_sessions, create_session, join_session, leave_session},
        test_helpers::{
            backdate_participant, create_user, insert_participant, insert_race_participation,
            insert_session, insert_session_race, seed_tracks_for_test, setup_db,
        },
    };

    // ── Track selection tests ──────────────────────────────────────────

    #[tokio::test]
    async fn test_next_track_picks_random_track() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        assert_eq!(race.race_number, 1);
        assert!((1..=6).contains(&race.track_id));
        assert!(!race.track_name.is_empty());
        assert!(!race.cup_name.is_empty());

        // Verify session_race row was created
        let rows = session_races::Entity::find()
            .filter(session_races::Column::SessionId.eq(session.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].track_id, race.track_id);
    }

    #[tokio::test]
    async fn test_next_track_excludes_already_used_tracks() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let r1 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r2 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r3 = next_track(&db, &session.id, &host_id).await.unwrap();

        // All three should be different tracks
        let ids = [r1.track_id, r2.track_id, r3.track_id];
        let unique: std::collections::HashSet<_> = ids.iter().collect();
        assert_eq!(unique.len(), 3, "All 3 track picks should be unique");
    }

    #[tokio::test]
    async fn test_next_track_resets_pool_when_exhausted() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // Use all 6 tracks
        for _ in 0..6 {
            next_track(&db, &session.id, &host_id).await.unwrap();
        }

        // 7th call should succeed — pool resets
        let r7 = next_track(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(r7.race_number, 7);
        assert!((1..=6).contains(&r7.track_id));
    }

    #[tokio::test]
    async fn test_next_track_only_host_can_call() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_id).await.unwrap();

        let result = next_track(&db, &session.id, &user_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_next_track_fails_on_closed_session() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // Close by having host leave
        leave_session(&db, &session.id, &host_id).await.unwrap();

        let result = next_track(&db, &session.id, &host_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_skip_turn_rerolls_current_track() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let original = next_track(&db, &session.id, &host_id).await.unwrap();
        let rerolled = skip_turn(&db, &session.id, &host_id).await.unwrap();

        // The old race should be gone and a new one should exist
        let old_race = session_races::Entity::find_by_id(original.id)
            .one(&db)
            .await
            .unwrap();
        assert!(old_race.is_none(), "Old race should be deleted");

        // New race should exist
        let new_race = session_races::Entity::find_by_id(rerolled.id)
            .one(&db)
            .await
            .unwrap();
        assert!(new_race.is_some(), "New race should exist");

        // Race number should be 1 (replaced, not incremented)
        assert_eq!(rerolled.race_number, 1);

        // Skipped track must not come back
        assert_ne!(
            rerolled.track_id, original.track_id,
            "Skip must not re-roll to the same track"
        );
    }

    #[tokio::test]
    async fn test_skip_turn_excludes_skipped_track_even_near_pool_exhaustion() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await; // 6 tracks
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // Use 5 of 6 tracks, leaving only 1 available
        for _ in 0..5 {
            next_track(&db, &session.id, &host_id).await.unwrap();
        }

        // Pick the 6th (last) track
        let last = next_track(&db, &session.id, &host_id).await.unwrap();

        // Skip it — pool is now fully exhausted, but the skipped track
        // must still be excluded. The reset should offer 5 tracks.
        let rerolled = skip_turn(&db, &session.id, &host_id).await.unwrap();
        assert_ne!(
            rerolled.track_id, last.track_id,
            "Skip near pool exhaustion must not re-roll to the same track"
        );
        assert_eq!(rerolled.race_number, 6, "Race number should stay at 6");
    }

    #[tokio::test]
    async fn test_skip_turn_fails_when_no_race_exists() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let result = skip_turn(&db, &session.id, &host_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_skip_turn_any_participant_can_call() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_id).await.unwrap();
        let original = next_track(&db, &session.id, &host_id).await.unwrap();

        // Non-host participant should be able to skip
        let result = skip_turn(&db, &session.id, &user_id).await;
        assert!(result.is_ok());
        let rerolled = result.unwrap();
        assert_ne!(rerolled.track_id, original.track_id);
    }

    #[tokio::test]
    async fn test_race_number_increments_correctly() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let r1 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r2 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r3 = next_track(&db, &session.id, &host_id).await.unwrap();

        assert_eq!(r1.race_number, 1);
        assert_eq!(r2.race_number, 2);
        assert_eq!(r3.race_number, 3);
    }

    // ── Race-creation participation hook (PR 3D-1) ──────────────────────

    #[tokio::test]
    async fn test_create_session_race_inserts_participations_for_currently_present_users() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();

        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        let parts = session_race_participations::Entity::find()
            .filter(session_race_participations::Column::SessionRaceId.eq(race.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(parts.len(), 2, "one row per currently-present user");
        let user_ids: std::collections::HashSet<String> =
            parts.iter().map(|p| p.user_id.clone()).collect();
        assert!(user_ids.contains(&host_id.to_string()));
        assert!(user_ids.contains(&user2_id.to_string()));
        for p in &parts {
            assert!(
                p.skipped_at.is_none(),
                "fresh participation rows are not skipped"
            );
        }
    }

    #[tokio::test]
    async fn test_create_session_race_does_not_insert_participations_for_left_users() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let leaver_id = create_user(&db, "leaver").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &leaver_id).await.unwrap();
        leave_session(&db, &session.id, &leaver_id).await.unwrap();

        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        let parts = session_race_participations::Entity::find()
            .filter(session_race_participations::Column::SessionRaceId.eq(race.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            parts.len(),
            1,
            "only the still-present host should be snapshotted"
        );
        assert_eq!(parts[0].user_id, host_id.to_string());
    }

    #[tokio::test]
    async fn test_create_session_race_atomic_with_race_insert() {
        // If a participation insert fails inside the same transaction as the
        // session_races insert, the entire transaction must roll back —
        // the race row must not be visible afterwards. We force the failure
        // by trying to INSERT a participation row with a non-existent
        // user_id (FK violation) inside the same txn.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;

        let txn = db.begin().await.unwrap();
        let race_id = Uuid::new_v4().to_string();

        session_races::ActiveModel {
            id: Set(race_id.clone()),
            session_id: Set(session_id.into()),
            race_number: Set(1),
            track_id: Set(1),
            chosen_by: Set(None),
            created_at: NotSet,
        }
        .insert(&txn)
        .await
        .expect("race insert succeeds");

        // FK violation: user_id "ghost" doesn't exist in users.
        let bad = session_race_participations::ActiveModel {
            session_race_id: Set(race_id.clone()),
            user_id: Set("ghost".to_string()),
            created_at: NotSet,
            skipped_at: Set(None),
        }
        .insert(&txn)
        .await;
        assert!(bad.is_err(), "FK violation must surface as Err");

        txn.rollback().await.unwrap();

        let race = session_races::Entity::find_by_id(&race_id)
            .one(&db)
            .await
            .unwrap();
        assert!(
            race.is_none(),
            "session_races row must be rolled back when a participation insert fails"
        );
    }

    // ── Pending race query (PR 3D-1) ─────────────────────────────────────

    #[tokio::test]
    async fn test_pending_includes_unresolved_present_races() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].id, race.id);
        assert_eq!(pending[0].race_number, race.race_number);
        assert_eq!(pending[0].track_id, race.track_id);
    }

    #[tokio::test]
    async fn test_pending_excludes_submitted_races() {
        // Insert a runs row directly to avoid pulling all of run-creation's
        // validation surface into a query test.
        use crate::{
            drink_type_id::drink_type_uuid, entities::drink_types, test_helpers::seed_game_data,
        };

        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, Utc::now().naive_utc()).await;
        insert_race_participation(&db, &race_id, &host_id, None).await;

        // Verify pending before the run
        let pending = get_pending_races(&db, &session_id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 1);

        // Insert a run row for this (race, user)
        let drink_id = drink_type_uuid("Test Beer");
        let _drink = drink_types::Entity::find_by_id(drink_id)
            .one(&db)
            .await
            .unwrap()
            .expect("seed_game_data inserts Test Beer");
        runs::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set((&host_id).into()),
            session_race_id: Set((&race_id).into()),
            track_id: Set(1),
            character_id: Set(1),
            body_id: Set(1),
            wheel_id: Set(1),
            glider_id: Set(1),
            track_time: Set(120_000),
            lap1_time: Set(40_000),
            lap2_time: Set(40_000),
            lap3_time: Set(40_000),
            drink_type_id: Set((&drink_id).into()),
            disqualified: Set(false),
            photo_path: Set(None),
            created_at: NotSet,
            notes: Set(None),
        }
        .insert(&db)
        .await
        .unwrap();

        let pending = get_pending_races(&db, &session_id, &host_id).await.unwrap();
        assert!(pending.is_empty(), "submitted races drop from pending");
    }

    #[tokio::test]
    async fn test_pending_excludes_skipped_races() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, Utc::now().naive_utc()).await;

        // skipped_at IS NOT NULL → not pending
        insert_race_participation(&db, &race_id, &host_id, Some(Utc::now().naive_utc())).await;

        let pending = get_pending_races(&db, &session_id, &host_id).await.unwrap();
        assert!(pending.is_empty(), "skipped races drop from pending");
    }

    #[tokio::test]
    async fn test_pending_excludes_races_user_was_absent_for() {
        // User B leaves before the race is created — so next_track only
        // snapshots A. After B rejoins, B has no participation row for that
        // race and therefore no pending entry for it.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        leave_session(&db, &session.id, &user_b).await.unwrap();

        next_track(&db, &session.id, &host_id).await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();

        let pending_b = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert!(
            pending_b.is_empty(),
            "user absent at race creation has no pending row"
        );

        let pending_a = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(
            pending_a.len(),
            1,
            "host who was present has a pending entry"
        );
    }

    #[tokio::test]
    async fn test_pending_returned_ordered_by_race_number_asc() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let r1 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r2 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r3 = next_track(&db, &session.id, &host_id).await.unwrap();

        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        let race_numbers: Vec<i32> = pending.iter().map(|r| r.race_number).collect();
        assert_eq!(race_numbers, vec![1, 2, 3]);
        assert_eq!(pending[0].id, r1.id);
        assert_eq!(pending[1].id, r2.id);
        assert_eq!(pending[2].id, r3.id);
    }

    #[tokio::test]
    async fn test_pending_returns_all_records_ui_caps() {
        // Schema/API has no cap; the 3-cap is a UI concern. Verify the
        // backend returns more than 3 when more than 3 are pending.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        for _ in 0..5 {
            next_track(&db, &session.id, &host_id).await.unwrap();
        }

        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 5, "API returns all; UI applies the cap");
    }

    // ── Grace period (PR 3D-1) ──────────────────────────────────────────

    #[tokio::test]
    async fn test_pending_accessible_when_currently_in_session() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 1, "left_at IS NULL → pending accessible");
    }

    #[tokio::test]
    async fn test_pending_accessible_when_within_grace() {
        // Two participants so the session stays active after user_b leaves
        // (host transfer keeps it open). This isolates the grace check from
        // the session-status filter — we want to assert grace alone keeps
        // pending accessible.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();
        leave_session(&db, &session.id, &user_b).await.unwrap();

        // Backdate user_b's left_at to 3 minutes ago — well inside the
        // 5-minute window. Session is still active (host remained).
        let three_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(3);
        backdate_participant(&db, &session.id, &user_b, None, Some(three_min_ago)).await;

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert_eq!(
            pending.len(),
            1,
            "within grace + active session → pending accessible"
        );
    }

    #[tokio::test]
    async fn test_pending_inaccessible_when_grace_expired() {
        // Two participants so the session stays active — this isolates the
        // grace check from the status filter, proving exclusion is from
        // grace expiration alone.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();
        leave_session(&db, &session.id, &user_b).await.unwrap();

        // Backdate user_b's left_at to 6 minutes ago — past the 5-minute window.
        let six_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(6);
        backdate_participant(&db, &session.id, &user_b, None, Some(six_min_ago)).await;

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert!(pending.is_empty(), "grace expired → no accessible pending");

        // The row still exists in the DB (lazy check, no cleanup).
        let count = session_race_participations::Entity::find()
            .filter(session_race_participations::Column::UserId.eq(user_b))
            .count(&db)
            .await
            .unwrap();
        assert_eq!(count, 1, "participation row remains for history");
    }

    /// Documents intent: no background timer or sweeper task touches
    /// `session_race_participations`. Pending accessibility is a pure read-time
    /// derivation from `(session_race_participations, session_participants,
    /// runs)` — see `get_pending_races`. Forfeited rows remain in the DB
    /// indefinitely as historical state. If a future PR adds a sweeper, this
    /// assertion (and the design rationale in docs/design.md "Pending Race
    /// Tracking") needs to be revisited.
    #[tokio::test]
    async fn test_lazy_check_assertion() {
        // No-op test by design — this comment IS the assertion.
    }

    #[tokio::test]
    async fn test_pending_excludes_closed_session_even_within_grace() {
        // Regression: `close_stale_sessions` (and the host-leaves-last
        // cascade in leave_session) sets `left_at = NOW()` for still-active
        // participants and flips `status` to closed in the same transaction.
        // Within the next 5 minutes, those users are inside the grace window
        // — but the session can no longer accept submissions or skips, so
        // the pending entries are phantom. `get_pending_races` must filter
        // them out via `sessions.status = 'active'`.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        // Sanity: pending exists while session is active.
        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 1);

        // Backdate last_activity_at past the stale threshold so
        // close_stale_sessions catches it; this mirrors the production path
        // (sets left_at = NOW() and status = 'closed' atomically).
        let two_hours_ago = Utc::now().naive_utc() - chrono::Duration::hours(2);
        let s = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let mut active: sessions::ActiveModel = s.into();
        active.last_activity_at = Set(two_hours_ago);
        active.update(&db).await.unwrap();
        close_stale_sessions(&db).await.unwrap();

        // Host's left_at is now ~now (well within the 5-min grace), but the
        // session is closed — pending must be empty.
        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert!(
            pending.is_empty(),
            "closed session must not return pending even within grace window"
        );

        // The participation row itself stays in the DB for history.
        let count = session_race_participations::Entity::find()
            .filter(session_race_participations::Column::UserId.eq(host_id))
            .count(&db)
            .await
            .unwrap();
        assert_eq!(count, 1, "participation row remains for history");
    }

    // ── Skip pending race (PR 3D-2) ─────────────────────────────────────

    #[tokio::test]
    async fn test_skip_pending_race_sets_skipped_at() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .expect("skip succeeds");

        let row = session_race_participations::Entity::find()
            .filter(
                Condition::all()
                    .add(session_race_participations::Column::SessionRaceId.eq(race.id))
                    .add(session_race_participations::Column::UserId.eq(host_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(row.skipped_at.is_some(), "skipped_at must be set");

        // And the race drops out of the pending list.
        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert!(pending.is_empty(), "skipped race must not be pending");
    }

    #[tokio::test]
    async fn test_skip_pending_race_idempotent() {
        // Second skip on the same (race, user) returns success without
        // changing skipped_at — the timestamp from the first call is
        // preserved.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .expect("first skip");
        let first_skipped_at = session_race_participations::Entity::find()
            .filter(
                Condition::all()
                    .add(session_race_participations::Column::SessionRaceId.eq(race.id))
                    .add(session_race_participations::Column::UserId.eq(host_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .skipped_at;

        // Sleep a hair so any update would produce a strictly later timestamp.
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;

        skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .expect("second skip is idempotent");
        let second_skipped_at = session_race_participations::Entity::find()
            .filter(
                Condition::all()
                    .add(session_race_participations::Column::SessionRaceId.eq(race.id))
                    .add(session_race_participations::Column::UserId.eq(host_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .skipped_at;

        assert_eq!(
            first_skipped_at, second_skipped_at,
            "skipped_at must not change on idempotent re-skip"
        );
    }

    #[tokio::test]
    async fn test_skip_unknown_race_returns_404() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // No race exists for this session.
        let bogus_race_id = SessionRaceId::new_v4();
        let err = skip_pending_race(&db, &session.id, &bogus_race_id, &host_id)
            .await
            .unwrap_err();
        assert!(matches!(err, Error::NotFound(_)));
    }

    #[tokio::test]
    async fn test_skip_already_submitted_returns_conflict() {
        // Insert a runs row for (race, user) directly to avoid pulling
        // create_run's full validation surface into a skip test.
        use crate::{
            drink_type_id::drink_type_uuid, entities::drink_types, test_helpers::seed_game_data,
        };

        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        // Insert a run row to satisfy the "already submitted" precondition.
        let drink_id = drink_type_uuid("Test Beer");
        drink_types::Entity::find_by_id(drink_id)
            .one(&db)
            .await
            .unwrap()
            .expect("seed_game_data inserts Test Beer");
        runs::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set((&host_id).into()),
            session_race_id: Set((&race.id).into()),
            track_id: Set(race.track_id),
            character_id: Set(1),
            body_id: Set(1),
            wheel_id: Set(1),
            glider_id: Set(1),
            track_time: Set(120_000),
            lap1_time: Set(40_000),
            lap2_time: Set(40_000),
            lap3_time: Set(40_000),
            drink_type_id: Set((&drink_id).into()),
            disqualified: Set(false),
            photo_path: Set(None),
            created_at: NotSet,
            notes: Set(None),
        }
        .insert(&db)
        .await
        .unwrap();

        let err = skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .unwrap_err();
        match err {
            Error::Conflict { client, .. } => assert_eq!(client, "Already submitted"),
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_skip_pending_race_fails_on_closed_session() {
        // Mirrors test_next_track_fails_on_closed_session and
        // test_create_run_fails_if_session_closed: every closed-session
        // guard in the codebase has a corresponding test. Asserts the
        // custom Conflict remap fires (Cannot skip in a closed session)
        // rather than leaking the generic load_active_session message.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        // Close by having the only participant (host) leave.
        leave_session(&db, &session.id, &host_id).await.unwrap();

        let err = skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .unwrap_err();
        match err {
            Error::Conflict { client, .. } => {
                assert!(
                    client.contains("closed"),
                    "expected closed-session message, got: {client}"
                );
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn test_skip_after_leaving_returns_forbidden() {
        // Symmetry with create_run: a user who has left the session cannot
        // act on their pending races (even though they may still see them
        // via get_pending_races during the grace window). They must rejoin
        // first. Two-participant session so the session stays active when
        // the leaver leaves.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        // Sanity: user_b has the race pending while still active.
        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert_eq!(pending.len(), 1);

        leave_session(&db, &session.id, &user_b).await.unwrap();

        let err = skip_pending_race(&db, &session.id, &race.id, &user_b)
            .await
            .unwrap_err();
        assert!(
            matches!(err, Error::Forbidden(_)),
            "expected Forbidden for left user, got {err:?}"
        );
    }

    #[tokio::test]
    async fn test_skip_advances_pending_list() {
        // After skipping the oldest pending, the next-oldest becomes the
        // "must-submit-first" target for ordered-submit purposes. This is
        // a behavioral test, not just a state test — confirms the pending
        // list updates correctly so newer races become submittable in
        // order.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let r1 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r2 = next_track(&db, &session.id, &host_id).await.unwrap();
        let r3 = next_track(&db, &session.id, &host_id).await.unwrap();

        // All three are pending; oldest is r1.
        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending[0].id, r1.id);

        skip_pending_race(&db, &session.id, &r1.id, &host_id)
            .await
            .unwrap();

        // After skipping r1, oldest pending should now be r2.
        let pending = get_pending_races(&db, &session.id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].id, r2.id);
        assert_eq!(pending[1].id, r3.id);
    }
}

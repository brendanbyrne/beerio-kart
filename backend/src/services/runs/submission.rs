//! Run submission and deletion — the write paths.
//!
//! Holds [`create_run`] (with the validation pipeline and transactional insert
//! it delegates to) and [`delete_run`]. The read counterparts ([`get_run`],
//! `list_runs`, defaults) live in [`super::read`].

use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, ModelTrait, QueryFilter, Set, TransactionTrait,
};
use serde::Deserialize;

use super::read::{RunDetail, get_run};
use crate::{
    domain::{
        BodyId, CharacterId, DrinkTypeId, GliderId, LapTimeMs, MAX_TIME_MS, MIN_TIME_MS,
        RaceTimeMs, RunId, SessionId, SessionRaceId, UserId, WheelId, assert_lap_sum,
    },
    entities::{
        bodies, characters, drink_types, gliders, runs, session_race_participations, session_races,
        wheels,
    },
    error::Error,
    services::{helpers, sessions},
    timeout::{db_query, db_txn},
};

/// Body shape for `POST /runs`.
#[derive(Deserialize)]
pub struct CreateRunRequest {
    /// Race this run is submitted to.
    pub session_race_id: SessionRaceId,
    /// Total time in ms. Bounds-checked via `RaceTimeMs` in
    /// [`validate_time_fields`]; #146 will move this to the typed form directly.
    pub track_time: i32,
    /// Lap 1 time in ms. Bounds-checked via `LapTimeMs`.
    pub lap1_time: i32,
    /// Lap 2 time in ms. Bounds-checked via `LapTimeMs`.
    pub lap2_time: i32,
    /// Lap 3 time in ms. Bounds-checked via `LapTimeMs`.
    pub lap3_time: i32,
    /// Character used for the run.
    pub character_id: CharacterId,
    /// Kart body used for the run.
    pub body_id: BodyId,
    /// Wheels used for the run.
    pub wheel_id: WheelId,
    /// Glider used for the run.
    pub glider_id: GliderId,
    /// Drink type consumed during the run.
    pub drink_type_id: DrinkTypeId,
    /// `true` if the run is self-reported DQ.
    pub disqualified: bool,
}

/// Times on a run submission parsed into their typed forms.
///
/// Returned by [`validate_time_fields`] so the caller doesn't have to
/// re-parse the raw `i32`s when building the entity row — the typed
/// values are already bounds-checked by construction, and unwrapping
/// back to `i32` for the entity column is a single `.as_ref()` per
/// field. Clippy's `struct_field_names` lint flags the shared `_time`
/// suffix; the redundancy is load-bearing because the field names line
/// up with the API contract's `track_time` / `lap{1,2,3}_time` and the
/// entity columns of the same name.
#[allow(clippy::struct_field_names)]
struct ValidatedRunTimes {
    track_time: RaceTimeMs,
    lap1_time: LapTimeMs,
    lap2_time: LapTimeMs,
    lap3_time: LapTimeMs,
}

/// Validate the four time fields on a run submission.
///
/// Parses each `i32` into its typed newtype (enforcing the
/// `MIN_TIME_MS..=MAX_TIME_MS` bound at construction time), then
/// delegates the lap-sum invariant
/// (`lap1 + lap2 + lap3 == track_time`) to [`assert_lap_sum`]. The
/// invariant lives in one place — see `domain/numeric.rs` — and this
/// function is the boundary that translates `nutype` errors into the
/// user-facing `BadRequest` messages the API contract expects.
fn validate_time_fields(body: &CreateRunRequest) -> Result<ValidatedRunTimes, Error> {
    let track_time = RaceTimeMs::try_from(body.track_time).map_err(|_| {
        Error::bad_request(format!(
            "track_time must be between {MIN_TIME_MS} and {MAX_TIME_MS} ms"
        ))
    })?;
    let parse_lap = |value: i32, label: &str| -> Result<LapTimeMs, Error> {
        LapTimeMs::try_from(value).map_err(|_| {
            Error::bad_request(format!(
                "{label} must be between {MIN_TIME_MS} and {MAX_TIME_MS} ms"
            ))
        })
    };
    let lap1_time = parse_lap(body.lap1_time, "lap1_time")?;
    let lap2_time = parse_lap(body.lap2_time, "lap2_time")?;
    let lap3_time = parse_lap(body.lap3_time, "lap3_time")?;

    assert_lap_sum([lap1_time, lap2_time, lap3_time], track_time)?;

    Ok(ValidatedRunTimes {
        track_time,
        lap1_time,
        lap2_time,
        lap3_time,
    })
}

/// Create a run for a session race. Top-level orchestrator: validate, insert,
/// fetch. The validation surface and the transactional insert each live in
/// their own helper below.
///
/// # Errors
///
/// Returns `BadRequest` for invalid time fields or unknown FK references;
/// `NotFound` if the session race doesn't exist; `Conflict` if the session
/// is closed, the user already submitted, the user skipped the race, or an
/// older pending race blocks the submission; `Forbidden` if the user is not
/// an active participant; `Internal` for unexpected DB failures.
#[tracing::instrument(
    skip(db, body),
    fields(user_id = %user_id, session_race_id = %body.session_race_id),
)]
pub async fn create_run(
    db: &DatabaseConnection,
    user_id: &UserId,
    body: CreateRunRequest,
) -> Result<RunDetail, Error> {
    let (session_race, times) = validate_run_request(db, user_id, &body).await?;
    let run_id = insert_run(db, user_id, body, &times, &session_race).await?;
    get_run(db, &run_id).await
}

/// Run every gate that must pass before a run can be inserted. Returns the
/// loaded `session_race` so the caller doesn't re-fetch it.
///
/// Gates run in this order:
/// 1. Time-field arithmetic (lap times sum to track time, all within range).
/// 2. `session_race` exists.
/// 3. The session is still active.
/// 4. The user is an active participant in that session.
/// 5. The user hasn't already submitted a run for this race.
/// 6. The user hasn't explicitly skipped this race (mutual exclusion with
///    submit, per docs/design.md "Pending Race Tracking" → "Submission rules").
/// 7. The user has no older pending race blocking this submission
///    (ordered-submit guard, same source).
/// 8. All FK references (`character/body/wheel/glider/drink_type`) exist.
async fn validate_run_request(
    db: &impl ConnectionTrait,
    user_id: &UserId,
    body: &CreateRunRequest,
) -> Result<(session_races::Model, ValidatedRunTimes), Error> {
    let times = validate_time_fields(body)?;

    let session_race = db_query(session_races::Entity::find_by_id(body.session_race_id).one(db))
        .await?
        .ok_or_else(|| Error::NotFound("Session race not found".to_string()))?;

    let session_id = SessionId::from_db(&session_race.session_id)?;
    helpers::load_active_session(db, &session_id)
        .await
        .map_err(|e| match e {
            Error::Conflict { .. } => {
                Error::session_closed("Cannot submit run for a closed session")
            }
            other => other,
        })?;
    helpers::require_active_participant(db, &session_id, user_id).await?;

    // Check for duplicate submission
    let existing = db_query(
        runs::Entity::find()
            .filter(
                Condition::all()
                    .add(runs::Column::SessionRaceId.eq(body.session_race_id))
                    .add(runs::Column::UserId.eq(user_id)),
            )
            .one(db),
    )
    .await?;

    if existing.is_some() {
        return Err(Error::conflict("Already submitted a run for this race"));
    }

    // Mutual exclusion with skip: if the user already explicitly skipped
    // this race, they can't submit a time for it. Skip is treated as a
    // permanent forfeiture, matching the "submit OR skip" framing in
    // docs/design.md "Pending Race Tracking" → "Submission rules" and the
    // mutual-exclusion guarantee in `skip_pending_race`'s docstring.
    let participation = db_query(
        session_race_participations::Entity::find()
            .filter(
                Condition::all()
                    .add(
                        session_race_participations::Column::SessionRaceId.eq(body.session_race_id),
                    )
                    .add(session_race_participations::Column::UserId.eq(user_id)),
            )
            .one(db),
    )
    .await?;
    if participation
        .as_ref()
        .is_some_and(|p| p.skipped_at.is_some())
    {
        return Err(Error::conflict("Cannot submit a run for a skipped race"));
    }

    // Ordered-submit guard: if the user has any pending race with a smaller
    // race_number, they must submit or skip those first. Prevents
    // cherry-picking favorable tracks for H2H purposes (docs/design.md "Pending
    // Race Tracking" → "Submission rules"). Skipping the older race or
    // submitting it (which clears it via the `runs` row check in
    // `get_pending_races`) unblocks newer submissions.
    //
    // Use filter + min_by_key rather than a bare `find`: `find` would rely
    // on `get_pending_races` returning ASC by race_number to make the first
    // match the oldest. The contract holds today, but `min_by_key` removes
    // the implicit dependency so the error message can't silently name a
    // wrong race if the SQL ORDER BY is ever dropped or inverted.
    let pending = sessions::get_pending_races(db, &session_id, user_id).await?;
    if let Some(older) = pending
        .iter()
        .filter(|p| p.race_number < session_race.race_number)
        .min_by_key(|p| p.race_number)
    {
        return Err(Error::pending_races_first(format!(
            "Must submit or skip pending race #{} first",
            older.race_number
        )));
    }

    // Validate FK references exist
    helpers::require_exists::<characters::Entity, _>(db, body.character_id.into(), "character")
        .await?;
    helpers::require_exists::<bodies::Entity, _>(db, body.body_id.into(), "body").await?;
    helpers::require_exists::<wheels::Entity, _>(db, body.wheel_id.into(), "wheel").await?;
    helpers::require_exists::<gliders::Entity, _>(db, body.glider_id.into(), "glider").await?;
    helpers::require_exists::<drink_types::Entity, _>(
        db,
        (&body.drink_type_id).into(),
        "drink_type",
    )
    .await?;

    Ok((session_race, times))
}

/// Insert the run row. Returns the new run's ID. Caller is expected to have
/// already validated the request via `validate_run_request`.
async fn insert_run(
    db: &DatabaseConnection,
    user_id: &UserId,
    body: CreateRunRequest,
    times: &ValidatedRunTimes,
    session_race: &session_races::Model,
) -> Result<RunId, Error> {
    let run_id = RunId::new_v4();

    let txn = db_txn(db.begin()).await?;

    db_query(
        runs::ActiveModel {
            id: Set((&run_id).into()),
            user_id: Set(user_id.into()),
            session_race_id: Set((&body.session_race_id).into()),
            track_id: Set(session_race.track_id),
            character_id: Set(body.character_id.into()),
            body_id: Set(body.body_id.into()),
            wheel_id: Set(body.wheel_id.into()),
            glider_id: Set(body.glider_id.into()),
            track_time: Set(*times.track_time.as_ref()),
            lap1_time: Set(*times.lap1_time.as_ref()),
            lap2_time: Set(*times.lap2_time.as_ref()),
            lap3_time: Set(*times.lap3_time.as_ref()),
            drink_type_id: Set((&body.drink_type_id).into()),
            disqualified: Set(body.disqualified),
            photo_path: Set(None),
            created_at: NotSet,
            notes: Set(None),
        }
        .insert(&txn),
    )
    .await?;

    db_txn(txn.commit()).await?;

    Ok(run_id)
}

/// Delete a run. Only the run's owner can delete, and the session must be active.
///
/// # Errors
///
/// Returns `NotFound` if no run with that ID exists; `Forbidden` if the
/// caller is not the run's owner; `Conflict` if the run's session is closed;
/// `Internal` for unexpected DB failures or data-corruption invariants (e.g.,
/// missing session for an existing run).
#[tracing::instrument(skip(db), fields(run_id = %run_id, user_id = %user_id))]
pub async fn delete_run(
    db: &DatabaseConnection,
    run_id: &RunId,
    user_id: &UserId,
) -> Result<(), Error> {
    let run = db_query(runs::Entity::find_by_id(run_id).one(db))
        .await?
        .ok_or_else(|| Error::NotFound("Run not found".to_string()))?;

    // Lift the entity-layer `String` to a typed `UserId` and compare in the
    // domain. Stays consistent with `session_context.rs::require_host` and
    // surfaces a corrupt-UUID-in-DB as 500 instead of a silent false-negative
    // compare (FK-protected so it should never fire, but the failure mode is
    // worth surfacing if it ever does).
    let owner = UserId::from_db(&run.user_id)?;
    if owner != *user_id {
        return Err(Error::forbidden("Only the run's owner can delete it"));
    }

    // Check that the session is still active
    let session_race = db_query(session_races::Entity::find_by_id(&run.session_race_id).one(db))
        .await?
        .ok_or_else(|| Error::Internal(anyhow::anyhow!("Session race not found for run")))?;

    let session_id = SessionId::from_db(&session_race.session_id)?;
    // FK guarantees the session exists; NotFound here signals data corruption.
    helpers::load_active_session(db, &session_id)
        .await
        .map_err(|e| match e {
            Error::NotFound(msg) => {
                Error::Internal(anyhow::anyhow!("Session not found for run: {msg}"))
            }
            Error::Conflict { .. } => {
                Error::session_closed("Cannot delete run from a closed session")
            }
            other => other,
        })?;

    let txn = db_txn(db.begin()).await?;

    db_query(run.delete(&txn)).await?;

    db_txn(txn.commit()).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::enums::SessionStatus,
        entities::sessions as sessions_entity,
        services::sessions,
        test_helpers::{create_user, seed_game_data, setup_db},
    };

    fn test_drink_id() -> DrinkTypeId {
        crate::drink_type_id::drink_type_uuid("Test Beer", true)
    }

    // ── validate_time_fields (pure, no DB needed) ────────────────────

    fn valid_time_request() -> CreateRunRequest {
        CreateRunRequest {
            session_race_id: SessionRaceId::new_v4(),
            track_time: 120_000,
            lap1_time: 40_000,
            lap2_time: 39_000,
            lap3_time: 41_000,
            character_id: CharacterId::new(1),
            body_id: BodyId::new(1),
            wheel_id: WheelId::new(1),
            glider_id: GliderId::new(1),
            drink_type_id: DrinkTypeId::new_v4(),
            disqualified: false,
        }
    }

    #[test]
    fn test_validate_time_fields_accepts_valid_times() {
        assert!(validate_time_fields(&valid_time_request()).is_ok());
    }

    #[test]
    fn test_validate_time_fields_rejects_zero_track_time() {
        let mut req = valid_time_request();
        req.track_time = 0;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_negative_track_time() {
        let mut req = valid_time_request();
        req.track_time = -1;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_track_time_over_max() {
        let mut req = valid_time_request();
        req.track_time = MAX_TIME_MS + 1;
        req.lap1_time = 200_001;
        req.lap2_time = 200_000;
        req.lap3_time = 200_000;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_zero_lap_time() {
        let mut req = valid_time_request();
        req.lap2_time = 0;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_negative_lap_time() {
        let mut req = valid_time_request();
        req.lap3_time = -5;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_lap_time_over_max() {
        let mut req = valid_time_request();
        req.lap1_time = MAX_TIME_MS + 1;
        assert!(validate_time_fields(&req).is_err());
    }

    #[test]
    fn test_validate_time_fields_rejects_lap_sum_mismatch() {
        let mut req = valid_time_request();
        req.lap1_time = 20_000;
        req.lap2_time = 20_000;
        req.lap3_time = 20_000;
        // laps sum to 60_000 but track_time is 120_000
        assert!(validate_time_fields(&req).is_err());
    }

    fn valid_run_request(session_race_id: &SessionRaceId) -> CreateRunRequest {
        CreateRunRequest {
            session_race_id: *session_race_id,
            track_time: 120_000,
            lap1_time: 40_000,
            lap2_time: 39_000,
            lap3_time: 41_000,
            character_id: CharacterId::new(1),
            body_id: BodyId::new(1),
            wheel_id: WheelId::new(1),
            glider_id: GliderId::new(1),
            drink_type_id: test_drink_id(),
            disqualified: false,
        }
    }

    /// Helper: create session, pick a track, return (`session_id`, `session_race_id`)
    async fn setup_session_with_race(
        db: &DatabaseConnection,
        host_id: &UserId,
    ) -> (SessionId, SessionRaceId) {
        let session = sessions::create_session(db, host_id, "random")
            .await
            .expect("create session");
        let race = sessions::next_track(db, &session.id, host_id)
            .await
            .expect("next track");
        (session.id, race.id)
    }

    #[tokio::test]
    async fn test_create_run_succeeds_with_valid_data() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let run = create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();

        assert_eq!(run.user_id, host_id);
        assert_eq!(run.track_time, 120_000);
        assert_eq!(run.lap1_time, 40_000);
        assert_eq!(run.username.as_ref(), "host");
        assert_eq!(run.drink_type_name, "Test Beer");
        assert!(!run.disqualified);
    }

    #[tokio::test]
    async fn test_create_run_fails_if_not_participant() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let outsider_id = create_user(&db, "outsider").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let result = create_run(&db, &outsider_id, valid_run_request(&race_id)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_if_duplicate_submission() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();
        let result = create_run(&db, &host_id, valid_run_request(&race_id)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_if_session_closed() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (session_id, race_id) = setup_session_with_race(&db, &host_id).await;

        // Close session by having host leave
        sessions::leave_session(&db, &session_id, &host_id)
            .await
            .unwrap();

        let result = create_run(&db, &host_id, valid_run_request(&race_id)).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_invalid_track_time() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        // Negative time
        let mut req = valid_run_request(&race_id);
        req.track_time = -1;
        assert!(create_run(&db, &host_id, req).await.is_err());

        // Over 10 minutes (also adjust laps to match so we test track_time cap, not lap sum)
        let mut req = valid_run_request(&race_id);
        req.track_time = 600_001;
        req.lap1_time = 200_000;
        req.lap2_time = 200_000;
        req.lap3_time = 200_001;
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_lap_sum_mismatch() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        // Laps sum to 60000 but total is 120000 — must match exactly
        let mut req = valid_run_request(&race_id);
        req.lap1_time = 20_000;
        req.lap2_time = 20_000;
        req.lap3_time = 20_000;
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_lap_time_exceeding_max() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let mut req = valid_run_request(&race_id);
        req.lap1_time = 600_001;
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_invalid_character_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let mut req = valid_run_request(&race_id);
        req.character_id = CharacterId::new(999);
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_invalid_drink_type_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let mut req = valid_run_request(&race_id);
        // Valid UUID shape but no matching row — exercises the FK check, not
        // the type-level parse (which a non-UUID string would short-circuit).
        req.drink_type_id = DrinkTypeId::new_v4();
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_delete_run_succeeds_for_owner() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let run = create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();
        assert!(delete_run(&db, &run.id, &host_id).await.is_ok());

        // Verify it's gone
        assert!(get_run(&db, &run.id).await.is_err());
    }

    #[tokio::test]
    async fn test_delete_run_fails_for_non_owner() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;
        let (session_id, race_id) = setup_session_with_race(&db, &host_id).await;
        sessions::join_session(&db, &session_id, &user_id)
            .await
            .unwrap();

        let run = create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();
        let result = delete_run(&db, &run.id, &user_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_run_fails_if_session_closed() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;
        let (session_id, race_id) = setup_session_with_race(&db, &host_id).await;
        sessions::join_session(&db, &session_id, &user_id)
            .await
            .unwrap();

        let run = create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();

        // Close by having everyone leave
        sessions::leave_session(&db, &session_id, &host_id)
            .await
            .ok();
        sessions::leave_session(&db, &session_id, &user_id)
            .await
            .ok();

        // Force-close in case leave order was wrong
        let s = sessions_entity::Entity::find_by_id(session_id)
            .one(&db)
            .await
            .unwrap();
        if let Some(s) = s {
            let mut active: sessions_entity::ActiveModel = s.into();
            active.status = Set(SessionStatus::Closed);
            active.update(&db).await.unwrap();
        }

        let result = delete_run(&db, &run.id, &host_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_submissions_appear_in_session_detail() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (session_id, race_id) = setup_session_with_race(&db, &host_id).await;

        // Before submitting
        let detail = sessions::get_session_detail(&db, &session_id, Some(&host_id))
            .await
            .unwrap();
        let current = detail.current_race.unwrap();
        assert!(current.submissions.is_empty());

        // Submit a run
        create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();

        // After submitting
        let detail = sessions::get_session_detail(&db, &session_id, Some(&host_id))
            .await
            .unwrap();
        let current = detail.current_race.unwrap();
        assert_eq!(current.submissions.len(), 1);
        assert_eq!(current.submissions[0].username.as_ref(), "host");
        assert_eq!(current.submissions[0].track_time, 120_000);
        assert!(!current.submissions[0].disqualified);
    }

    // ── Ordered-submit guard (PR 3D-2) ────────────────────────────────────

    /// Helper: create a session, pick N tracks, return (`session_id`, `race_ids`).
    async fn setup_session_with_n_races(
        db: &DatabaseConnection,
        host_id: &UserId,
        n: usize,
    ) -> (SessionId, Vec<SessionRaceId>) {
        let session = sessions::create_session(db, host_id, "random")
            .await
            .expect("create session");
        let mut race_ids = Vec::with_capacity(n);
        for _ in 0..n {
            let race = sessions::next_track(db, &session.id, host_id)
                .await
                .expect("next track");
            race_ids.push(race.id);
        }
        (session.id, race_ids)
    }

    #[tokio::test]
    async fn test_submit_oldest_pending_first_succeeds() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 3).await;

        // Submit race 1 (the oldest pending) — should succeed.
        create_run(&db, &host_id, valid_run_request(&race_ids[0]))
            .await
            .expect("submitting oldest pending should succeed");
    }

    #[tokio::test]
    async fn test_submit_newer_pending_while_older_exists_returns_conflict() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 3).await;

        // Try to submit race 3 while races 1 and 2 are still pending.
        match create_run(&db, &host_id, valid_run_request(&race_ids[2])).await {
            Err(Error::Conflict { client, .. }) => assert!(
                client.contains("Must submit or skip pending race #1"),
                "expected message about race #1, got: {client}"
            ),
            Err(other) => panic!("expected Conflict, got {other:?}"),
            Ok(_) => panic!("expected Conflict, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_submit_current_race_with_no_pending_succeeds() {
        // Control case: with only one race in the session and no prior pending,
        // submitting the current race must succeed. Confirms the guard does
        // not over-reject.
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 1).await;

        create_run(&db, &host_id, valid_run_request(&race_ids[0]))
            .await
            .expect("no pending → submit succeeds");
    }

    #[tokio::test]
    async fn test_submit_current_race_with_older_pending_returns_conflict() {
        // Same shape as the "newer while older exists" test but framed as
        // "current race counts as newer." Two-race session, race 2 is current,
        // race 1 is pending.
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 2).await;

        match create_run(&db, &host_id, valid_run_request(&race_ids[1])).await {
            Err(Error::Conflict { .. }) => {}
            Err(other) => panic!("expected Conflict, got {other:?}"),
            Ok(_) => panic!("expected Conflict, got Ok"),
        }
    }

    #[tokio::test]
    async fn test_submit_current_race_after_skipping_all_pending_succeeds() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 3).await;

        // Skip races 1 and 2; race 3 should now be submittable.
        sessions::skip_pending_race(&db, &session_id, &race_ids[0], &host_id)
            .await
            .expect("skip race 1");
        sessions::skip_pending_race(&db, &session_id, &race_ids[1], &host_id)
            .await
            .expect("skip race 2");

        create_run(&db, &host_id, valid_run_request(&race_ids[2]))
            .await
            .expect("after skipping all older pending, submit succeeds");
    }

    #[tokio::test]
    async fn test_submit_after_skip_returns_conflict() {
        // Regression for the submit-after-skip bypass: skip and submit are
        // mutually exclusive in BOTH directions. After skipping race N, a
        // later submit for race N must be rejected — otherwise the user
        // could skip race N to clear the ordered-submit guard for race N+1
        // and then come back and submit race N anyway.
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (session_id, race_ids) = setup_session_with_n_races(&db, &host_id, 1).await;

        sessions::skip_pending_race(&db, &session_id, &race_ids[0], &host_id)
            .await
            .expect("skip succeeds");

        match create_run(&db, &host_id, valid_run_request(&race_ids[0])).await {
            Err(Error::Conflict { client, .. }) => assert!(
                client.contains("skipped"),
                "expected message about skipped race, got: {client}"
            ),
            Err(other) => panic!("expected Conflict, got {other:?}"),
            Ok(_) => panic!("submitting after skip must Conflict, got Ok"),
        }
    }
}

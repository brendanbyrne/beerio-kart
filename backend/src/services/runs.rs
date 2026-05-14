use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, ModelTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{
        BodyId, CharacterId, DrinkTypeId, GliderId, LapTimeMs, MAX_TIME_MS, RaceTimeMs, RunId,
        SessionId, SessionRaceId, TrackId, UserId, Username, WheelId, numeric::assert_lap_sum,
    },
    entities::{
        bodies, characters, drink_types, gliders, runs, session_race_participations, session_races,
        users, wheels,
    },
    error::Error,
    services::{helpers, sessions},
};

#[derive(Deserialize)]
pub struct CreateRunRequest {
    pub session_race_id: SessionRaceId,
    pub track_time: i32,
    pub lap1_time: i32,
    pub lap2_time: i32,
    pub lap3_time: i32,
    pub character_id: CharacterId,
    pub body_id: BodyId,
    pub wheel_id: WheelId,
    pub glider_id: GliderId,
    pub drink_type_id: DrinkTypeId,
    pub disqualified: bool,
}

#[derive(Serialize)]
pub struct RunDetail {
    pub id: RunId,
    pub user_id: UserId,
    pub username: Username,
    pub session_race_id: SessionRaceId,
    pub track_id: TrackId,
    pub track_time: i32,
    pub lap1_time: i32,
    pub lap2_time: i32,
    pub lap3_time: i32,
    pub character_id: CharacterId,
    pub body_id: BodyId,
    pub wheel_id: WheelId,
    pub glider_id: GliderId,
    pub drink_type_id: DrinkTypeId,
    pub drink_type_name: String,
    pub disqualified: bool,
    pub created_at: DateTime<Utc>,
}

/// Row shape for the run detail JOIN query.
#[derive(Debug, FromQueryResult)]
struct RunDetailRow {
    id: String,
    user_id: String,
    username: String,
    session_race_id: String,
    track_id: i32,
    track_time: i32,
    lap1_time: i32,
    lap2_time: i32,
    lap3_time: i32,
    character_id: i32,
    body_id: i32,
    wheel_id: i32,
    glider_id: i32,
    drink_type_id: String,
    drink_type_name: String,
    disqualified: bool,
    created_at: NaiveDateTime,
}

impl RunDetailRow {
    /// Parse a row into a typed [`RunDetail`]. Fallible because every UUID
    /// column has to round-trip through `from_db`; an invalid UUID in any of
    /// those columns is data corruption and surfaces as `Internal`.
    fn try_into_detail(self) -> Result<RunDetail, Error> {
        Ok(RunDetail {
            id: RunId::from_db(&self.id)?,
            user_id: UserId::from_db(&self.user_id)?,
            username: Username::from_db(self.username, "users.username")?,
            session_race_id: SessionRaceId::from_db(&self.session_race_id)?,
            track_id: TrackId::new(self.track_id),
            track_time: self.track_time,
            lap1_time: self.lap1_time,
            lap2_time: self.lap2_time,
            lap3_time: self.lap3_time,
            character_id: CharacterId::new(self.character_id),
            body_id: BodyId::new(self.body_id),
            wheel_id: WheelId::new(self.wheel_id),
            glider_id: GliderId::new(self.glider_id),
            drink_type_id: DrinkTypeId::from_db(&self.drink_type_id)?,
            drink_type_name: self.drink_type_name,
            disqualified: self.disqualified,
            created_at: self.created_at.and_utc(),
        })
    }
}

#[derive(Serialize)]
pub struct RunDefaults {
    pub drink_type_id: Option<DrinkTypeId>,
    pub character_id: Option<CharacterId>,
    pub body_id: Option<BodyId>,
    pub wheel_id: Option<WheelId>,
    pub glider_id: Option<GliderId>,
    pub source: String,
}

pub struct RunFilters {
    pub session_race_id: Option<SessionRaceId>,
    pub user_id: Option<UserId>,
    pub track_id: Option<TrackId>,
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
/// Parses each `i32` into its typed newtype (enforcing the `1..=MAX_TIME_MS`
/// bound at construction time), then delegates the lap-sum invariant
/// (`lap1 + lap2 + lap3 == track_time`) to [`assert_lap_sum`]. The
/// invariant lives in one place — see `domain/numeric.rs` — and this
/// function is the boundary that translates `nutype` errors into the
/// user-facing `BadRequest` messages the API contract expects.
fn validate_time_fields(body: &CreateRunRequest) -> Result<ValidatedRunTimes, Error> {
    let track_time = RaceTimeMs::try_from(body.track_time).map_err(|_| {
        Error::bad_request(format!("track_time must be between 1 and {MAX_TIME_MS} ms"))
    })?;
    let parse_lap = |value: i32, label: &str| -> Result<LapTimeMs, Error> {
        LapTimeMs::try_from(value).map_err(|_| {
            Error::bad_request(format!("{label} must be between 1 and {MAX_TIME_MS} ms"))
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

    let session_race = session_races::Entity::find_by_id(body.session_race_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Session race not found".to_string()))?;

    let session_id = SessionId::from_db(&session_race.session_id)?;
    helpers::load_active_session(db, &session_id)
        .await
        .map_err(|e| match e {
            Error::Conflict { .. } => Error::conflict("Cannot submit run for a closed session"),
            other => other,
        })?;
    helpers::require_active_participant(db, &session_id, user_id).await?;

    // Check for duplicate submission
    let existing = runs::Entity::find()
        .filter(
            Condition::all()
                .add(runs::Column::SessionRaceId.eq(body.session_race_id))
                .add(runs::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(Error::conflict("Already submitted a run for this race"));
    }

    // Mutual exclusion with skip: if the user already explicitly skipped
    // this race, they can't submit a time for it. Skip is treated as a
    // permanent forfeiture, matching the "submit OR skip" framing in
    // docs/design.md "Pending Race Tracking" → "Submission rules" and the
    // mutual-exclusion guarantee in `skip_pending_race`'s docstring.
    let participation = session_race_participations::Entity::find()
        .filter(
            Condition::all()
                .add(session_race_participations::Column::SessionRaceId.eq(body.session_race_id))
                .add(session_race_participations::Column::UserId.eq(user_id)),
        )
        .one(db)
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
        return Err(Error::conflict(format!(
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

/// Insert the run row and bump the session's `last_activity_at`, atomically.
/// Returns the new run's ID. Caller is expected to have already validated
/// the request via `validate_run_request`.
async fn insert_run(
    db: &DatabaseConnection,
    user_id: &UserId,
    body: CreateRunRequest,
    times: &ValidatedRunTimes,
    session_race: &session_races::Model,
) -> Result<RunId, Error> {
    let run_id = RunId::new_v4();

    let txn = db.begin().await?;

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
    .insert(&txn)
    .await?;

    let session_id = SessionId::from_db(&session_race.session_id)?;
    helpers::touch_session(&txn, &session_id).await?;

    txn.commit().await?;

    Ok(run_id)
}

/// Fetch a single run by ID with `JOINed` username and `drink_type_name`.
///
/// # Errors
///
/// Returns `NotFound` if no run with that ID exists; `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip(db), fields(run_id = %run_id))]
pub async fn get_run(db: &impl ConnectionTrait, run_id: &RunId) -> Result<RunDetail, Error> {
    let row = RunDetailRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT r.id, r.user_id, u.username, r.session_race_id, r.track_id,
               r.track_time, r.lap1_time, r.lap2_time, r.lap3_time,
               r.character_id, r.body_id, r.wheel_id, r.glider_id,
               r.drink_type_id, dt.name AS drink_type_name,
               r.disqualified, r.created_at
        FROM runs r
        JOIN users u ON r.user_id = u.id
        JOIN drink_types dt ON r.drink_type_id = dt.id
        WHERE r.id = $1
        "#,
        [run_id.into()],
    ))
    .one(db)
    .await?
    .ok_or_else(|| Error::NotFound("Run not found".to_string()))?;

    row.try_into_detail()
}

/// List runs with optional filters, ordered by `track_time` ASC.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(
    skip(db, filters),
    fields(
        session_race_id = ?filters.session_race_id,
        user_id = ?filters.user_id,
        track_id = ?filters.track_id,
    ),
)]
pub async fn list_runs(
    db: &impl ConnectionTrait,
    filters: RunFilters,
) -> Result<Vec<RunDetail>, Error> {
    let mut conditions = Vec::new();
    let mut params: Vec<sea_orm::Value> = Vec::new();

    let mut add_filter = |column: &str, value: sea_orm::Value| {
        let idx = params.len() + 1;
        conditions.push(format!("{column} = ${idx}"));
        params.push(value);
    };

    if let Some(sr_id) = filters.session_race_id {
        add_filter("r.session_race_id", sr_id.into());
    }
    if let Some(uid) = filters.user_id {
        add_filter("r.user_id", uid.into());
    }
    if let Some(tid) = filters.track_id {
        add_filter("r.track_id", tid.into());
    }

    let where_clause = if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    };

    let sql = format!(
        r#"
        SELECT r.id, r.user_id, u.username, r.session_race_id, r.track_id,
               r.track_time, r.lap1_time, r.lap2_time, r.lap3_time,
               r.character_id, r.body_id, r.wheel_id, r.glider_id,
               r.drink_type_id, dt.name AS drink_type_name,
               r.disqualified, r.created_at
        FROM runs r
        JOIN users u ON r.user_id = u.id
        JOIN drink_types dt ON r.drink_type_id = dt.id
        {where_clause}
        ORDER BY r.track_time ASC
        LIMIT 100
        "#
    );

    let rows = RunDetailRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        &sql,
        params,
    ))
    .all(db)
    .await?;

    rows.into_iter()
        .map(RunDetailRow::try_into_detail)
        .collect()
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
    let run = runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Run not found".to_string()))?;

    // Lift the entity-layer `String` to a typed `UserId` and compare in the
    // domain. Stays consistent with `session_context.rs::require_host` and
    // surfaces a corrupt-UUID-in-DB as 500 instead of a silent false-negative
    // compare (FK-protected so it should never fire, but the failure mode is
    // worth surfacing if it ever does).
    let owner = UserId::from_db(&run.user_id)?;
    if owner != *user_id {
        return Err(Error::Forbidden(
            "Only the run's owner can delete it".to_string(),
        ));
    }

    // Check that the session is still active
    let session_race = session_races::Entity::find_by_id(&run.session_race_id)
        .one(db)
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
            Error::Conflict { .. } => Error::conflict("Cannot delete run from a closed session"),
            other => other,
        })?;

    let txn = db.begin().await?;

    run.delete(&txn).await?;

    helpers::touch_session(&txn, &session_id).await?;

    txn.commit().await?;

    Ok(())
}

/// Get run defaults for pre-filling the run entry form.
///
/// Cascade: previous run → user preferences → none.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(user_id = %user_id))]
pub async fn get_run_defaults(
    db: &impl ConnectionTrait,
    user_id: &UserId,
) -> Result<RunDefaults, Error> {
    // Try most recent run
    let latest_run = runs::Entity::find()
        .filter(runs::Column::UserId.eq(user_id))
        .order_by_desc(runs::Column::CreatedAt)
        .one(db)
        .await?;

    if let Some(run) = latest_run {
        return Ok(RunDefaults {
            drink_type_id: Some(DrinkTypeId::from_db(&run.drink_type_id)?),
            character_id: Some(CharacterId::new(run.character_id)),
            body_id: Some(BodyId::new(run.body_id)),
            wheel_id: Some(WheelId::new(run.wheel_id)),
            glider_id: Some(GliderId::new(run.glider_id)),
            source: "previous_run".to_string(),
        });
    }

    // Fall back to user preferences
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("User not found".to_string()))?;

    if user.preferred_character_id.is_some() || user.preferred_drink_type_id.is_some() {
        return Ok(RunDefaults {
            drink_type_id: user
                .preferred_drink_type_id
                .as_deref()
                .map(DrinkTypeId::from_db)
                .transpose()?,
            character_id: user.preferred_character_id.map(CharacterId::new),
            body_id: user.preferred_body_id.map(BodyId::new),
            wheel_id: user.preferred_wheel_id.map(WheelId::new),
            glider_id: user.preferred_glider_id.map(GliderId::new),
            source: "preferences".to_string(),
        });
    }

    Ok(RunDefaults {
        drink_type_id: None,
        character_id: None,
        body_id: None,
        wheel_id: None,
        glider_id: None,
        source: "none".to_string(),
    })
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
        crate::drink_type_id::drink_type_uuid("Test Beer")
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
    async fn test_list_runs_filters_by_session_race_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (session_id, race1_id) = setup_session_with_race(&db, &host_id).await;

        create_run(&db, &host_id, valid_run_request(&race1_id))
            .await
            .unwrap();

        // Create a second race
        let race2 = sessions::next_track(&db, &session_id, &host_id)
            .await
            .unwrap();
        // Need a second user for race2 since host already submitted for race1
        // Actually, host can submit for race2 too (different session_race_id)
        create_run(&db, &host_id, valid_run_request(&race2.id))
            .await
            .unwrap();

        let filtered = list_runs(
            &db,
            RunFilters {
                session_race_id: Some(race1_id),
                user_id: None,
                track_id: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(filtered.len(), 1);
    }

    #[tokio::test]
    async fn test_list_runs_ordered_by_time_ascending() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;
        let (session_id, race_id) = setup_session_with_race(&db, &host_id).await;
        sessions::join_session(&db, &session_id, &user_id)
            .await
            .unwrap();

        // Host submits slower time
        let mut slow = valid_run_request(&race_id);
        slow.track_time = 150_000;
        slow.lap1_time = 50_000;
        slow.lap2_time = 50_000;
        slow.lap3_time = 50_000;
        create_run(&db, &host_id, slow).await.unwrap();

        // User submits faster time
        let mut fast = valid_run_request(&race_id);
        fast.track_time = 100_000;
        fast.lap1_time = 33_000;
        fast.lap2_time = 33_000;
        fast.lap3_time = 34_000;
        create_run(&db, &user_id, fast).await.unwrap();

        let runs = list_runs(
            &db,
            RunFilters {
                session_race_id: Some(race_id),
                user_id: None,
                track_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].track_time, 100_000); // fastest first
        assert_eq!(runs[1].track_time, 150_000);
    }

    #[tokio::test]
    async fn test_get_run_defaults_returns_previous_run_data() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();

        let defaults = get_run_defaults(&db, &host_id).await.unwrap();
        assert_eq!(defaults.source, "previous_run");
        assert_eq!(defaults.character_id, Some(CharacterId::new(1)));
        assert_eq!(defaults.drink_type_id, Some(test_drink_id()));
    }

    #[tokio::test]
    async fn test_get_run_defaults_falls_back_to_preferences() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;

        // Set preferences on the user
        let user = users::Entity::find_by_id(host_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let mut active: users::ActiveModel = user.into();
        active.preferred_character_id = Set(Some(1));
        active.preferred_body_id = Set(Some(1));
        active.preferred_wheel_id = Set(Some(1));
        active.preferred_glider_id = Set(Some(1));
        active.preferred_drink_type_id = Set(Some(test_drink_id().into()));
        active.update(&db).await.unwrap();

        let defaults = get_run_defaults(&db, &host_id).await.unwrap();
        assert_eq!(defaults.source, "preferences");
        assert_eq!(defaults.character_id, Some(CharacterId::new(1)));
    }

    #[tokio::test]
    async fn test_get_run_defaults_returns_none_when_no_data() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;

        let defaults = get_run_defaults(&db, &host_id).await.unwrap();
        assert_eq!(defaults.source, "none");
        assert!(defaults.character_id.is_none());
        assert!(defaults.drink_type_id.is_none());
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

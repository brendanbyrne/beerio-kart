//! Run reads: single fetch, list with filters, run-entry defaults.
//!
//! Holds the response DTOs ([`RunDetail`], [`RunDefaults`]) and the read
//! functions that return them. The write counterparts ([`super::submission::create_run`],
//! [`super::submission::delete_run`]) live in [`super::submission`].

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ColumnTrait, ConnectionTrait, EntityTrait, FromQueryResult, QueryFilter, QueryOrder,
};
use serde::Serialize;

use crate::{
    domain::{
        BodyId, CharacterId, DrinkTypeId, GliderId, RunId, SessionRaceId, TrackId, UserId,
        Username, WheelId,
    },
    entities::{runs, users},
    error::Error,
};

/// Full wire view of a single run — JOIN-expanded for username and drink-type
/// name so the frontend doesn't need follow-up requests.
#[derive(Serialize)]
pub struct RunDetail {
    /// Stable UUID of the run row.
    pub id: RunId,
    /// User who submitted the run.
    pub user_id: UserId,
    /// Cached username for display (saves a JOIN on the read path).
    pub username: Username,
    /// Race this run belongs to.
    pub session_race_id: SessionRaceId,
    /// Track this run was raced on.
    pub track_id: TrackId,
    /// Total time in milliseconds.
    pub track_time: i32,
    /// Lap 1 time in milliseconds.
    pub lap1_time: i32,
    /// Lap 2 time in milliseconds.
    pub lap2_time: i32,
    /// Lap 3 time in milliseconds.
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
    /// Cached drink-type display name (saves a JOIN on the read path).
    pub drink_type_name: String,
    /// `true` if the run was self-reported DQ.
    pub disqualified: bool,
    /// Submission timestamp, UTC.
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

/// Pre-fill values for the run-recording UI. Tries the user's last run first;
/// falls back to their saved profile preferences if they've never raced.
#[derive(Serialize)]
pub struct RunDefaults {
    /// Suggested drink type, or `None` if no history and no profile preference.
    pub drink_type_id: Option<DrinkTypeId>,
    /// Suggested character.
    pub character_id: Option<CharacterId>,
    /// Suggested kart body.
    pub body_id: Option<BodyId>,
    /// Suggested wheels.
    pub wheel_id: Option<WheelId>,
    /// Suggested glider.
    pub glider_id: Option<GliderId>,
    /// Which source produced these defaults — `"last_run"` or `"profile"`.
    pub source: String,
}

/// Optional filters passed to [`list_runs`]. All filters AND together;
/// omitting a field doesn't constrain the result.
pub struct RunFilters {
    /// Only runs from this race.
    pub session_race_id: Option<SessionRaceId>,
    /// Only runs submitted by this user.
    pub user_id: Option<UserId>,
    /// Only runs on this track.
    pub track_id: Option<TrackId>,
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
    use sea_orm::{ActiveModelTrait, DatabaseConnection, Set};

    use super::*;
    use crate::{
        domain::SessionId,
        services::{
            runs::{CreateRunRequest, create_run},
            sessions,
        },
        test_helpers::{create_user, seed_game_data, setup_db},
    };

    fn test_drink_id() -> DrinkTypeId {
        crate::drink_type_id::drink_type_uuid("Test Beer")
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
}

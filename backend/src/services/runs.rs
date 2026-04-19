use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, ModelTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::entities::{
    bodies, characters, drink_types, gliders, runs, session_races, users, wheels,
};
use crate::error::AppError;
use crate::services::helpers;

/// Maximum allowed track time: 10 minutes in milliseconds.
const MAX_TRACK_TIME_MS: i32 = 600_000;

#[derive(Deserialize)]
pub struct CreateRunRequest {
    pub session_race_id: String,
    pub track_time: i32,
    pub lap1_time: i32,
    pub lap2_time: i32,
    pub lap3_time: i32,
    pub character_id: i32,
    pub body_id: i32,
    pub wheel_id: i32,
    pub glider_id: i32,
    pub drink_type_id: String,
    pub disqualified: bool,
}

#[derive(Serialize)]
pub struct RunDetail {
    pub id: String,
    pub user_id: String,
    pub username: String,
    pub session_race_id: String,
    pub track_id: i32,
    pub track_time: i32,
    pub lap1_time: i32,
    pub lap2_time: i32,
    pub lap3_time: i32,
    pub character_id: i32,
    pub body_id: i32,
    pub wheel_id: i32,
    pub glider_id: i32,
    pub drink_type_id: String,
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

impl From<RunDetailRow> for RunDetail {
    fn from(r: RunDetailRow) -> Self {
        Self {
            id: r.id,
            user_id: r.user_id,
            username: r.username,
            session_race_id: r.session_race_id,
            track_id: r.track_id,
            track_time: r.track_time,
            lap1_time: r.lap1_time,
            lap2_time: r.lap2_time,
            lap3_time: r.lap3_time,
            character_id: r.character_id,
            body_id: r.body_id,
            wheel_id: r.wheel_id,
            glider_id: r.glider_id,
            drink_type_id: r.drink_type_id,
            drink_type_name: r.drink_type_name,
            disqualified: r.disqualified,
            created_at: r.created_at.and_utc(),
        }
    }
}

#[derive(Serialize)]
pub struct RunDefaults {
    pub drink_type_id: Option<String>,
    pub character_id: Option<i32>,
    pub body_id: Option<i32>,
    pub wheel_id: Option<i32>,
    pub glider_id: Option<i32>,
    pub source: String,
}

pub struct RunFilters {
    pub session_race_id: Option<String>,
    pub user_id: Option<String>,
    pub track_id: Option<i32>,
}

/// Create a run for a session race. Validates all inputs before inserting.
pub async fn create_run(
    db: &DatabaseConnection,
    user_id: &str,
    body: CreateRunRequest,
) -> Result<RunDetail, AppError> {
    // Validate time fields
    if body.track_time <= 0 || body.track_time > MAX_TRACK_TIME_MS {
        return Err(AppError::BadRequest(format!(
            "track_time must be between 1 and {MAX_TRACK_TIME_MS} ms"
        )));
    }
    if body.lap1_time <= 0 || body.lap2_time <= 0 || body.lap3_time <= 0 {
        return Err(AppError::BadRequest(
            "All lap times must be positive".to_string(),
        ));
    }
    if body.lap1_time > MAX_TRACK_TIME_MS
        || body.lap2_time > MAX_TRACK_TIME_MS
        || body.lap3_time > MAX_TRACK_TIME_MS
    {
        return Err(AppError::BadRequest(format!(
            "Each lap time must be at most {MAX_TRACK_TIME_MS} ms"
        )));
    }

    // Lap times must sum exactly to track_time
    let lap_sum = body.lap1_time + body.lap2_time + body.lap3_time;
    if lap_sum != body.track_time {
        let diff = (lap_sum - body.track_time).abs();
        return Err(AppError::BadRequest(format!(
            "Lap times must add up to total time (off by {diff}ms)"
        )));
    }

    // Validate session_race exists and belongs to an active session
    let session_race = session_races::Entity::find_by_id(&body.session_race_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session race not found".to_string()))?;

    helpers::load_active_session(db, &session_race.session_id)
        .await
        .map_err(|e| match e {
            AppError::Conflict(_) => {
                AppError::Conflict("Cannot submit run for a closed session".to_string())
            }
            other => other,
        })?;
    helpers::require_active_participant(db, &session_race.session_id, user_id).await?;

    // Check for duplicate submission
    let existing = runs::Entity::find()
        .filter(
            Condition::all()
                .add(runs::Column::SessionRaceId.eq(&body.session_race_id))
                .add(runs::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(AppError::Conflict(
            "Already submitted a run for this race".to_string(),
        ));
    }

    // Validate FK references exist
    helpers::require_exists::<characters::Entity, _>(db, body.character_id, "character").await?;
    helpers::require_exists::<bodies::Entity, _>(db, body.body_id, "body").await?;
    helpers::require_exists::<wheels::Entity, _>(db, body.wheel_id, "wheel").await?;
    helpers::require_exists::<gliders::Entity, _>(db, body.glider_id, "glider").await?;
    helpers::require_exists::<drink_types::Entity, _>(db, body.drink_type_id.clone(), "drink_type")
        .await?;

    let now = Utc::now().naive_utc();
    let run_id = Uuid::new_v4().to_string();

    let txn = db.begin().await?;

    runs::ActiveModel {
        id: Set(run_id.clone()),
        user_id: Set(user_id.to_string()),
        session_race_id: Set(body.session_race_id.clone()),
        track_id: Set(session_race.track_id),
        character_id: Set(body.character_id),
        body_id: Set(body.body_id),
        wheel_id: Set(body.wheel_id),
        glider_id: Set(body.glider_id),
        track_time: Set(body.track_time),
        lap1_time: Set(body.lap1_time),
        lap2_time: Set(body.lap2_time),
        lap3_time: Set(body.lap3_time),
        drink_type_id: Set(body.drink_type_id),
        disqualified: Set(body.disqualified),
        photo_path: Set(None),
        created_at: Set(now),
        notes: Set(None),
    }
    .insert(&txn)
    .await?;

    helpers::touch_session(&txn, &session_race.session_id).await?;

    txn.commit().await?;

    get_run(db, &run_id).await
}

/// Fetch a single run by ID with JOINed username and drink_type_name.
pub async fn get_run(db: &DatabaseConnection, run_id: &str) -> Result<RunDetail, AppError> {
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
    .ok_or_else(|| AppError::NotFound("Run not found".to_string()))?;

    Ok(row.into())
}

/// List runs with optional filters, ordered by track_time ASC.
pub async fn list_runs(
    db: &DatabaseConnection,
    filters: RunFilters,
) -> Result<Vec<RunDetail>, AppError> {
    let mut conditions = Vec::new();
    let mut params: Vec<sea_orm::Value> = Vec::new();

    let mut add_filter = |column: &str, value: sea_orm::Value| {
        let idx = params.len() + 1;
        conditions.push(format!("{column} = ${idx}"));
        params.push(value);
    };

    if let Some(ref sr_id) = filters.session_race_id {
        add_filter("r.session_race_id", sr_id.clone().into());
    }
    if let Some(ref uid) = filters.user_id {
        add_filter("r.user_id", uid.clone().into());
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

    Ok(rows.into_iter().map(RunDetail::from).collect())
}

/// Delete a run. Only the run's owner can delete, and the session must be active.
pub async fn delete_run(
    db: &DatabaseConnection,
    run_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    let run = runs::Entity::find_by_id(run_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Run not found".to_string()))?;

    if run.user_id != user_id {
        return Err(AppError::Forbidden(
            "Only the run's owner can delete it".to_string(),
        ));
    }

    // Check that the session is still active
    let session_race = session_races::Entity::find_by_id(&run.session_race_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::Internal("Session race not found for run".to_string()))?;

    // FK guarantees the session exists; NotFound here signals data corruption.
    helpers::load_active_session(db, &session_race.session_id)
        .await
        .map_err(|e| match e {
            AppError::NotFound(_) => AppError::Internal("Session not found for run".to_string()),
            AppError::Conflict(_) => {
                AppError::Conflict("Cannot delete run from a closed session".to_string())
            }
            other => other,
        })?;

    let txn = db.begin().await?;

    run.delete(&txn).await?;

    helpers::touch_session(&txn, &session_race.session_id).await?;

    txn.commit().await?;

    Ok(())
}

/// Get run defaults for pre-filling the run entry form.
/// Cascade: previous run → user preferences → none.
pub async fn get_run_defaults(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<RunDefaults, AppError> {
    // Try most recent run
    let latest_run = runs::Entity::find()
        .filter(runs::Column::UserId.eq(user_id))
        .order_by_desc(runs::Column::CreatedAt)
        .one(db)
        .await?;

    if let Some(run) = latest_run {
        return Ok(RunDefaults {
            drink_type_id: Some(run.drink_type_id),
            character_id: Some(run.character_id),
            body_id: Some(run.body_id),
            wheel_id: Some(run.wheel_id),
            glider_id: Some(run.glider_id),
            source: "previous_run".to_string(),
        });
    }

    // Fall back to user preferences
    let user = users::Entity::find_by_id(user_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".to_string()))?;

    if user.preferred_character_id.is_some() || user.preferred_drink_type_id.is_some() {
        return Ok(RunDefaults {
            drink_type_id: user.preferred_drink_type_id,
            character_id: user.preferred_character_id,
            body_id: user.preferred_body_id,
            wheel_id: user.preferred_wheel_id,
            glider_id: user.preferred_glider_id,
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
    use crate::services::sessions;
    use crate::test_helpers::{create_user, seed_game_data, setup_db};

    fn test_drink_id() -> String {
        crate::drink_type_id::drink_type_uuid("Test Beer")
    }

    fn valid_run_request(session_race_id: &str) -> CreateRunRequest {
        CreateRunRequest {
            session_race_id: session_race_id.to_string(),
            track_time: 120_000,
            lap1_time: 40_000,
            lap2_time: 39_000,
            lap3_time: 41_000,
            character_id: 1,
            body_id: 1,
            wheel_id: 1,
            glider_id: 1,
            drink_type_id: test_drink_id(),
            disqualified: false,
        }
    }

    /// Helper: create session, pick a track, return (session_id, session_race_id)
    async fn setup_session_with_race(db: &DatabaseConnection, host_id: &str) -> (String, String) {
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
        assert_eq!(run.username, "host");
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
        req.character_id = 999;
        assert!(create_run(&db, &host_id, req).await.is_err());
    }

    #[tokio::test]
    async fn test_create_run_fails_with_invalid_drink_type_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let (_, race_id) = setup_session_with_race(&db, &host_id).await;

        let mut req = valid_run_request(&race_id);
        req.drink_type_id = "nonexistent-uuid".to_string();
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
        use crate::entities::sessions as sessions_entity;
        let s = sessions_entity::Entity::find_by_id(&session_id)
            .one(&db)
            .await
            .unwrap();
        if let Some(s) = s {
            let mut active: sessions_entity::ActiveModel = s.into();
            active.status = Set("closed".to_string());
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
        assert_eq!(defaults.character_id, Some(1));
        assert_eq!(defaults.drink_type_id, Some(test_drink_id()));
    }

    #[tokio::test]
    async fn test_get_run_defaults_falls_back_to_preferences() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;

        // Set preferences on the user
        let user = users::Entity::find_by_id(&host_id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let mut active: users::ActiveModel = user.into();
        active.preferred_character_id = Set(Some(1));
        active.preferred_body_id = Set(Some(1));
        active.preferred_wheel_id = Set(Some(1));
        active.preferred_glider_id = Set(Some(1));
        active.preferred_drink_type_id = Set(Some(test_drink_id()));
        active.update(&db).await.unwrap();

        let defaults = get_run_defaults(&db, &host_id).await.unwrap();
        assert_eq!(defaults.source, "preferences");
        assert_eq!(defaults.character_id, Some(1));
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
        let detail = sessions::get_session_detail(&db, &session_id)
            .await
            .unwrap();
        let current = detail.current_race.unwrap();
        assert!(current.submissions.is_empty());

        // Submit a run
        create_run(&db, &host_id, valid_run_request(&race_id))
            .await
            .unwrap();

        // After submitting
        let detail = sessions::get_session_detail(&db, &session_id)
            .await
            .unwrap();
        let current = detail.current_race.unwrap();
        assert_eq!(current.submissions.len(), 1);
        assert_eq!(current.submissions[0].username, "host");
        assert_eq!(current.submissions[0].track_time, 120_000);
        assert!(!current.submissions[0].disqualified);
    }
}

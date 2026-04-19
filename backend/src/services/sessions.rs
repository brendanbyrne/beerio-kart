use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
    sea_query::Expr,
};
use uuid::Uuid;

use crate::domain::enums::{Ruleset, SessionStatus};
use crate::entities::{cups, runs, session_participants, session_races, sessions, users};
use crate::error::AppError;
use crate::services::helpers;

/// Row shape for the active-participant-in-active-session query.
#[derive(Debug, FromQueryResult)]
struct ActiveParticipantRow {
    session_id: String,
}

/// Check that the user is not already active in any *active* session.
/// Returns an error with the existing session ID if they are.
/// JOINs sessions to ensure closed sessions don't block the user.
async fn check_not_in_any_session(
    db: &impl ConnectionTrait,
    user_id: &str,
) -> Result<(), AppError> {
    let existing =
        ActiveParticipantRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
            SELECT sp.session_id
            FROM session_participants sp
            JOIN sessions s ON sp.session_id = s.id
            WHERE sp.user_id = $1
              AND sp.left_at IS NULL
              AND s.status = 'active'
            LIMIT 1
            "#,
            [user_id.into()],
        ))
        .one(db)
        .await?;

    if let Some(row) = existing {
        return Err(AppError::Conflict(format!(
            "Already in session {}",
            row.session_id
        )));
    }

    Ok(())
}

/// Returns the session ID the user is currently active in, or None.
/// Only considers active sessions (not closed/stale ones).
pub async fn get_active_session_id(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Option<String>, AppError> {
    let row = ActiveParticipantRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sp.session_id
        FROM session_participants sp
        JOIN sessions s ON sp.session_id = s.id
        WHERE sp.user_id = $1
          AND sp.left_at IS NULL
          AND s.status = 'active'
        LIMIT 1
        "#,
        [user_id.into()],
    ))
    .one(db)
    .await?;

    Ok(row.map(|r| r.session_id))
}

/// Create a new session. The creator becomes both the host and the first
/// participant. Returns the full session detail.
pub async fn create_session(
    db: &DatabaseConnection,
    user_id: &str,
    ruleset: &str,
) -> Result<SessionDetail, AppError> {
    let parsed: Ruleset = ruleset.parse()?;

    check_not_in_any_session(db, user_id).await?;

    let now = Utc::now().naive_utc();
    let session_id = Uuid::new_v4().to_string();

    let txn = db.begin().await?;

    sessions::ActiveModel {
        id: Set(session_id.clone()),
        created_by: Set(user_id.to_string()),
        host_id: Set(user_id.to_string()),
        ruleset: Set(parsed.to_string()),
        least_played_drink_category: Set(None),
        status: Set(SessionStatus::Active.to_string()),
        created_at: Set(now),
        last_activity_at: Set(now),
    }
    .insert(&txn)
    .await?;

    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.clone()),
        user_id: Set(user_id.to_string()),
        joined_at: Set(now),
        left_at: Set(None),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;

    get_session_detail(db, &session_id).await
}

/// Summary info for listing active sessions.
#[derive(serde::Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub host_username: String,
    pub participant_count: i64,
    pub race_number: i64,
    pub ruleset: String,
    pub last_activity_at: DateTime<Utc>,
}

/// Row shape returned by the list-sessions JOIN query.
#[derive(Debug, FromQueryResult)]
struct SessionSummaryRow {
    id: String,
    host_username: String,
    participant_count: i64,
    race_count: i64,
    ruleset: String,
    last_activity_at: NaiveDateTime,
}

/// List active sessions sorted by last_activity_at DESC.
/// Uses a single JOIN query instead of N+1 queries.
pub async fn list_active_sessions(
    db: &DatabaseConnection,
) -> Result<Vec<SessionSummary>, AppError> {
    let rows = SessionSummaryRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT
            s.id,
            u.username AS host_username,
            COUNT(DISTINCT CASE WHEN sp.left_at IS NULL THEN sp.id END) AS participant_count,
            COUNT(DISTINCT sr.id) AS race_count,
            s.ruleset,
            s.last_activity_at
        FROM sessions s
        JOIN users u ON s.host_id = u.id
        LEFT JOIN session_participants sp ON sp.session_id = s.id
        LEFT JOIN session_races sr ON sr.session_id = s.id
        WHERE s.status = 'active'
        GROUP BY s.id
        ORDER BY s.last_activity_at DESC
        "#,
        [],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| SessionSummary {
            id: r.id,
            host_username: r.host_username,
            participant_count: r.participant_count,
            race_number: r.race_count.max(1),
            ruleset: r.ruleset,
            last_activity_at: r.last_activity_at.and_utc(),
        })
        .collect())
}

/// Participant info for the detail response.
#[derive(serde::Serialize)]
pub struct ParticipantInfo {
    pub user_id: String,
    pub username: String,
    pub joined_at: DateTime<Utc>,
    pub left_at: Option<DateTime<Utc>>,
}

/// Row shape returned by the participant JOIN query.
#[derive(Debug, FromQueryResult)]
struct ParticipantRow {
    user_id: String,
    username: String,
    joined_at: NaiveDateTime,
    left_at: Option<NaiveDateTime>,
}

/// Submission info for a single participant in a race.
#[derive(serde::Serialize, Clone)]
pub struct RaceSubmission {
    pub user_id: String,
    pub username: String,
    pub track_time: i32,
    pub disqualified: bool,
}

/// Row shape for the submissions query.
#[derive(Debug, FromQueryResult)]
struct SubmissionRow {
    user_id: String,
    username: String,
    track_time: i32,
    disqualified: bool,
}

/// Info about a single race in the session (returned on create / skip / poll).
#[derive(serde::Serialize, Clone)]
pub struct SessionRaceInfo {
    pub id: String,
    pub race_number: i32,
    pub track_id: i32,
    pub track_name: String,
    pub cup_name: String,
    pub image_path: String,
    pub created_at: DateTime<Utc>,
    pub submissions: Vec<RaceSubmission>,
}

/// Race info for the race history list.
#[derive(serde::Serialize)]
pub struct RaceInfo {
    pub id: String,
    pub race_number: i32,
    pub track_id: i32,
    pub track_name: String,
    pub cup_name: String,
    pub run_count: i64,
    pub created_at: DateTime<Utc>,
}

/// Row shape returned by the race history query.
#[derive(Debug, FromQueryResult)]
struct RaceHistoryRow {
    id: String,
    race_number: i32,
    track_id: i32,
    track_name: String,
    cup_name: String,
    run_count: i64,
    created_at: NaiveDateTime,
}

/// Row shape for the current race query.
#[derive(Debug, FromQueryResult)]
struct CurrentRaceRow {
    id: String,
    race_number: i32,
    track_id: i32,
    track_name: String,
    cup_name: String,
    image_path: String,
    created_at: NaiveDateTime,
}

/// Full session detail for polling.
#[derive(serde::Serialize)]
pub struct SessionDetail {
    pub id: String,
    pub created_by: String,
    pub host_id: String,
    pub host_username: String,
    pub ruleset: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub participants: Vec<ParticipantInfo>,
    pub race_number: usize,
    pub current_race: Option<SessionRaceInfo>,
    pub races: Vec<RaceInfo>,
}

// ── get_session_detail sub-queries ────────────────────────────────────

/// Look up the host's username. Returns `Internal` if the FK-referenced
/// user row is missing (data corruption — FKs should prevent this).
async fn load_host_username(db: &impl ConnectionTrait, host_id: &str) -> Result<String, AppError> {
    users::Entity::find_by_id(host_id)
        .one(db)
        .await?
        .map(|u| u.username)
        .ok_or_else(|| AppError::Internal(format!("Host user not found for host_id {host_id}")))
}

/// Fetch all participants with usernames in a single JOIN query.
async fn load_participants(
    db: &impl ConnectionTrait,
    session_id: &str,
) -> Result<Vec<ParticipantInfo>, AppError> {
    let rows = ParticipantRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sp.user_id, u.username, sp.joined_at, sp.left_at
        FROM session_participants sp
        JOIN users u ON sp.user_id = u.id
        WHERE sp.session_id = $1
        ORDER BY sp.joined_at ASC
        "#,
        [session_id.into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| ParticipantInfo {
            user_id: r.user_id,
            username: r.username,
            joined_at: r.joined_at.and_utc(),
            left_at: r.left_at.map(|t| t.and_utc()),
        })
        .collect())
}

/// Fetch the most recent race with its submissions. Returns `None` if
/// no races have been created yet.
async fn load_current_race_with_submissions(
    db: &impl ConnectionTrait,
    session_id: &str,
) -> Result<Option<SessionRaceInfo>, AppError> {
    let race_row = CurrentRaceRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sr.id, sr.race_number, sr.track_id,
               t.name AS track_name, c.name AS cup_name,
               t.image_path, sr.created_at
        FROM session_races sr
        JOIN tracks t ON sr.track_id = t.id
        JOIN cups c ON t.cup_id = c.id
        WHERE sr.session_id = $1
        ORDER BY sr.race_number DESC
        LIMIT 1
        "#,
        [session_id.into()],
    ))
    .one(db)
    .await?;

    let Some(row) = race_row else {
        return Ok(None);
    };

    let submissions = SubmissionRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT r.user_id, u.username, r.track_time, r.disqualified
        FROM runs r
        JOIN users u ON r.user_id = u.id
        WHERE r.session_race_id = $1
        ORDER BY r.track_time ASC
        "#,
        [row.id.clone().into()],
    ))
    .all(db)
    .await?
    .into_iter()
    .map(|s| RaceSubmission {
        user_id: s.user_id,
        username: s.username,
        track_time: s.track_time,
        disqualified: s.disqualified,
    })
    .collect();

    Ok(Some(SessionRaceInfo {
        id: row.id,
        race_number: row.race_number,
        track_id: row.track_id,
        track_name: row.track_name,
        cup_name: row.cup_name,
        image_path: row.image_path,
        created_at: row.created_at.and_utc(),
        submissions,
    }))
}

/// Fetch all races in a session with run counts, ordered by race_number ASC.
async fn load_race_history(
    db: &impl ConnectionTrait,
    session_id: &str,
) -> Result<Vec<RaceInfo>, AppError> {
    let rows = RaceHistoryRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sr.id, sr.race_number, sr.track_id,
               t.name AS track_name, c.name AS cup_name,
               COUNT(r.id) AS run_count, sr.created_at
        FROM session_races sr
        JOIN tracks t ON sr.track_id = t.id
        JOIN cups c ON t.cup_id = c.id
        LEFT JOIN runs r ON r.session_race_id = sr.id
        WHERE sr.session_id = $1
        GROUP BY sr.id
        ORDER BY sr.race_number ASC
        "#,
        [session_id.into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RaceInfo {
            id: r.id,
            race_number: r.race_number,
            track_id: r.track_id,
            track_name: r.track_name,
            cup_name: r.cup_name,
            run_count: r.run_count,
            created_at: r.created_at.and_utc(),
        })
        .collect())
}

/// Get full session detail — the polling endpoint.
pub async fn get_session_detail(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<SessionDetail, AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let host_username = load_host_username(db, &session.host_id).await?;
    let participants = load_participants(db, session_id).await?;
    let current_race = load_current_race_with_submissions(db, session_id).await?;
    let races = load_race_history(db, session_id).await?;

    // Derive race_number from history instead of a separate COUNT query —
    // saves one DB round trip on every poll. Safe because race numbers are
    // 1-indexed and gapless: `next_track` appends monotonically, and
    // `skip_turn` replaces in-place (preserves race_number). No deletion
    // path exists. Under this invariant, last().race_number == COUNT(*).
    let race_number = races.last().map(|r| r.race_number as usize).unwrap_or(1);

    Ok(SessionDetail {
        id: session.id,
        created_by: session.created_by,
        host_id: session.host_id,
        host_username,
        ruleset: session.ruleset,
        status: session.status,
        created_at: session.created_at.and_utc(),
        last_activity_at: session.last_activity_at.and_utc(),
        participants,
        race_number,
        current_race,
        races,
    })
}

/// Join a session. Creates a new participant row.
pub async fn join_session(
    db: &DatabaseConnection,
    session_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    helpers::load_active_session(db, session_id)
        .await
        .map_err(|e| match e {
            AppError::Conflict(_) => AppError::Conflict("Cannot join a closed session".to_string()),
            other => other,
        })?;
    check_not_in_any_session(db, user_id).await?;

    let now = Utc::now().naive_utc();
    let txn = db.begin().await?;

    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.to_string()),
        user_id: Set(user_id.to_string()),
        joined_at: Set(now),
        left_at: Set(None),
    }
    .insert(&txn)
    .await?;

    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    Ok(())
}

/// What should happen to the session after a participant leaves.
#[derive(Debug, PartialEq, Eq)]
enum HostDisposition {
    /// Host role transferred to the given user ID.
    TransferredTo(String),
    /// No active participants remain — session should close.
    SessionClosed,
    /// The leaver wasn't the host and participants remain — no change needed.
    NoChange,
}

/// Decide what happens to the host role when a participant leaves.
///
/// If the host is leaving, pick the earliest-joined remaining participant.
/// If no one remains, close the session. If a non-host leaves but no active
/// participants remain, also close.
///
/// **Precondition:** the leaving user's `left_at` must already be set within
/// the same transaction before calling this function. The non-host branch
/// counts active participants via `left_at IS NULL`, and relies on the
/// leaver's row being excluded by the prior update.
async fn transfer_host_or_close(
    txn: &impl ConnectionTrait,
    session_id: &str,
    leaving_user_id: &str,
    is_host_leaving: bool,
) -> Result<HostDisposition, AppError> {
    if is_host_leaving {
        let next_host = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::UserId.ne(leaving_user_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .order_by_asc(session_participants::Column::JoinedAt)
            .one(txn)
            .await?;

        match next_host {
            Some(new_host) => Ok(HostDisposition::TransferredTo(new_host.user_id)),
            None => Ok(HostDisposition::SessionClosed),
        }
    } else {
        let remaining = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .count(txn)
            .await?;

        if remaining == 0 {
            Ok(HostDisposition::SessionClosed)
        } else {
            Ok(HostDisposition::NoChange)
        }
    }
}

/// Leave a session. Sets left_at and handles host transfer.
pub async fn leave_session(
    db: &DatabaseConnection,
    session_id: &str,
    user_id: &str,
) -> Result<(), AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    // require_active_participant returns Forbidden (authorization guard), but
    // leaving a session you're not in is bad input, not an auth failure.
    let participant = helpers::require_active_participant(db, session_id, user_id)
        .await
        .map_err(|_| AppError::BadRequest("Not currently in this session".to_string()))?;

    let now = Utc::now().naive_utc();
    let txn = db.begin().await?;

    let mut active_participant: session_participants::ActiveModel = participant.into();
    active_participant.left_at = Set(Some(now));
    active_participant.update(&txn).await?;

    let mut active_session: sessions::ActiveModel = session.clone().into();
    let disposition =
        transfer_host_or_close(&txn, session_id, user_id, session.host_id == user_id).await?;

    match disposition {
        HostDisposition::TransferredTo(new_host_id) => {
            active_session.host_id = Set(new_host_id);
        }
        HostDisposition::SessionClosed => {
            active_session.status = Set(SessionStatus::Closed.to_string());
        }
        HostDisposition::NoChange => {}
    }

    active_session.last_activity_at = Set(now);
    active_session.update(&txn).await?;

    txn.commit().await?;

    Ok(())
}

/// Pick the next track for a session. Host-only.
/// Randomly selects from tracks not yet used in this session.
/// If all tracks have been used, resets the pool.
pub async fn next_track(
    db: &DatabaseConnection,
    session_id: &str,
    user_id: &str,
) -> Result<SessionRaceInfo, AppError> {
    use crate::services::session_context::SessionContext;

    let ctx = SessionContext::load_active(db, session_id).await?;
    ctx.require_host(user_id)?;

    // Get already-used track IDs
    let used_races = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .all(db)
        .await?;
    let race_count = used_races.len() as i32;
    let used_track_ids: Vec<i32> = used_races.iter().map(|r| r.track_id).collect();

    let chosen = helpers::pick_random_track(db, &used_track_ids, &[]).await?;

    let now = Utc::now().naive_utc();
    let race_id = Uuid::new_v4().to_string();
    let new_race_number = race_count + 1;

    let txn = db.begin().await?;

    session_races::ActiveModel {
        id: Set(race_id.clone()),
        session_id: Set(session_id.to_string()),
        race_number: Set(new_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: Set(now),
    }
    .insert(&txn)
    .await?;

    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    // Look up cup name for the response (FK-protected — missing is corruption)
    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::Internal(format!("Cup not found for cup_id {}", chosen.cup_id)))?
        .name;

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: new_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: chosen.image_path.clone(),
        created_at: now.and_utc(),
        submissions: Vec::new(),
    })
}

/// Re-roll the current track. Any participant can trigger this
/// (per DESIGN.md — "any participant can pass the chooser's turn").
/// Only valid if the most recent race has no runs submitted.
/// Deletes the current race and picks a new one in a single transaction,
/// excluding the skipped track from the pool so it can't come back.
pub async fn skip_turn(
    db: &DatabaseConnection,
    session_id: &str,
    _user_id: &str,
) -> Result<SessionRaceInfo, AppError> {
    helpers::load_active_session(db, session_id).await?;

    // Find the most recent race
    let current_race = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .order_by_desc(session_races::Column::RaceNumber)
        .one(db)
        .await?
        .ok_or_else(|| AppError::BadRequest("No track to skip".to_string()))?;

    // Verify no runs exist for this race
    let run_count = runs::Entity::find()
        .filter(runs::Column::SessionRaceId.eq(&current_race.id))
        .count(db)
        .await?;

    if run_count > 0 {
        return Err(AppError::BadRequest(
            "Can't skip — runs already submitted".to_string(),
        ));
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

    let now = Utc::now().naive_utc();
    let race_id = Uuid::new_v4().to_string();

    // Delete old race + insert new one + update activity in a single transaction
    let txn = db.begin().await?;

    current_race.delete(&txn).await?;

    session_races::ActiveModel {
        id: Set(race_id.clone()),
        session_id: Set(session_id.to_string()),
        race_number: Set(keep_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: Set(now),
    }
    .insert(&txn)
    .await?;

    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::Internal(format!("Cup not found for cup_id {}", chosen.cup_id)))?
        .name;

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: keep_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: chosen.image_path.clone(),
        created_at: now.and_utc(),
        submissions: Vec::new(),
    })
}

/// List all races in a session, ordered by race_number ASC.
pub async fn list_races(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<Vec<RaceInfo>, AppError> {
    // Verify session exists
    sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let rows = RaceHistoryRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
        db.get_database_backend(),
        r#"
        SELECT sr.id, sr.race_number, sr.track_id,
               t.name AS track_name, c.name AS cup_name,
               COUNT(r.id) AS run_count, sr.created_at
        FROM session_races sr
        JOIN tracks t ON sr.track_id = t.id
        JOIN cups c ON t.cup_id = c.id
        LEFT JOIN runs r ON r.session_race_id = sr.id
        WHERE sr.session_id = $1
        GROUP BY sr.id
        ORDER BY sr.race_number ASC
        "#,
        [session_id.into()],
    ))
    .all(db)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| RaceInfo {
            id: r.id,
            race_number: r.race_number,
            track_id: r.track_id,
            track_name: r.track_name,
            cup_name: r.cup_name,
            run_count: r.run_count,
            created_at: r.created_at.and_utc(),
        })
        .collect())
}

/// Close sessions that have had no activity for over an hour.
/// Also marks all remaining active participants as left, preventing
/// users from being soft-locked out of creating/joining new sessions.
/// Returns the number of sessions closed.
pub async fn close_stale_sessions(db: &DatabaseConnection) -> Result<u64, AppError> {
    let one_hour_ago = (Utc::now() - chrono::Duration::hours(1)).naive_utc();

    let stale = sessions::Entity::find()
        .filter(
            Condition::all()
                .add(sessions::Column::Status.eq(SessionStatus::Active.as_str()))
                .add(sessions::Column::LastActivityAt.lt(one_hour_ago)),
        )
        .all(db)
        .await?;

    let count = stale.len() as u64;
    let now = Utc::now().naive_utc();

    let txn = db.begin().await?;
    for session in stale {
        let session_id = session.id.clone();

        // Mark all still-active participants as left
        session_participants::Entity::update_many()
            .col_expr(session_participants::Column::LeftAt, Expr::value(now))
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .exec(&txn)
            .await?;

        let mut active: sessions::ActiveModel = session.into();
        active.status = Set(SessionStatus::Closed.to_string());
        active.update(&txn).await?;
    }
    txn.commit().await?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{
        create_user, insert_participant, insert_session, seed_tracks_for_test, setup_db,
    };

    // ── load_host_username ───────────────────────────────────────────

    #[tokio::test]
    async fn test_load_host_username_returns_username() {
        let db = setup_db().await;
        let user_id = create_user(&db, "alice").await;
        let username = load_host_username(&db, &user_id).await.unwrap();
        assert_eq!(username, "alice");
    }

    #[tokio::test]
    async fn test_load_host_username_missing_user_returns_internal() {
        let db = setup_db().await;
        let err = load_host_username(&db, "nonexistent-id").await.unwrap_err();
        assert!(matches!(err, AppError::Internal(_)));
    }

    // ── transfer_host_or_close ───────────────────────────────────────

    #[tokio::test]
    async fn test_transfer_host_leaves_with_successor() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let user2 = create_user(&db, "user2").await;
        let session_id = insert_session(&db, &host, "active").await;
        // host has already left (left_at set in the real flow before this call)
        insert_participant(&db, &session_id, &host, Some(Utc::now().naive_utc())).await;
        insert_participant(&db, &session_id, &user2, None).await;

        let result = transfer_host_or_close(&db, &session_id, &host, true)
            .await
            .unwrap();
        assert_eq!(result, HostDisposition::TransferredTo(user2));
    }

    #[tokio::test]
    async fn test_transfer_host_leaves_alone_closes_session() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host, "active").await;
        // host's participant row has left_at set
        insert_participant(&db, &session_id, &host, Some(Utc::now().naive_utc())).await;

        let result = transfer_host_or_close(&db, &session_id, &host, true)
            .await
            .unwrap();
        assert_eq!(result, HostDisposition::SessionClosed);
    }

    #[tokio::test]
    async fn test_transfer_non_host_leaves_with_others_remaining() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let user2 = create_user(&db, "user2").await;
        let session_id = insert_session(&db, &host, "active").await;
        insert_participant(&db, &session_id, &host, None).await;
        // user2 has already left (left_at set in the real flow)
        insert_participant(&db, &session_id, &user2, Some(Utc::now().naive_utc())).await;

        let result = transfer_host_or_close(&db, &session_id, &user2, false)
            .await
            .unwrap();
        assert_eq!(result, HostDisposition::NoChange);
    }

    #[tokio::test]
    async fn test_transfer_non_host_leaves_as_last_closes_session() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let user2 = create_user(&db, "user2").await;
        let session_id = insert_session(&db, &host, "active").await;
        // Both have left_at set (host left first, then user2)
        insert_participant(&db, &session_id, &host, Some(Utc::now().naive_utc())).await;
        insert_participant(&db, &session_id, &user2, Some(Utc::now().naive_utc())).await;

        let result = transfer_host_or_close(&db, &session_id, &user2, false)
            .await
            .unwrap();
        assert_eq!(result, HostDisposition::SessionClosed);
    }

    // ── existing tests ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_host_transfer_goes_to_earliest_joined_participant() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;
        let user3_id = create_user(&db, "user3").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();

        join_session(&db, &session.id, &user2_id).await.unwrap();
        join_session(&db, &session.id, &user3_id).await.unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let updated = sessions::Entity::find_by_id(&session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.host_id, user2_id);
        assert_eq!(updated.status, "active");
    }

    #[tokio::test]
    async fn test_host_transfer_closes_session_when_last_participant_leaves() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let updated = sessions::Entity::find_by_id(&session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, "closed");
    }

    #[tokio::test]
    async fn test_cannot_join_closed_session() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        leave_session(&db, &session.id, &host_id).await.unwrap();

        let result = join_session(&db, &session.id, &user2_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cannot_join_twice_while_active() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();

        let result = join_session(&db, &session.id, &user2_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_can_rejoin_after_leaving() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();
        leave_session(&db, &session.id, &user2_id).await.unwrap();

        let result = join_session(&db, &session.id, &user2_id).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_leave_sets_left_at_without_affecting_others() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();

        leave_session(&db, &session.id, &user2_id).await.unwrap();

        let user2_row = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&user2_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(user2_row.left_at.is_some());

        let host_row = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&host_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(host_row.left_at.is_none());
    }

    #[tokio::test]
    async fn test_create_with_invalid_ruleset_returns_error() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;

        let result = create_session(&db, &host_id, "invalid_ruleset").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_active_sessions_returns_correct_counts() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;
        let user3_id = create_user(&db, "user3").await;

        // host creates s1, user2 creates s2
        let s1 = create_session(&db, &host_id, "random").await.unwrap();
        let _s2 = create_session(&db, &user2_id, "random").await.unwrap();

        // user3 joins s1 (user3 is not in any session yet)
        join_session(&db, &s1.id, &user3_id).await.unwrap();

        let summaries = list_active_sessions(&db).await.unwrap();
        assert_eq!(summaries.len(), 2);

        let s1_summary = summaries.iter().find(|s| s.id == s1.id).unwrap();
        assert_eq!(s1_summary.participant_count, 2);
        assert_eq!(s1_summary.host_username, "host");
        assert_eq!(s1_summary.race_number, 1);

        let s2_summary = summaries.iter().find(|s| s.id != s1.id).unwrap();
        assert_eq!(s2_summary.participant_count, 1);
        assert_eq!(s2_summary.host_username, "user2");
    }

    #[tokio::test]
    async fn test_get_session_detail_returns_participants_with_usernames() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();

        let detail = get_session_detail(&db, &session.id).await.unwrap();
        assert_eq!(detail.participants.len(), 2);
        assert_eq!(detail.host_username, "host");
        assert_eq!(detail.race_number, 1);

        let usernames: Vec<&str> = detail
            .participants
            .iter()
            .map(|p| p.username.as_str())
            .collect();
        assert!(usernames.contains(&"host"));
        assert!(usernames.contains(&"user2"));
    }

    #[tokio::test]
    async fn test_cannot_join_another_session_while_active_in_one() {
        let db = setup_db().await;
        let host1_id = create_user(&db, "host1").await;
        let host2_id = create_user(&db, "host2").await;
        let user_id = create_user(&db, "user").await;

        let s1 = create_session(&db, &host1_id, "random").await.unwrap();
        let s2 = create_session(&db, &host2_id, "random").await.unwrap();

        join_session(&db, &s1.id, &user_id).await.unwrap();

        // Should fail — already active in s1
        let result = join_session(&db, &s2.id, &user_id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_cannot_create_session_while_active_in_one() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_id).await.unwrap();

        // Should fail — user is already active in a session
        let result = create_session(&db, &user_id, "random").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_can_join_new_session_after_leaving_previous() {
        let db = setup_db().await;
        let host1_id = create_user(&db, "host1").await;
        let host2_id = create_user(&db, "host2").await;
        let user_id = create_user(&db, "user").await;

        let s1 = create_session(&db, &host1_id, "random").await.unwrap();
        let s2 = create_session(&db, &host2_id, "random").await.unwrap();

        join_session(&db, &s1.id, &user_id).await.unwrap();
        leave_session(&db, &s1.id, &user_id).await.unwrap();

        // Should succeed — left s1, now free to join s2
        let result = join_session(&db, &s2.id, &user_id).await;
        assert!(result.is_ok());
    }

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
            .filter(session_races::Column::SessionId.eq(&session.id))
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
        let old_race = session_races::Entity::find_by_id(&original.id)
            .one(&db)
            .await
            .unwrap();
        assert!(old_race.is_none(), "Old race should be deleted");

        // New race should exist
        let new_race = session_races::Entity::find_by_id(&rerolled.id)
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
    async fn test_current_race_appears_in_session_detail() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // Before any track pick, current_race should be None
        let detail = get_session_detail(&db, &session.id).await.unwrap();
        assert!(detail.current_race.is_none());

        // After picking a track, current_race should be populated
        let race = next_track(&db, &session.id, &host_id).await.unwrap();
        let detail = get_session_detail(&db, &session.id).await.unwrap();
        let current = detail.current_race.expect("current_race should be Some");
        assert_eq!(current.track_id, race.track_id);
        assert_eq!(current.track_name, race.track_name);
        assert_eq!(current.cup_name, race.cup_name);
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

    #[tokio::test]
    async fn test_list_races_returns_all_races_in_order() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        next_track(&db, &session.id, &host_id).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        let races = list_races(&db, &session.id).await.unwrap();
        assert_eq!(races.len(), 3);
        assert_eq!(races[0].race_number, 1);
        assert_eq!(races[1].race_number, 2);
        assert_eq!(races[2].race_number, 3);

        // All should have 0 runs (no run submission in this PR)
        for race in &races {
            assert_eq!(race.run_count, 0);
        }
    }

    #[tokio::test]
    async fn test_stale_cleanup_marks_participants_as_left() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_id).await.unwrap();

        // Backdate last_activity_at past the stale threshold
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let s = sessions::Entity::find_by_id(&session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        let mut active: sessions::ActiveModel = s.into();
        active.last_activity_at = Set(two_hours_ago);
        active.update(&db).await.unwrap();

        close_stale_sessions(&db).await.unwrap();

        // Both users must be able to create a new session after their old one times out
        assert!(
            create_session(&db, &host_id, "random").await.is_ok(),
            "host should be freed from stale session"
        );
        assert!(
            create_session(&db, &user_id, "random").await.is_ok(),
            "user should be freed from stale session"
        );
    }
}

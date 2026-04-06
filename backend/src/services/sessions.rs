use chrono::Utc;
use rand::seq::SliceRandom;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{cups, runs, session_participants, session_races, sessions, tracks, users};
use crate::error::AppError;

/// Allowed rulesets for this PR. Only "random" is supported.
const VALID_RULESETS: &[&str] = &["random"];

/// Check that the user is not already active in any session. Returns an error
/// with the existing session ID if they are.
async fn check_not_in_any_session(
    db: &impl ConnectionTrait,
    user_id: &str,
) -> Result<(), AppError> {
    let existing = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
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
pub async fn get_active_session_id(
    db: &DatabaseConnection,
    user_id: &str,
) -> Result<Option<String>, AppError> {
    let row = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
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
    if !VALID_RULESETS.contains(&ruleset) {
        return Err(AppError::BadRequest(format!(
            "Invalid ruleset: '{ruleset}'. Valid options: {}",
            VALID_RULESETS.join(", ")
        )));
    }

    check_not_in_any_session(db, user_id).await?;

    let now = Utc::now().to_rfc3339();
    let session_id = Uuid::new_v4().to_string();

    let txn = db.begin().await?;

    sessions::ActiveModel {
        id: Set(session_id.clone()),
        created_by: Set(user_id.to_string()),
        host_id: Set(user_id.to_string()),
        ruleset: Set(ruleset.to_string()),
        least_played_drink_category: Set(None),
        status: Set("active".to_string()),
        created_at: Set(now.clone()),
        last_activity_at: Set(now.clone()),
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
    pub last_activity_at: String,
}

/// Row shape returned by the list-sessions JOIN query.
#[derive(Debug, FromQueryResult)]
struct SessionSummaryRow {
    id: String,
    host_username: String,
    participant_count: i64,
    race_count: i64,
    ruleset: String,
    last_activity_at: String,
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
            last_activity_at: r.last_activity_at,
        })
        .collect())
}

/// Participant info for the detail response.
#[derive(serde::Serialize)]
pub struct ParticipantInfo {
    pub user_id: String,
    pub username: String,
    pub joined_at: String,
    pub left_at: Option<String>,
}

/// Row shape returned by the participant JOIN query.
#[derive(Debug, FromQueryResult)]
struct ParticipantRow {
    user_id: String,
    username: String,
    joined_at: String,
    left_at: Option<String>,
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
    pub created_at: String,
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
    pub created_at: String,
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
    created_at: String,
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
    created_at: String,
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
    pub created_at: String,
    pub last_activity_at: String,
    pub participants: Vec<ParticipantInfo>,
    pub race_number: usize,
    pub current_race: Option<SessionRaceInfo>,
    pub races: Vec<RaceInfo>,
}

/// Get full session detail — the polling endpoint.
/// Uses JOINs to fetch participants with usernames in a single query.
pub async fn get_session_detail(
    db: &DatabaseConnection,
    session_id: &str,
) -> Result<SessionDetail, AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    let host_username = users::Entity::find_by_id(&session.host_id)
        .one(db)
        .await?
        .map(|u| u.username)
        .unwrap_or_else(|| "Unknown".to_string());

    // Fetch all participants with usernames in a single JOIN query
    let participant_rows =
        ParticipantRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
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

    let participants: Vec<ParticipantInfo> = participant_rows
        .into_iter()
        .map(|r| ParticipantInfo {
            user_id: r.user_id,
            username: r.username,
            joined_at: r.joined_at,
            left_at: r.left_at,
        })
        .collect();

    let races_created = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .count(db)
        .await? as usize;
    let race_number = races_created.max(1);

    // Fetch the most recent race with track + cup info in a single JOIN
    let current_race_row =
        CurrentRaceRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
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

    let current_race = current_race_row.map(|r| SessionRaceInfo {
        id: r.id,
        race_number: r.race_number,
        track_id: r.track_id,
        track_name: r.track_name,
        cup_name: r.cup_name,
        image_path: r.image_path,
        created_at: r.created_at,
    });

    // Fetch all races for history (reuses the same query shape as list_races)
    let race_rows = RaceHistoryRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
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

    let races: Vec<RaceInfo> = race_rows
        .into_iter()
        .map(|r| RaceInfo {
            id: r.id,
            race_number: r.race_number,
            track_id: r.track_id,
            track_name: r.track_name,
            cup_name: r.cup_name,
            run_count: r.run_count,
            created_at: r.created_at,
        })
        .collect();

    Ok(SessionDetail {
        id: session.id,
        created_by: session.created_by,
        host_id: session.host_id,
        host_username,
        ruleset: session.ruleset,
        status: session.status,
        created_at: session.created_at,
        last_activity_at: session.last_activity_at,
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
    // Check session exists and is active
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if session.status != "active" {
        return Err(AppError::BadRequest(
            "Cannot join a closed session".to_string(),
        ));
    }

    // Check user isn't already active in any session (including this one)
    check_not_in_any_session(db, user_id).await?;

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.to_string()),
        user_id: Set(user_id.to_string()),
        joined_at: Set(now.clone()),
        left_at: Set(None),
    }
    .insert(&txn)
    .await?;

    let mut active_session: sessions::ActiveModel = session.into();
    active_session.last_activity_at = Set(now);
    active_session.update(&txn).await?;

    txn.commit().await?;

    Ok(())
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

    // Find user's active participant row
    let participant = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
        .one(db)
        .await?
        .ok_or_else(|| AppError::BadRequest("Not currently in this session".to_string()))?;

    let now = Utc::now().to_rfc3339();
    let txn = db.begin().await?;

    // Set left_at
    let mut active_participant: session_participants::ActiveModel = participant.into();
    active_participant.left_at = Set(Some(now.clone()));
    active_participant.update(&txn).await?;

    // Check if host is leaving
    let mut active_session: sessions::ActiveModel = session.clone().into();

    if session.host_id == user_id {
        // Find earliest-joined remaining participant
        let next_host = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::UserId.ne(user_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .order_by_asc(session_participants::Column::JoinedAt)
            .one(&txn)
            .await?;

        match next_host {
            Some(new_host) => {
                active_session.host_id = Set(new_host.user_id);
            }
            None => {
                active_session.status = Set("closed".to_string());
            }
        }
    } else {
        // Check if any active participants remain at all
        let remaining = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .count(&txn)
            .await?;

        if remaining == 0 {
            active_session.status = Set("closed".to_string());
        }
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
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if session.status != "active" {
        return Err(AppError::BadRequest("Session is not active".to_string()));
    }

    if session.host_id != user_id {
        return Err(AppError::Forbidden(
            "Only the host can pick tracks".to_string(),
        ));
    }

    // Get already-used track IDs
    let used_races = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .all(db)
        .await?;
    let race_count = used_races.len() as i32;
    let used_track_ids: Vec<i32> = used_races.iter().map(|r| r.track_id).collect();

    // Get all tracks
    let all_tracks = tracks::Entity::find().all(db).await?;

    // Filter to available tracks
    let mut available: Vec<&tracks::Model> = all_tracks
        .iter()
        .filter(|t| !used_track_ids.contains(&t.id))
        .collect();

    // If pool is empty, reset — all tracks become available again
    if available.is_empty() {
        tracing::info!(
            session_id = session_id,
            "All {} tracks used — resetting pool",
            all_tracks.len()
        );
        available = all_tracks.iter().collect();
    }

    // Pick a random track. Scope the rng so ThreadRng (which is !Send)
    // doesn't live across the subsequent .await points.
    let chosen_idx = {
        let mut rng = rand::thread_rng();
        available
            .choose(&mut rng)
            .map(|t| t.id)
            .ok_or_else(|| AppError::Internal("No tracks available".to_string()))?
    };
    let chosen = all_tracks
        .iter()
        .find(|t| t.id == chosen_idx)
        .ok_or_else(|| AppError::Internal("Track disappeared".to_string()))?;

    let now = Utc::now().to_rfc3339();
    let race_id = Uuid::new_v4().to_string();
    let new_race_number = race_count + 1;

    let txn = db.begin().await?;

    session_races::ActiveModel {
        id: Set(race_id.clone()),
        session_id: Set(session_id.to_string()),
        race_number: Set(new_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(Some(user_id.to_string())),
        created_at: Set(now.clone()),
    }
    .insert(&txn)
    .await?;

    // Update last_activity_at
    let mut active_session: sessions::ActiveModel = session.into();
    active_session.last_activity_at = Set(now.clone());
    active_session.update(&txn).await?;

    txn.commit().await?;

    // Look up cup name for the response
    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .map(|c| c.name)
        .unwrap_or_else(|| "Unknown Cup".to_string());

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: new_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: chosen.image_path.clone(),
        created_at: now,
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
    user_id: &str,
) -> Result<SessionRaceInfo, AppError> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound("Session not found".to_string()))?;

    if session.status != "active" {
        return Err(AppError::BadRequest("Session is not active".to_string()));
    }

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
    let mut exclude_ids: Vec<i32> = used_races.iter().map(|r| r.track_id).collect();
    // Ensure the skipped track stays excluded even though its row is about to be deleted
    if !exclude_ids.contains(&skipped_track_id) {
        exclude_ids.push(skipped_track_id);
    }

    // Get all tracks and filter
    let all_tracks = tracks::Entity::find().all(db).await?;
    let mut available: Vec<&tracks::Model> = all_tracks
        .iter()
        .filter(|t| !exclude_ids.contains(&t.id))
        .collect();

    // If pool is empty (all tracks used + skipped), reset but still exclude the skipped track
    if available.is_empty() {
        tracing::info!(
            session_id = session_id,
            "All tracks used during skip — resetting pool (excluding skipped track {})",
            skipped_track_id
        );
        available = all_tracks
            .iter()
            .filter(|t| t.id != skipped_track_id)
            .collect();
    }

    let chosen_idx = {
        let mut rng = rand::thread_rng();
        available
            .choose(&mut rng)
            .map(|t| t.id)
            .ok_or_else(|| AppError::Internal("No tracks available for skip".to_string()))?
    };
    let chosen = all_tracks
        .iter()
        .find(|t| t.id == chosen_idx)
        .ok_or_else(|| AppError::Internal("Track disappeared".to_string()))?;

    let now = Utc::now().to_rfc3339();
    let race_id = Uuid::new_v4().to_string();

    // Delete old race + insert new one + update activity in a single transaction
    let txn = db.begin().await?;

    current_race.delete(&txn).await?;

    session_races::ActiveModel {
        id: Set(race_id.clone()),
        session_id: Set(session_id.to_string()),
        race_number: Set(keep_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(Some(user_id.to_string())),
        created_at: Set(now.clone()),
    }
    .insert(&txn)
    .await?;

    let mut active_session: sessions::ActiveModel = session.into();
    active_session.last_activity_at = Set(now.clone());
    active_session.update(&txn).await?;

    txn.commit().await?;

    let cup = cups::Entity::find_by_id(chosen.cup_id)
        .one(db)
        .await?
        .map(|c| c.name)
        .unwrap_or_else(|| "Unknown Cup".to_string());

    Ok(SessionRaceInfo {
        id: race_id,
        race_number: keep_race_number,
        track_id: chosen.id,
        track_name: chosen.name.clone(),
        cup_name: cup,
        image_path: chosen.image_path.clone(),
        created_at: now,
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
            created_at: r.created_at,
        })
        .collect())
}

/// Close sessions that have had no activity for over an hour.
/// Returns the number of sessions closed.
pub async fn close_stale_sessions(db: &DatabaseConnection) -> Result<u64, AppError> {
    let one_hour_ago = (Utc::now() - chrono::Duration::hours(1)).to_rfc3339();

    let stale = sessions::Entity::find()
        .filter(
            Condition::all()
                .add(sessions::Column::Status.eq("active"))
                .add(sessions::Column::LastActivityAt.lt(&one_hour_ago)),
        )
        .all(db)
        .await?;

    let count = stale.len() as u64;

    let txn = db.begin().await?;
    for session in stale {
        let mut active: sessions::ActiveModel = session.into();
        active.status = Set("closed".to_string());
        active.update(&txn).await?;
    }
    txn.commit().await?;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::Database;

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("connect");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("pragma");
        Migrator::up(&db, None).await.expect("migrate");
        db
    }

    /// Seed a small set of cups and tracks for testing track selection.
    /// Creates 3 cups with 2 tracks each (6 tracks total).
    async fn seed_tracks_for_test(db: &DatabaseConnection) {
        let cup_names = ["Test Cup A", "Test Cup B", "Test Cup C"];
        for (i, name) in cup_names.iter().enumerate() {
            cups::ActiveModel {
                id: Set((i + 1) as i32),
                name: Set(name.to_string()),
                image_path: Set(format!("images/cups/test-cup-{}.webp", i + 1)),
            }
            .insert(db)
            .await
            .expect("insert cup");
        }

        let track_data = [
            (1, "Track Alpha", 1, 1),
            (2, "Track Beta", 1, 2),
            (3, "Track Gamma", 2, 1),
            (4, "Track Delta", 2, 2),
            (5, "Track Epsilon", 3, 1),
            (6, "Track Zeta", 3, 2),
        ];
        for (id, name, cup_id, position) in track_data {
            tracks::ActiveModel {
                id: Set(id),
                name: Set(name.to_string()),
                cup_id: Set(cup_id),
                position: Set(position),
                image_path: Set(format!("images/tracks/track-{id}.webp")),
            }
            .insert(db)
            .await
            .expect("insert track");
        }
    }

    async fn create_user(db: &DatabaseConnection, username: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let hash = "$argon2id$v=19$m=19456,t=2,p=1$dGVzdHNhbHQ$abc123";
        users::ActiveModel {
            id: Set(id.clone()),
            username: Set(username.to_string()),
            email: Set(None),
            password_hash: Set(hash.to_string()),
            preferred_character_id: Set(None),
            preferred_body_id: Set(None),
            preferred_wheel_id: Set(None),
            preferred_glider_id: Set(None),
            preferred_drink_type_id: Set(None),
            refresh_token_version: Set(0),
            created_at: Set(now.clone()),
            updated_at: Set(now),
        }
        .insert(db)
        .await
        .expect("insert user");
        id
    }

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
}

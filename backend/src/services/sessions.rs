use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder, Set,
};
use uuid::Uuid;

use crate::entities::{session_participants, session_races, sessions, users};
use crate::error::AppError;

/// Allowed rulesets for this PR. Only "random" is supported.
const VALID_RULESETS: &[&str] = &["random"];

/// Create a new session. The creator becomes both the host and the first
/// participant.
pub async fn create_session(
    db: &DatabaseConnection,
    user_id: &str,
    ruleset: &str,
) -> Result<sessions::Model, AppError> {
    if !VALID_RULESETS.contains(&ruleset) {
        return Err(AppError::BadRequest(format!(
            "Invalid ruleset: '{ruleset}'. Valid options: {}",
            VALID_RULESETS.join(", ")
        )));
    }

    let now = Utc::now().to_rfc3339();
    let session_id = Uuid::new_v4().to_string();

    let session = sessions::ActiveModel {
        id: Set(session_id.clone()),
        created_by: Set(user_id.to_string()),
        host_id: Set(user_id.to_string()),
        ruleset: Set(ruleset.to_string()),
        least_played_drink_category: Set(None),
        status: Set("active".to_string()),
        created_at: Set(now.clone()),
        last_activity_at: Set(now.clone()),
    }
    .insert(db)
    .await?;

    // Add creator as first participant
    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id),
        user_id: Set(user_id.to_string()),
        joined_at: Set(now),
        left_at: Set(None),
    }
    .insert(db)
    .await?;

    Ok(session)
}

/// Summary info for listing active sessions.
#[derive(serde::Serialize)]
pub struct SessionSummary {
    pub id: String,
    pub host_username: String,
    pub participant_count: usize,
    pub race_count: usize,
    pub ruleset: String,
    pub last_activity_at: String,
}

/// List active sessions sorted by last_activity_at DESC.
pub async fn list_active_sessions(
    db: &DatabaseConnection,
) -> Result<Vec<SessionSummary>, AppError> {
    let active_sessions = sessions::Entity::find()
        .filter(sessions::Column::Status.eq("active"))
        .order_by_desc(sessions::Column::LastActivityAt)
        .all(db)
        .await?;

    let mut summaries = Vec::with_capacity(active_sessions.len());

    for session in active_sessions {
        // Get host username
        let host = users::Entity::find_by_id(&session.host_id)
            .one(db)
            .await?
            .map(|u| u.username)
            .unwrap_or_else(|| "Unknown".to_string());

        // Count active participants (left_at IS NULL)
        let participant_count = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .count(db)
            .await? as usize;

        // Count races
        let race_count = session_races::Entity::find()
            .filter(session_races::Column::SessionId.eq(&session.id))
            .count(db)
            .await? as usize;

        summaries.push(SessionSummary {
            id: session.id,
            host_username: host,
            participant_count,
            race_count,
            ruleset: session.ruleset,
            last_activity_at: session.last_activity_at,
        });
    }

    Ok(summaries)
}

/// Participant info for the detail response.
#[derive(serde::Serialize)]
pub struct ParticipantInfo {
    pub user_id: String,
    pub username: String,
    pub joined_at: String,
    pub left_at: Option<String>,
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
    pub race_count: usize,
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

    let host_username = users::Entity::find_by_id(&session.host_id)
        .one(db)
        .await?
        .map(|u| u.username)
        .unwrap_or_else(|| "Unknown".to_string());

    // Get all participants with usernames
    let participant_rows = session_participants::Entity::find()
        .filter(session_participants::Column::SessionId.eq(session_id))
        .order_by_asc(session_participants::Column::JoinedAt)
        .all(db)
        .await?;

    let mut participants = Vec::with_capacity(participant_rows.len());
    for p in participant_rows {
        let username = users::Entity::find_by_id(&p.user_id)
            .one(db)
            .await?
            .map(|u| u.username)
            .unwrap_or_else(|| "Unknown".to_string());

        participants.push(ParticipantInfo {
            user_id: p.user_id,
            username,
            joined_at: p.joined_at,
            left_at: p.left_at,
        });
    }

    let race_count = session_races::Entity::find()
        .filter(session_races::Column::SessionId.eq(session_id))
        .count(db)
        .await? as usize;

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
        race_count,
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

    // Check if user already has an active row (left_at IS NULL)
    let existing = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id))
                .add(session_participants::Column::LeftAt.is_null()),
        )
        .one(db)
        .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Already in this session".to_string()));
    }

    let now = Utc::now().to_rfc3339();

    // Create new participant row (handles rejoin after leaving)
    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.to_string()),
        user_id: Set(user_id.to_string()),
        joined_at: Set(now.clone()),
        left_at: Set(None),
    }
    .insert(db)
    .await?;

    // Update last_activity_at
    let mut active_session: sessions::ActiveModel = session.into();
    active_session.last_activity_at = Set(now);
    active_session.update(db).await?;

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

    // Set left_at
    let mut active_participant: session_participants::ActiveModel = participant.into();
    active_participant.left_at = Set(Some(now.clone()));
    active_participant.update(db).await?;

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
            .one(db)
            .await?;

        match next_host {
            Some(new_host) => {
                active_session.host_id = Set(new_host.user_id);
            }
            None => {
                // No participants remain — close the session
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
            .count(db)
            .await?;

        if remaining == 0 {
            active_session.status = Set("closed".to_string());
        }
    }

    active_session.last_activity_at = Set(now);
    active_session.update(db).await?;

    Ok(())
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

    for session in stale {
        let mut active: sessions::ActiveModel = session.into();
        active.status = Set("closed".to_string());
        active.update(db).await?;
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use migration::{Migrator, MigratorTrait};
    use sea_orm::{ConnectionTrait, Database};

    async fn setup_db() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:").await.expect("connect");
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("pragma");
        Migrator::up(&db, None).await.expect("migrate");
        db
    }

    async fn create_user(db: &DatabaseConnection, username: &str) -> String {
        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        // Use a valid argon2 hash for test users
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

        // Create session (host is automatically a participant)
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // user2 joins, then user3 joins
        join_session(&db, &session.id, &user2_id).await.unwrap();
        join_session(&db, &session.id, &user3_id).await.unwrap();

        // Host leaves
        leave_session(&db, &session.id, &host_id).await.unwrap();

        // user2 should be the new host (earliest joined)
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

        // Only participant (host) leaves
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
        leave_session(&db, &session.id, &host_id).await.unwrap(); // closes it

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

        // Should be able to rejoin
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

        // user2 leaves
        leave_session(&db, &session.id, &user2_id).await.unwrap();

        // user2's row should have left_at set
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

        // host's row should still be active
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
}

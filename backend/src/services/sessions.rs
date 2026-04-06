use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{session_participants, session_races, sessions, users};
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
}

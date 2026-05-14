//! Session detail aggregation — the polling read path.
//!
//! Builds [`SessionDetail`] for the polling endpoint and exposes
//! [`list_races`] for the race-history view. [`RaceInfo`] (race-history row)
//! and [`ParticipantInfo`] live here because only `SessionDetail` consumes
//! them; the cross-submodule DTOs [`SessionRaceInfo`] and
//! [`RaceSubmission`] live in [`super::types`].
//!
//! [`SessionRaceInfo`]: super::types::SessionRaceInfo
//! [`RaceSubmission`]: super::types::RaceSubmission

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{ConnectionTrait, EntityTrait, FromQueryResult};

use super::{
    races::get_pending_races,
    types::{RaceSubmission, SessionRaceInfo},
};
use crate::{
    domain::{
        ImagePath, SessionId, SessionRaceId, UserId, Username,
        enums::{SessionRuleset, SessionStatus},
    },
    entities::{sessions, users},
    error::Error,
};

/// Participant info for the detail response.
#[derive(serde::Serialize)]
pub struct ParticipantInfo {
    /// Participant's user ID.
    pub user_id: UserId,
    /// Cached username for display (saves a JOIN on the read path).
    pub username: Username,
    /// When this participation row was created (joined or rejoined).
    pub joined_at: DateTime<Utc>,
    /// When the participant left, or `None` if still active.
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

/// Row shape for the submissions query.
#[derive(Debug, FromQueryResult)]
struct SubmissionRow {
    user_id: String,
    username: String,
    track_time: i32,
    disqualified: bool,
}

/// Race info for the race history list.
#[derive(serde::Serialize)]
pub struct RaceInfo {
    /// Stable UUID of the race row.
    pub id: SessionRaceId,
    /// 1-indexed position within the session.
    pub race_number: i32,
    /// FK to `tracks.id`.
    pub track_id: i32,
    /// Cached track name (saves a JOIN on the read path).
    pub track_name: String,
    /// Cached parent-cup name for display.
    pub cup_name: String,
    /// Number of runs submitted to this race so far.
    pub run_count: i64,
    /// Race-creation timestamp, UTC.
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
    /// Session's stable UUID.
    pub id: SessionId,
    /// Current host's user ID.
    pub host_id: UserId,
    /// Cached host display name (saves a JOIN on the read path).
    pub host_username: Username,
    /// Track-selection ruleset chosen at session creation.
    pub ruleset: SessionRuleset,
    /// Lifecycle state — `Active` or `Closed`.
    pub status: SessionStatus,
    /// Session-creation timestamp, UTC.
    pub created_at: DateTime<Utc>,
    /// Most recent activity timestamp; used by stale-session cleanup.
    pub last_activity_at: DateTime<Utc>,
    /// All participants who have ever joined this session, in join order.
    pub participants: Vec<ParticipantInfo>,
    /// 1-indexed race count: 1 means "no races completed; race 1 is up next".
    pub race_number: usize,
    /// The current in-progress race, if one exists.
    pub current_race: Option<SessionRaceInfo>,
    /// All races for this session, oldest first.
    pub races: Vec<RaceInfo>,
    /// Pending races for the requesting user, oldest first. Empty if the
    /// user is not in this session, has no pending races, or is past the
    /// 5-minute grace window after leaving. The API returns all matching
    /// rows; the UI applies the "max 3 pending" cap.
    pub your_pending: Vec<SessionRaceInfo>,
}

// ── get_session_detail sub-queries ────────────────────────────────────

/// Look up the host's username. Returns `Internal` if the FK-referenced
/// user row is missing (data corruption — FKs should prevent this) or if
/// the stored username fails the newtype's invariant.
async fn load_host_username(
    db: &impl ConnectionTrait,
    host_id: &UserId,
) -> Result<Username, Error> {
    let user = users::Entity::find_by_id(host_id)
        .one(db)
        .await?
        .ok_or_else(|| {
            Error::Internal(anyhow::anyhow!("Host user not found for host_id {host_id}"))
        })?;
    Username::from_db(user.username, "users.username")
}

/// Fetch all participants with usernames in a single JOIN query.
async fn load_participants(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
) -> Result<Vec<ParticipantInfo>, Error> {
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

    rows.into_iter()
        .map(|r| {
            Ok(ParticipantInfo {
                user_id: UserId::from_db(&r.user_id)?,
                username: Username::from_db(r.username, "users.username")?,
                joined_at: r.joined_at.and_utc(),
                left_at: r.left_at.map(|t| t.and_utc()),
            })
        })
        .collect()
}

/// Fetch the most recent race with its submissions. Returns `None` if
/// no races have been created yet.
async fn load_current_race_with_submissions(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
) -> Result<Option<SessionRaceInfo>, Error> {
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

    let submissions: Vec<RaceSubmission> =
        SubmissionRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
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
        .map(|s| {
            Ok::<_, Error>(RaceSubmission {
                user_id: UserId::from_db(&s.user_id)?,
                username: Username::from_db(s.username, "users.username")?,
                track_time: s.track_time,
                disqualified: s.disqualified,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(SessionRaceInfo {
        id: SessionRaceId::from_db(&row.id)?,
        race_number: row.race_number,
        track_id: row.track_id,
        track_name: row.track_name,
        cup_name: row.cup_name,
        image_path: ImagePath::from_db(row.image_path, "tracks.image_path")?,
        created_at: row.created_at.and_utc(),
        submissions,
    }))
}

/// Fetch all races in a session with run counts, ordered by `race_number` ASC.
async fn load_race_history(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
) -> Result<Vec<RaceInfo>, Error> {
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

    rows.into_iter()
        .map(|r| {
            Ok(RaceInfo {
                id: SessionRaceId::from_db(&r.id)?,
                race_number: r.race_number,
                track_id: r.track_id,
                track_name: r.track_name,
                cup_name: r.cup_name,
                run_count: r.run_count,
                created_at: r.created_at.and_utc(),
            })
        })
        .collect()
}

/// Get full session detail — the polling endpoint.
///
/// `requesting_user_id` is the authenticated caller — used to compute the
/// per-user `your_pending` list. Anonymous callers aren't supported by the
/// route layer, so this is always `Some` in production; tests pass `None`
/// when they don't care about pending state.
///
/// # Errors
///
/// Returns `NotFound` if no session with that ID exists; `Internal` for
/// unexpected DB failures on any of the helper queries.
#[tracing::instrument(
    skip(db),
    fields(session_id = %session_id, requesting_user_id = ?requesting_user_id),
)]
pub async fn get_session_detail(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
    requesting_user_id: Option<&UserId>,
) -> Result<SessionDetail, Error> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Session not found".to_string()))?;

    let host_id = UserId::from_db(&session.host_id)?;
    let host_username = load_host_username(db, &host_id).await?;
    let participants = load_participants(db, session_id).await?;
    let current_race = load_current_race_with_submissions(db, session_id).await?;
    let races = load_race_history(db, session_id).await?;
    let your_pending = match requesting_user_id {
        Some(uid) => get_pending_races(db, session_id, uid).await?,
        None => Vec::new(),
    };

    // Derive race_number from history instead of a separate COUNT query —
    // saves one DB round trip on every poll. Safe because race numbers are
    // 1-indexed and gapless: `next_track` appends monotonically, and
    // `skip_turn` replaces in-place (preserves race_number). No deletion
    // path exists. Under this invariant, last().race_number == COUNT(*).
    let race_number = match races.last() {
        None => 1,
        Some(r) => usize::try_from(r.race_number).map_err(|_| {
            Error::Internal(anyhow::anyhow!(
                "race_number invariant violated: got {}",
                r.race_number
            ))
        })?,
    };

    Ok(SessionDetail {
        id: SessionId::from_db(&session.id)?,
        host_id,
        host_username,
        ruleset: session.ruleset,
        status: session.status,
        created_at: session.created_at.and_utc(),
        last_activity_at: session.last_activity_at.and_utc(),
        participants,
        race_number,
        current_race,
        races,
        your_pending,
    })
}

/// List all races in a session, ordered by `race_number` ASC.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip(db), fields(session_id = %session_id))]
pub async fn list_races(
    db: &impl ConnectionTrait,
    session_id: &SessionId,
) -> Result<Vec<RaceInfo>, Error> {
    // Verify session exists
    sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Session not found".to_string()))?;

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

    rows.into_iter()
        .map(|r| {
            Ok(RaceInfo {
                id: SessionRaceId::from_db(&r.id)?,
                race_number: r.race_number,
                track_id: r.track_id,
                track_name: r.track_name,
                cup_name: r.cup_name,
                run_count: r.run_count,
                created_at: r.created_at.and_utc(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::sessions::{create_session, join_session, next_track},
        test_helpers::{create_user, seed_tracks_for_test, setup_db},
    };

    // ── load_host_username ───────────────────────────────────────────

    #[tokio::test]
    async fn test_load_host_username_returns_username() {
        let db = setup_db().await;
        let user_id = create_user(&db, "alice").await;
        let username = load_host_username(&db, &user_id).await.unwrap();
        assert_eq!(username.as_ref(), "alice");
    }

    #[tokio::test]
    async fn test_load_host_username_missing_user_returns_internal() {
        let db = setup_db().await;
        let err = load_host_username(&db, &UserId::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Internal(_)));
    }

    // ── get_session_detail / list_races ──────────────────────────────

    #[tokio::test]
    async fn test_get_session_detail_returns_participants_with_usernames() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();

        let detail = get_session_detail(&db, &session.id, Some(&host_id))
            .await
            .unwrap();
        assert_eq!(detail.participants.len(), 2);
        assert_eq!(detail.host_username.as_ref(), "host");
        assert_eq!(detail.race_number, 1);

        let usernames: Vec<String> = detail
            .participants
            .iter()
            .map(|p| p.username.to_string())
            .collect();
        assert!(usernames.iter().any(|u| u == "host"));
        assert!(usernames.iter().any(|u| u == "user2"));
    }

    #[tokio::test]
    async fn test_current_race_appears_in_session_detail() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        // Before any track pick, current_race should be None
        let detail = get_session_detail(&db, &session.id, Some(&host_id))
            .await
            .unwrap();
        assert!(detail.current_race.is_none());

        // After picking a track, current_race should be populated
        let race = next_track(&db, &session.id, &host_id).await.unwrap();
        let detail = get_session_detail(&db, &session.id, Some(&host_id))
            .await
            .unwrap();
        let current = detail.current_race.expect("current_race should be Some");
        assert_eq!(current.track_id, race.track_id);
        assert_eq!(current.track_name, race.track_name);
        assert_eq!(current.cup_name, race.cup_name);
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

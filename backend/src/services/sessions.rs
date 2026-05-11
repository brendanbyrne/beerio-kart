use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait,
    FromQueryResult, ModelTrait, PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
    sea_query::Expr,
};
use uuid::Uuid;

use crate::{
    domain::{
        SessionId, SessionRaceId, UserId,
        enums::{Ruleset, SessionStatus},
    },
    entities::{
        cups, runs, session_participants, session_race_participations, session_races, sessions,
        users,
    },
    error::Error,
    services::helpers,
};

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
    user_id: &UserId,
) -> Result<(), Error> {
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
        return Err(Error::conflict(format!(
            "Already in session {}",
            row.session_id
        )));
    }

    Ok(())
}

/// Returns the session ID the user is currently active in, or None.
/// Only considers active sessions (not closed/stale ones).
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
pub async fn get_active_session_id(
    db: &DatabaseConnection,
    user_id: &UserId,
) -> Result<Option<SessionId>, Error> {
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

    Ok(row.map(|r| SessionId::new(r.session_id)))
}

/// Create a new session. The creator becomes both the host and the first
/// participant. Returns the full session detail.
///
/// # Errors
///
/// Returns `BadRequest` if `ruleset` doesn't parse as a known `Ruleset`;
/// `Conflict` if the user is already in another active session; `Internal`
/// for unexpected DB failures.
pub async fn create_session(
    db: &DatabaseConnection,
    user_id: &UserId,
    ruleset: &str,
) -> Result<SessionDetail, Error> {
    let parsed: Ruleset = ruleset.parse()?;

    check_not_in_any_session(db, user_id).await?;

    let now = Utc::now().naive_utc();
    let session_id = SessionId::new(Uuid::new_v4().to_string());

    let txn = db.begin().await?;

    sessions::ActiveModel {
        id: Set(session_id.as_str().to_string()),
        host_id: Set(user_id.as_str().to_string()),
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
        session_id: Set(session_id.as_str().to_string()),
        user_id: Set(user_id.as_str().to_string()),
        joined_at: Set(now),
        left_at: Set(None),
    }
    .insert(&txn)
    .await?;

    txn.commit().await?;

    get_session_detail(db, &session_id, Some(user_id)).await
}

/// Summary info for listing active sessions.
#[derive(serde::Serialize)]
pub struct SessionSummary {
    pub id: SessionId,
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

/// List active sessions sorted by `last_activity_at` DESC.
/// Uses a single JOIN query instead of N+1 queries.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
pub async fn list_active_sessions(db: &DatabaseConnection) -> Result<Vec<SessionSummary>, Error> {
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
            id: SessionId::new(r.id),
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
    pub user_id: UserId,
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
    pub user_id: UserId,
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
    pub id: SessionRaceId,
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
    pub id: SessionRaceId,
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
    pub id: SessionId,
    pub host_id: UserId,
    pub host_username: String,
    pub ruleset: String,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub participants: Vec<ParticipantInfo>,
    pub race_number: usize,
    pub current_race: Option<SessionRaceInfo>,
    pub races: Vec<RaceInfo>,
    /// Pending races for the requesting user, oldest first. Empty if the
    /// user is not in this session, has no pending races, or is past the
    /// 5-minute grace window after leaving. The API returns all matching
    /// rows; the UI applies the "max 3 pending" cap.
    pub your_pending: Vec<SessionRaceInfo>,
}

// ── get_session_detail sub-queries ────────────────────────────────────

/// Look up the host's username. Returns `Internal` if the FK-referenced
/// user row is missing (data corruption — FKs should prevent this).
async fn load_host_username(db: &impl ConnectionTrait, host_id: &UserId) -> Result<String, Error> {
    users::Entity::find_by_id(host_id)
        .one(db)
        .await?
        .map(|u| u.username)
        .ok_or_else(|| {
            Error::Internal(anyhow::anyhow!("Host user not found for host_id {host_id}"))
        })
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

    Ok(rows
        .into_iter()
        .map(|r| ParticipantInfo {
            user_id: UserId::new(r.user_id),
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
        user_id: UserId::new(s.user_id),
        username: s.username,
        track_time: s.track_time,
        disqualified: s.disqualified,
    })
    .collect();

    Ok(Some(SessionRaceInfo {
        id: SessionRaceId::new(row.id),
        race_number: row.race_number,
        track_id: row.track_id,
        track_name: row.track_name,
        cup_name: row.cup_name,
        image_path: row.image_path,
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

    Ok(rows
        .into_iter()
        .map(|r| RaceInfo {
            id: SessionRaceId::new(r.id),
            race_number: r.race_number,
            track_id: r.track_id,
            track_name: r.track_name,
            cup_name: r.cup_name,
            run_count: r.run_count,
            created_at: r.created_at.and_utc(),
        })
        .collect())
}

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

    Ok(rows
        .into_iter()
        .map(|r| SessionRaceInfo {
            id: SessionRaceId::new(r.id),
            race_number: r.race_number,
            track_id: r.track_id,
            track_name: r.track_name,
            cup_name: r.cup_name,
            image_path: r.image_path,
            created_at: r.created_at.and_utc(),
            submissions: Vec::new(),
        })
        .collect())
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
    let Some(race) = race.filter(|r| r.session_id == session_id.as_str()) else {
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
pub async fn get_session_detail(
    db: &DatabaseConnection,
    session_id: &SessionId,
    requesting_user_id: Option<&UserId>,
) -> Result<SessionDetail, Error> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Session not found".to_string()))?;

    let host_id = UserId::new(session.host_id);
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
        id: SessionId::new(session.id),
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

/// Grace window for "rejoin without losing pre-leave pending races."
///
/// Within this window of `left_at`, rejoining preserves `joined_at` (and
/// therefore preserves access to pre-leave pending races, per the §3 grace
/// semantics). After this window, `joined_at` is reset to `NOW()`, forfeiting
/// any pre-gap pending records.
pub const REJOIN_GRACE_MINUTES: i64 = 5;

/// Join (or rejoin) a session. **Single mutable row per (session, user)** —
/// first join INSERTs, rejoin mutates the existing row.
///
/// Rejoin behavior:
/// - **Within grace** (`NOW() - left_at <= 5 min`): clear `left_at`, leave
///   `joined_at` untouched. The user is treated as continuously present, so
///   any pending races created before the leave remain accessible.
/// - **Outside grace** (`NOW() - left_at > 5 min`): clear `left_at` AND reset
///   `joined_at = NOW()`. Pre-gap pending records remain in the DB for
///   history but are filtered out by the pending-races query
///   (`session_races.created_at >= session_participants.joined_at`).
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Conflict` if the
/// session is closed or the user is already in another active session;
/// `Internal` for unexpected DB failures.
pub async fn join_session(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<(), Error> {
    helpers::load_active_session(db, session_id)
        .await
        .map_err(|e| match e {
            Error::Conflict { .. } => Error::conflict("Cannot join a closed session"),
            other => other,
        })?;
    check_not_in_any_session(db, user_id).await?;

    let existing = session_participants::Entity::find()
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.eq(session_id))
                .add(session_participants::Column::UserId.eq(user_id)),
        )
        .one(db)
        .await?;

    let now = Utc::now().naive_utc();
    let txn = db.begin().await?;

    match existing {
        None => {
            session_participants::ActiveModel {
                id: Set(Uuid::new_v4().to_string()),
                session_id: Set(session_id.as_str().to_string()),
                user_id: Set(user_id.as_str().to_string()),
                joined_at: Set(now),
                left_at: Set(None),
            }
            .insert(&txn)
            .await?;
        }
        Some(row) => {
            let Some(left_at) = row.left_at else {
                // Defensive: should be unreachable in normal flow. `existing`
                // is filtered to (session_id, user_id), so a row with
                // `left_at IS NULL` here means the user is already active in
                // *this* session — and `check_not_in_any_session` above would
                // have surfaced that as a 409 before we got here. Landing in
                // this branch implies a race between the two queries (very
                // unlikely without external manipulation) or direct DB
                // tampering. Return Conflict either way for safety.
                return Err(Error::conflict(
                    "Already an active participant in this session",
                ));
            };

            let mut active: session_participants::ActiveModel = row.into();
            active.left_at = Set(None);

            let gap = now.signed_duration_since(left_at);
            if gap > chrono::Duration::minutes(REJOIN_GRACE_MINUTES) {
                // Outside grace — reset joined_at so pre-gap pending records
                // are filtered out of the user's pending list.
                active.joined_at = Set(now);
            }

            active.update(&txn).await?;
        }
    }

    helpers::touch_session(&txn, session_id).await?;

    txn.commit().await?;

    Ok(())
}

/// What should happen to the session after a participant leaves.
#[derive(Debug, PartialEq, Eq)]
enum HostDisposition {
    /// Host role transferred to the given user ID.
    TransferredTo(UserId),
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
///
/// **Note on `joined_at` semantics:** `joined_at` is the start of the
/// participant's *current* presence segment, which gets reset on a long-gap
/// rejoin (see `join_session`). So "earliest-joined" effectively means
/// "most-tenured in the current segment." That's the right host successor —
/// someone who just rejoined after a long break shouldn't outrank a steadily
/// present participant.
async fn transfer_host_or_close(
    txn: &impl ConnectionTrait,
    session_id: &SessionId,
    leaving_user_id: &UserId,
    is_host_leaving: bool,
) -> Result<HostDisposition, Error> {
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
            Some(new_host) => Ok(HostDisposition::TransferredTo(UserId::new(
                new_host.user_id,
            ))),
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

/// Leave a session. Sets `left_at` and handles host transfer.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `BadRequest` if the
/// user is not currently in the session; `Internal` for unexpected DB
/// failures or invariant violations (e.g., last-active-participant flow
/// failing to find any remaining participant to promote).
pub async fn leave_session(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<(), Error> {
    let session = sessions::Entity::find_by_id(session_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound("Session not found".to_string()))?;

    // require_active_participant returns Forbidden (authorization guard), but
    // leaving a session you're not in is bad input, not an auth failure.
    let participant = helpers::require_active_participant(db, session_id, user_id)
        .await
        .map_err(|_| Error::bad_request("Not currently in this session"))?;

    let now = Utc::now().naive_utc();
    let txn = db.begin().await?;

    let mut active_participant: session_participants::ActiveModel = participant.into();
    active_participant.left_at = Set(Some(now));
    active_participant.update(&txn).await?;

    let is_host_leaving = session.host_id.as_str() == user_id.as_str();
    let mut active_session: sessions::ActiveModel = session.into();
    let disposition = transfer_host_or_close(&txn, session_id, user_id, is_host_leaving).await?;

    match disposition {
        HostDisposition::TransferredTo(new_host_id) => {
            active_session.host_id = Set(new_host_id.into_string());
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
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Conflict` if the
/// session is closed; `Forbidden` if `user_id` isn't the host; `Internal`
/// for unexpected DB failures or an empty `tracks` table.
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

    let now = Utc::now().naive_utc();
    let race_id = SessionRaceId::new(Uuid::new_v4().to_string());
    let new_race_number = race_count + 1;

    let txn = db.begin().await?;

    session_races::ActiveModel {
        id: Set(race_id.as_str().to_string()),
        session_id: Set(session_id.as_str().to_string()),
        race_number: Set(new_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: Set(now),
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
        image_path: chosen.image_path.clone(),
        created_at: now.and_utc(),
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
pub async fn skip_turn(
    db: &DatabaseConnection,
    session_id: &SessionId,
    _user_id: &UserId,
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

    let now = Utc::now().naive_utc();
    let race_id = SessionRaceId::new(Uuid::new_v4().to_string());

    // Delete old race + insert new one + snapshot present users in a single
    // transaction. The old race's `session_race_participations` rows cascade
    // away with the delete; the new race gets a fresh snapshot of who's
    // currently present.
    let txn = db.begin().await?;

    current_race.delete(&txn).await?;

    session_races::ActiveModel {
        id: Set(race_id.as_str().to_string()),
        session_id: Set(session_id.as_str().to_string()),
        race_number: Set(keep_race_number),
        track_id: Set(chosen.id),
        chosen_by: Set(None),
        created_at: Set(now),
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
        image_path: chosen.image_path.clone(),
        created_at: now.and_utc(),
        submissions: Vec::new(),
    })
}

/// List all races in a session, ordered by `race_number` ASC.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Internal` for
/// unexpected DB failures.
pub async fn list_races(
    db: &DatabaseConnection,
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

    Ok(rows
        .into_iter()
        .map(|r| RaceInfo {
            id: SessionRaceId::new(r.id),
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
///
/// Also marks all remaining active participants as left, preventing
/// users from being soft-locked out of creating/joining new sessions.
/// Returns the number of sessions closed.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures on any of the SELECT or
/// UPDATE statements that drive the cleanup.
pub async fn close_stale_sessions(db: &DatabaseConnection) -> Result<u64, Error> {
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
    use crate::{
        entities::session_race_participations,
        test_helpers::{
            backdate_participant, create_user, insert_participant, insert_race_participation,
            insert_session, insert_session_race, seed_tracks_for_test, setup_db,
        },
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
        let err = load_host_username(&db, &UserId::new("nonexistent-id"))
            .await
            .unwrap_err();
        assert!(matches!(err, Error::Internal(_)));
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
        assert_eq!(updated.host_id, user2_id.as_str());
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

        let detail = get_session_detail(&db, &session.id, Some(&host_id))
            .await
            .unwrap();
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
            .filter(session_race_participations::Column::SessionRaceId.eq(&race.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(parts.len(), 2, "one row per currently-present user");
        let user_ids: std::collections::HashSet<&str> =
            parts.iter().map(|p| p.user_id.as_str()).collect();
        assert!(user_ids.contains(host_id.as_str()));
        assert!(user_ids.contains(user2_id.as_str()));
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
            .filter(session_race_participations::Column::SessionRaceId.eq(&race.id))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(
            parts.len(),
            1,
            "only the still-present host should be snapshotted"
        );
        assert_eq!(parts[0].user_id, host_id.as_str());
    }

    #[tokio::test]
    async fn test_create_session_race_atomic_with_race_insert() {
        // If a participation insert fails inside the same transaction as the
        // session_races insert, the entire transaction must roll back —
        // the race row must not be visible afterwards. We force the failure
        // by trying to INSERT a participation row with a non-existent
        // user_id (FK violation) inside the same txn.
        use sea_orm::TransactionTrait;

        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, "active").await;
        insert_participant(&db, &session_id, &host_id, None).await;

        let txn = db.begin().await.unwrap();
        let race_id = Uuid::new_v4().to_string();
        let now = Utc::now().naive_utc();

        session_races::ActiveModel {
            id: Set(race_id.clone()),
            session_id: Set(session_id.as_str().to_string()),
            race_number: Set(1),
            track_id: Set(1),
            chosen_by: Set(None),
            created_at: Set(now),
        }
        .insert(&txn)
        .await
        .expect("race insert succeeds");

        // FK violation: user_id "ghost" doesn't exist in users.
        let bad = session_race_participations::ActiveModel {
            session_race_id: Set(race_id.clone()),
            user_id: Set("ghost".to_string()),
            created_at: Set(now),
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
            drink_type_id::drink_type_uuid,
            entities::{drink_types, runs},
            test_helpers::seed_game_data,
        };

        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, "active").await;
        insert_participant(&db, &session_id, &host_id, None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, Utc::now().naive_utc()).await;
        insert_race_participation(&db, &race_id, &host_id, None).await;

        // Verify pending before the run
        let pending = get_pending_races(&db, &session_id, &host_id).await.unwrap();
        assert_eq!(pending.len(), 1);

        // Insert a run row for this (race, user)
        let drink_id = drink_type_uuid("Test Beer");
        let _drink = drink_types::Entity::find_by_id(&drink_id)
            .one(&db)
            .await
            .unwrap()
            .expect("seed_game_data inserts Test Beer");
        runs::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(host_id.as_str().to_string()),
            session_race_id: Set(race_id.as_str().to_string()),
            track_id: Set(1),
            character_id: Set(1),
            body_id: Set(1),
            wheel_id: Set(1),
            glider_id: Set(1),
            track_time: Set(120_000),
            lap1_time: Set(40_000),
            lap2_time: Set(40_000),
            lap3_time: Set(40_000),
            drink_type_id: Set(drink_id),
            disqualified: Set(false),
            photo_path: Set(None),
            created_at: Set(Utc::now().naive_utc()),
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
        let session_id = insert_session(&db, &host_id, "active").await;
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
            .filter(session_race_participations::Column::UserId.eq(&user_b))
            .count(&db)
            .await
            .unwrap();
        assert_eq!(count, 1, "participation row remains for history");
    }

    #[tokio::test]
    async fn test_rejoin_within_grace_preserves_pending() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        // Capture B's joined_at, then leave + backdate left_at to 3 min ago.
        let original_joined = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&user_b)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .joined_at;
        leave_session(&db, &session.id, &user_b).await.unwrap();
        let three_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(3);
        backdate_participant(&db, &session.id, &user_b, None, Some(three_min_ago)).await;

        join_session(&db, &session.id, &user_b).await.unwrap();

        let row = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&user_b)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(row.left_at.is_none(), "left_at cleared on rejoin");
        assert_eq!(
            row.joined_at, original_joined,
            "within-grace rejoin must NOT advance joined_at"
        );

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert_eq!(
            pending.len(),
            1,
            "pre-leave pending preserved on within-grace rejoin"
        );
    }

    #[tokio::test]
    async fn test_rejoin_after_grace_resets_joined_at_and_forfeits_pending() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        let original_joined = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&user_b)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap()
            .joined_at;

        leave_session(&db, &session.id, &user_b).await.unwrap();
        // Backdate left_at to 20 min ago — well past the 5-minute window.
        let twenty_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(20);
        backdate_participant(&db, &session.id, &user_b, None, Some(twenty_min_ago)).await;

        join_session(&db, &session.id, &user_b).await.unwrap();

        let row = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(&session.id))
                    .add(session_participants::Column::UserId.eq(&user_b)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(row.left_at.is_none(), "left_at cleared on rejoin");
        assert!(
            row.joined_at > original_joined,
            "outside-grace rejoin must advance joined_at: was {}, now {}",
            original_joined,
            row.joined_at,
        );

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert!(
            pending.is_empty(),
            "pre-gap pending is forfeited (filtered by created_at >= joined_at)"
        );

        // The participation row itself stays for history.
        let count = session_race_participations::Entity::find()
            .filter(session_race_participations::Column::UserId.eq(&user_b))
            .count(&db)
            .await
            .unwrap();
        assert_eq!(count, 1, "forfeited participation row remains in DB");
    }

    #[tokio::test]
    async fn test_multiple_short_flaps_within_grace_preserve_pending() {
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        // Flap 1: leave, backdate left_at to 2 min ago, rejoin.
        leave_session(&db, &session.id, &user_b).await.unwrap();
        let two_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(2);
        backdate_participant(&db, &session.id, &user_b, None, Some(two_min_ago)).await;
        join_session(&db, &session.id, &user_b).await.unwrap();

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert_eq!(pending.len(), 1, "after first short flap, pending intact");

        // Flap 2: leave again, backdate left_at to 1 min ago, rejoin.
        leave_session(&db, &session.id, &user_b).await.unwrap();
        let one_min_ago = Utc::now().naive_utc() - chrono::Duration::minutes(1);
        backdate_participant(&db, &session.id, &user_b, None, Some(one_min_ago)).await;
        join_session(&db, &session.id, &user_b).await.unwrap();

        let pending = get_pending_races(&db, &session.id, &user_b).await.unwrap();
        assert_eq!(pending.len(), 1, "multiple short flaps preserve pending");
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
        let s = sessions::Entity::find_by_id(&session.id)
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
            .filter(session_race_participations::Column::UserId.eq(&host_id))
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
                    .add(session_race_participations::Column::SessionRaceId.eq(&race.id))
                    .add(session_race_participations::Column::UserId.eq(&host_id)),
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
                    .add(session_race_participations::Column::SessionRaceId.eq(&race.id))
                    .add(session_race_participations::Column::UserId.eq(&host_id)),
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
                    .add(session_race_participations::Column::SessionRaceId.eq(&race.id))
                    .add(session_race_participations::Column::UserId.eq(&host_id)),
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
        let bogus_race_id = SessionRaceId::new(Uuid::new_v4().to_string());
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
            drink_type_id::drink_type_uuid,
            entities::{drink_types, runs},
            test_helpers::seed_game_data,
        };

        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        // Insert a run row to satisfy the "already submitted" precondition.
        let drink_id = drink_type_uuid("Test Beer");
        drink_types::Entity::find_by_id(&drink_id)
            .one(&db)
            .await
            .unwrap()
            .expect("seed_game_data inserts Test Beer");
        runs::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(host_id.as_str().to_string()),
            session_race_id: Set(race.id.as_str().to_string()),
            track_id: Set(race.track_id),
            character_id: Set(1),
            body_id: Set(1),
            wheel_id: Set(1),
            glider_id: Set(1),
            track_time: Set(120_000),
            lap1_time: Set(40_000),
            lap2_time: Set(40_000),
            lap3_time: Set(40_000),
            drink_type_id: Set(drink_id),
            disqualified: Set(false),
            photo_path: Set(None),
            created_at: Set(Utc::now().naive_utc()),
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

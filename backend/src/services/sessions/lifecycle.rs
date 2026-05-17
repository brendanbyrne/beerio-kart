//! Session lifecycle: create / join / leave / list / close-stale.
//!
//! Operates on the session as an entity — existence, membership, host role.
//! Detail aggregation lives in [`super::detail`]; race orchestration in
//! [`super::races`].

use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use super::{
    detail::{SessionDetail, get_session_detail},
    types::RACE_WINDOW_HOURS,
};
use crate::{
    domain::{
        SessionId, UserId, Username,
        enums::{SessionRuleset, SessionStatus},
    },
    entities::{session_participants, sessions},
    error::Error,
    services::{helpers, notifications},
    timeout::{db_query, db_txn},
};

/// Row shape for the active-participant-in-active-session query.
#[derive(Debug, FromQueryResult)]
struct ActiveParticipantRow {
    session_id: String,
}

/// Check that the user is not already active in any *live* session.
/// Returns an error with the existing session ID if they are.
///
/// Delegates the liveness logic to [`get_active_session_id`] so both the
/// "are you in a session" answers come from one query — see that function
/// for the race-derived liveness predicate (ADR-0035).
async fn check_not_in_any_session(
    db: &impl ConnectionTrait,
    user_id: &UserId,
) -> Result<(), Error> {
    if let Some(session_id) = get_active_session_id(db, user_id).await? {
        return Err(Error::conflict(format!("Already in session {session_id}")));
    }
    Ok(())
}

/// Returns the session ID the user is currently active in, or None.
///
/// "Currently active in" means: the user has a `session_participants` row
/// with `left_at IS NULL`, the session's `status` is `'active'`, **and** the
/// session is race-derived *live* — it has a `session_races` row created
/// within the last [`RACE_WINDOW_HOURS`], or the session itself was created
/// that recently (the bootstrap case: a brand-new session with no race
/// chosen yet).
///
/// The liveness clause is what decouples user lockout from sweep timing
/// (ADR-0035): a user whose abandoned session has gone stale is freed
/// immediately by this read path, without waiting for `close_stale_sessions`
/// to flip `status` to `'closed'`.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(user_id = %user_id))]
pub async fn get_active_session_id(
    db: &impl ConnectionTrait,
    user_id: &UserId,
) -> Result<Option<SessionId>, Error> {
    let window_start = (Utc::now() - chrono::Duration::hours(RACE_WINDOW_HOURS)).naive_utc();

    let row = db_query(
        ActiveParticipantRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
        SELECT sp.session_id
        FROM session_participants sp
        JOIN sessions s ON sp.session_id = s.id
        WHERE sp.user_id = $1
          AND sp.left_at IS NULL
          AND s.status = 'active'
          AND (
            s.created_at >= $2
            OR EXISTS (
              SELECT 1 FROM session_races sr
              WHERE sr.session_id = s.id
                AND sr.created_at >= $2
            )
          )
        LIMIT 1
        "#,
            [user_id.into(), window_start.into()],
        ))
        .one(db),
    )
    .await?;

    row.map(|r| SessionId::from_db(&r.session_id)).transpose()
}

/// Mark the user's dangling participant row (`left_at IS NULL`) as left, if
/// one exists.
///
/// The partial unique index `idx_session_participants_one_active_session`
/// permits a user at most one `left_at IS NULL` participant row across all
/// sessions. A user abandoned in a session that has since gone race-derived
/// *stale* (ADR-0035) still holds such a row — and the `INSERT` of a fresh
/// participant row in `create_session` / `join_session` would collide with
/// that index before the periodic sweeper gets around to settling it.
///
/// Calling this inside the create/join transaction settles that dangling row
/// up front, so user lockout is genuinely decoupled from sweep timing — not
/// merely decoupled at the application-level `check_not_in_any_session`.
/// Semantically: a user implicitly leaves an abandoned session by starting or
/// joining a new one. The stale session's `status` and any other participants'
/// rows are left for the sweeper (eventual consistency — its remaining job).
///
/// **Why this is safe:** the caller has already passed
/// `check_not_in_any_session`, so any dangling row found here belonged to a
/// race-derived-stale session *as of that check*. The check runs before the
/// transaction, so a concurrent `next_track` from another participant could
/// in principle revive that session in the gap — but settling the row is
/// still correct regardless: the user is explicitly starting or joining a
/// new session, and the one-active-session invariant requires them to leave
/// the old one either way.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
async fn settle_dangling_participation(
    txn: &impl ConnectionTrait,
    user_id: &UserId,
) -> Result<(), Error> {
    let now = Utc::now().naive_utc();
    db_query(
        session_participants::Entity::update_many()
            .col_expr(session_participants::Column::LeftAt, Expr::value(now))
            .filter(
                Condition::all()
                    .add(session_participants::Column::UserId.eq(user_id))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .exec(txn),
    )
    .await?;
    Ok(())
}

/// Create a new session. The creator becomes both the host and the first
/// participant. Returns the full session detail.
///
/// # Errors
///
/// Returns `BadRequest` if `ruleset` doesn't parse as a known
/// [`SessionRuleset`], or if it parses to a variant whose track-selection
/// logic isn't implemented yet (everything except `Random` — see
/// [`SessionRuleset`] for the gate); `Conflict` if the user is already in
/// another active session; `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(user_id = %user_id, ruleset = %ruleset))]
pub async fn create_session(
    db: &DatabaseConnection,
    user_id: &UserId,
    ruleset: &str,
) -> Result<SessionDetail, Error> {
    let parsed: SessionRuleset = ruleset.parse()?;
    if parsed != SessionRuleset::Random {
        // The DB column accepts every variant, but the service layer only
        // knows how to drive `Random` sessions today. Reject the others
        // here rather than creating a structurally-broken session whose
        // ruleset the rest of the code has no path to honor.
        return Err(Error::bad_request(format!(
            "Ruleset '{ruleset}' is not yet supported",
        )));
    }

    check_not_in_any_session(db, user_id).await?;

    // `joined_at` is application-managed (not handled by `before_save` — see
    // `entities/sessions_behavior.rs` for why), so capture `now` here and
    // stamp it by hand. `sessions.created_at` is populated by
    // `ActiveModelBehavior::before_save`.
    let now = Utc::now().naive_utc();
    let session_id = SessionId::new_v4();

    let txn = db_txn(db.begin()).await?;

    // Settle any dangling row from a now-stale session so the new
    // participant INSERT doesn't collide with the one-active-session index.
    settle_dangling_participation(&txn, user_id).await?;

    db_query(
        sessions::ActiveModel {
            id: Set(session_id.into()),
            host_id: Set(user_id.into()),
            ruleset: Set(parsed),
            least_played_drink_category: Set(None),
            status: Set(SessionStatus::Active),
            created_at: NotSet,
        }
        .insert(&txn),
    )
    .await?;

    db_query(
        session_participants::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            session_id: Set(session_id.into()),
            user_id: Set(user_id.into()),
            joined_at: Set(now),
            left_at: Set(None),
        }
        .insert(&txn),
    )
    .await?;

    db_txn(txn.commit()).await?;

    get_session_detail(db, &session_id, Some(user_id)).await
}

/// Summary info for listing active sessions.
#[derive(serde::Serialize)]
pub struct SessionSummary {
    /// Session's stable UUID.
    pub id: SessionId,
    /// Display name of the session host (for showing "Bob's session" in lists).
    pub host_username: Username,
    /// Count of participants who haven't left (`left_at IS NULL`).
    pub participant_count: i64,
    /// 1-indexed race count: 1 means "no races completed; race 1 is up next".
    pub race_number: i64,
    /// Track-selection ruleset chosen at session creation.
    pub ruleset: SessionRuleset,
}

/// Row shape returned by the list-sessions JOIN query.
#[derive(Debug, FromQueryResult)]
struct SessionSummaryRow {
    id: String,
    host_username: String,
    participant_count: i64,
    race_count: i64,
    ruleset: SessionRuleset,
}

/// List active sessions, most recently active first.
///
/// "Activity" is race-derived (ADR-0035): sessions sort by their newest
/// `session_races.created_at`, falling back to `sessions.created_at` for a
/// session that has no races yet. Uses a single JOIN query instead of N+1.
///
/// Note this filters only on `status = 'active'` and does not apply the
/// race-derived liveness predicate — a session whose races have all expired
/// but that the sweeper hasn't closed yet still appears here until the next
/// sweep. That is acceptable for a listing surface; the frontend filters
/// near-stale sessions out of the join UI (ADR-0035 § Negative consequences).
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db))]
pub async fn list_active_sessions(db: &impl ConnectionTrait) -> Result<Vec<SessionSummary>, Error> {
    let rows = db_query(
        SessionSummaryRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
            db.get_database_backend(),
            r#"
        SELECT
            s.id,
            u.username AS host_username,
            COUNT(DISTINCT CASE WHEN sp.left_at IS NULL THEN sp.id END) AS participant_count,
            COUNT(DISTINCT sr.id) AS race_count,
            s.ruleset
        FROM sessions s
        JOIN users u ON s.host_id = u.id
        LEFT JOIN session_participants sp ON sp.session_id = s.id
        LEFT JOIN session_races sr ON sr.session_id = s.id
        WHERE s.status = 'active'
        GROUP BY s.id
        ORDER BY COALESCE(MAX(sr.created_at), s.created_at) DESC
        "#,
            [],
        ))
        .all(db),
    )
    .await?;

    rows.into_iter()
        .map(|r| {
            Ok(SessionSummary {
                id: SessionId::from_db(&r.id)?,
                host_username: Username::from_db(r.host_username, "users.username")?,
                participant_count: r.participant_count,
                race_number: r.race_count.max(1),
                ruleset: r.ruleset,
            })
        })
        .collect()
}

/// Join (or rejoin) a session. **Single mutable row per (session, user)** —
/// first join INSERTs, rejoin mutates the existing row.
///
/// Rejoin clears `left_at` unconditionally and never touches `joined_at`
/// (ADR-0035). `joined_at` is therefore monotonic — set once on first join,
/// it genuinely means "when this user first joined this session." Pending-race
/// access no longer depends on it: a flaked-out user's pending races stay
/// pending for as long as the session is alive (ADR-0037), regardless of how
/// long they were gone — they rejoin to act on them.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `Conflict` if the
/// session is closed or the user is already in another active session;
/// `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(session_id = %session_id, user_id = %user_id))]
pub async fn join_session(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<(), Error> {
    helpers::load_active_session(db, session_id)
        .await
        .map_err(|e| match e {
            Error::Conflict { .. } => Error::session_closed("Cannot join a closed session"),
            other => other,
        })?;
    check_not_in_any_session(db, user_id).await?;

    let existing = db_query(
        session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::UserId.eq(user_id)),
            )
            .one(db),
    )
    .await?;

    let now = Utc::now().naive_utc();
    let txn = db_txn(db.begin()).await?;

    // Settle any dangling row from a now-stale session so a fresh
    // participant INSERT doesn't collide with the one-active-session index.
    // A rejoin of *this* session is unaffected — its row has `left_at` set.
    settle_dangling_participation(&txn, user_id).await?;

    match existing {
        None => {
            db_query(
                session_participants::ActiveModel {
                    id: Set(Uuid::new_v4().to_string()),
                    session_id: Set(session_id.into()),
                    user_id: Set(user_id.into()),
                    joined_at: Set(now),
                    left_at: Set(None),
                }
                .insert(&txn),
            )
            .await?;
        }
        Some(row) => {
            if row.left_at.is_none() {
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
            }

            // Rejoin: clear `left_at`, leave `joined_at` untouched (monotonic).
            let mut active: session_participants::ActiveModel = row.into();
            active.left_at = Set(None);
            db_query(active.update(&txn)).await?;
        }
    }

    db_txn(txn.commit()).await?;

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
/// **Note on `joined_at` semantics:** `joined_at` is monotonic (ADR-0035) —
/// set once on first join and never reset, even across a leave/rejoin. So
/// "earliest-joined" genuinely means "first to ever join this session,"
/// which is a stable, well-defined host successor.
async fn transfer_host_or_close(
    txn: &impl ConnectionTrait,
    session_id: &SessionId,
    leaving_user_id: &UserId,
    is_host_leaving: bool,
) -> Result<HostDisposition, Error> {
    if is_host_leaving {
        let next_host = db_query(
            session_participants::Entity::find()
                .filter(
                    Condition::all()
                        .add(session_participants::Column::SessionId.eq(session_id))
                        .add(session_participants::Column::UserId.ne(leaving_user_id))
                        .add(session_participants::Column::LeftAt.is_null()),
                )
                .order_by_asc(session_participants::Column::JoinedAt)
                .one(txn),
        )
        .await?;

        match next_host {
            Some(new_host) => Ok(HostDisposition::TransferredTo(UserId::from_db(
                &new_host.user_id,
            )?)),
            None => Ok(HostDisposition::SessionClosed),
        }
    } else {
        let remaining = db_query(
            session_participants::Entity::find()
                .filter(
                    Condition::all()
                        .add(session_participants::Column::SessionId.eq(session_id))
                        .add(session_participants::Column::LeftAt.is_null()),
                )
                .count(txn),
        )
        .await?;

        if remaining == 0 {
            Ok(HostDisposition::SessionClosed)
        } else {
            Ok(HostDisposition::NoChange)
        }
    }
}

/// Row shape for the per-user pending-drop count query.
#[derive(Debug, FromQueryResult)]
struct PendingDropRow {
    user_id: String,
    dropped_count: i64,
}

/// Drop every unresolved pending race in a closing session and notify the
/// affected users (ADR-0037).
///
/// A pending race is *unresolved* for `(race, user)` when a
/// `session_race_participations` row exists with `skipped_at IS NULL`,
/// `dropped_at IS NULL`, and no `runs` row for that pair. Each such row gets
/// `dropped_at` stamped; each affected user gets one `PendingRacesDropped`
/// notification (ADR-0038) carrying their drop count.
///
/// `skipped` and `raced` rows are deliberately untouched — skip and submit
/// both beat drop, so the four-state status enum stays mutually exclusive.
///
/// **Caller contract:** must run inside the session-close transaction, before
/// `sessions.status` is flipped. If the drop UPDATE or any notification INSERT
/// fails, the whole close rolls back — drops are atomic with the close
/// (ADR-0037 / ADR-0038 § Atomicity).
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures on the count query, the drop
/// UPDATE, or any notification INSERT.
#[tracing::instrument(skip(txn), fields(session_id = %session_id))]
async fn close_session_and_drop_pending(
    txn: &impl ConnectionTrait,
    session_id: &SessionId,
) -> Result<(), Error> {
    let now = Utc::now().naive_utc();

    // Per-user count of rows about to be dropped — captured before the UPDATE
    // so each notification payload carries an accurate `dropped_count`. The
    // `NOT EXISTS` correlation is clumsy in the builder API, so this is
    // hand-rolled SQL (`seaorm.md` § 1).
    let drops: Vec<PendingDropRow> = db_query(
        PendingDropRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            SELECT srp.user_id AS user_id, COUNT(*) AS dropped_count
            FROM session_race_participations srp
            JOIN session_races sr ON srp.session_race_id = sr.id
            WHERE sr.session_id = $1
              AND srp.skipped_at IS NULL
              AND srp.dropped_at IS NULL
              AND NOT EXISTS (
                  SELECT 1 FROM runs r
                  WHERE r.session_race_id = srp.session_race_id
                    AND r.user_id = srp.user_id
              )
            GROUP BY srp.user_id
            "#,
            [session_id.into()],
        ))
        .all(txn),
    )
    .await?;

    if drops.is_empty() {
        // Nothing unresolved — a session can close with every race already
        // raced or skipped. No drops, no notifications.
        return Ok(());
    }

    // Stamp `dropped_at` on every unresolved pending row in this session, in
    // one set-based UPDATE. The predicate matches the count query above.
    db_query(txn.execute(sea_orm::Statement::from_sql_and_values(
        txn.get_database_backend(),
        r#"
        UPDATE session_race_participations
        SET dropped_at = $2
        WHERE session_race_id IN (
                  SELECT id FROM session_races WHERE session_id = $1
              )
          AND skipped_at IS NULL
          AND dropped_at IS NULL
          AND NOT EXISTS (
              SELECT 1 FROM runs r
              WHERE r.session_race_id = session_race_participations.session_race_id
                AND r.user_id = session_race_participations.user_id
          )
        "#,
        [session_id.into(), now.into()],
    )))
    .await?;

    // One notification per affected user (ADR-0038 consumer trigger).
    for drop in drops {
        let user_id = UserId::from_db(&drop.user_id)?;
        let dropped_count = u32::try_from(drop.dropped_count).map_err(|_| {
            Error::Internal(anyhow::anyhow!(
                "dropped_count {} does not fit in u32",
                drop.dropped_count
            ))
        })?;
        notifications::record_pending_drops(txn, &user_id, session_id, dropped_count).await?;
    }

    Ok(())
}

/// Leave a session. Sets `left_at` and handles host transfer.
///
/// # Errors
///
/// Returns `NotFound` if the session doesn't exist; `BadRequest` if the
/// user is not currently in the session; `Internal` for unexpected DB
/// failures or invariant violations (e.g., last-active-participant flow
/// failing to find any remaining participant to promote).
#[tracing::instrument(skip(db), fields(session_id = %session_id, user_id = %user_id))]
pub async fn leave_session(
    db: &DatabaseConnection,
    session_id: &SessionId,
    user_id: &UserId,
) -> Result<(), Error> {
    let session = db_query(sessions::Entity::find_by_id(session_id).one(db))
        .await?
        .ok_or_else(|| Error::NotFound("Session not found".to_string()))?;

    // require_active_participant returns Forbidden (authorization guard), but
    // leaving a session you're not in is bad input, not an auth failure.
    let participant = helpers::require_active_participant(db, session_id, user_id)
        .await
        .map_err(|_| Error::bad_request("Not currently in this session"))?;

    let now = Utc::now().naive_utc();
    let txn = db_txn(db.begin()).await?;

    let mut active_participant: session_participants::ActiveModel = participant.into();
    active_participant.left_at = Set(Some(now));
    db_query(active_participant.update(&txn)).await?;

    // Lift to typed before comparing — matches the pattern in
    // `session_context.rs::require_host` and surfaces a malformed UUID in
    // `sessions.host_id` as 500 rather than a silent false-negative compare.
    let host_id = UserId::from_db(&session.host_id)?;
    let is_host_leaving = host_id == *user_id;
    let disposition = transfer_host_or_close(&txn, session_id, user_id, is_host_leaving).await?;

    // Only write the session row when the disposition actually changes a
    // column. The session carries no activity timestamp to bump anymore
    // (ADR-0035), so the `NoChange` case has nothing to UPDATE.
    match disposition {
        HostDisposition::TransferredTo(new_host_id) => {
            let mut active_session: sessions::ActiveModel = session.into();
            active_session.host_id = Set((&new_host_id).into());
            db_query(active_session.update(&txn)).await?;
        }
        HostDisposition::SessionClosed => {
            // Drop unresolved pending races + notify affected users before
            // flipping status — same transaction (ADR-0037).
            close_session_and_drop_pending(&txn, session_id).await?;
            let mut active_session: sessions::ActiveModel = session.into();
            active_session.status = Set(SessionStatus::Closed);
            db_query(active_session.update(&txn)).await?;
        }
        HostDisposition::NoChange => {}
    }

    db_txn(txn.commit()).await?;

    Ok(())
}

/// Row shape for the stale-session SELECT.
#[derive(Debug, FromQueryResult)]
struct StaleSessionRow {
    id: String,
}

/// Close sessions whose activity window has fully elapsed (ADR-0035, ADR-0037).
///
/// A session is *stale* when it is `status = 'active'`, was created more than
/// [`RACE_WINDOW_HOURS`] ago, **and** has had no meaningful activity within
/// that same window — no new race, no submitted run, no join or leave, no
/// skipped pending race (the five-signal predicate from ADR-0037). The
/// `created_at` clause handles the bootstrap case: a brand-new session with
/// no race chosen yet keeps its first hour from its own creation timestamp
/// before it can go stale.
///
/// Since ADR-0037 removed the per-race timer from `get_pending_races`, this
/// sweep is the sole gatekeeper for closing dormant sessions — a pending race
/// stays submittable for as long as the session is alive, and the session is
/// alive until this predicate says otherwise. Clean exits still close their
/// session inline via `leave_session`; this handles the abandoned case
/// (phones died, tabs closed without a Leave tap).
///
/// For each closing session it drops every unresolved pending race
/// (`dropped_at` stamped) and records a `PendingRacesDropped` notification per
/// affected user — see [`close_session_and_drop_pending`]. It also marks all
/// remaining active participants as left. Returns the number of sessions
/// closed.
///
/// The cleanup runs in one transaction (`seaorm.md` § 1): a per-session drop
/// step (see [`close_session_and_drop_pending`]), then two set-based `UPDATE`s
/// — one to settle still-active participants, one to flip the session rows to
/// `Closed`. The list of stale ids is fetched once up front so every step
/// scopes to the same set without re-querying.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures on any of the SELECT or
/// UPDATE statements that drive the cleanup.
#[tracing::instrument(skip(db))]
pub async fn close_stale_sessions(db: &DatabaseConnection) -> Result<u64, Error> {
    let now = Utc::now().naive_utc();
    let window_start = now - chrono::Duration::hours(RACE_WINDOW_HOURS);

    let txn = db_txn(db.begin()).await?;

    // Capture the stale ids once. The `NOT EXISTS` subqueries are awkward in
    // the builder API, so this read is hand-rolled SQL (`seaorm.md` § 1 — raw
    // SQL for clumsy multi-table shapes). Both `UPDATE`s below scope to the
    // captured set, and the empty-set case short-circuits cleanly (the common
    // case).
    //
    // The predicate enumerates every meaningful activity signal a session can
    // produce (ADR-0037): a new race, a submitted run, a join or leave, or a
    // skipped pending race within the window all keep the session alive. With
    // the per-race timer gone from `get_pending_races`, the sweeper is the
    // sole gatekeeper for closing dormant sessions, so it honors all of them.
    // Kept in lockstep with the ETag formula in `api-contract.md` § 4 — the
    // two share a maintenance contract.
    let stale_ids: Vec<String> = db_query(
        StaleSessionRow::find_by_statement(sea_orm::Statement::from_sql_and_values(
            txn.get_database_backend(),
            r#"
            SELECT s.id
            FROM sessions s
            WHERE s.status = 'active'
              AND s.created_at < $1
              AND NOT EXISTS (
                  SELECT 1 FROM session_races sr
                  WHERE sr.session_id = s.id
                    AND sr.created_at >= $1
              )
              AND NOT EXISTS (
                  SELECT 1 FROM runs r
                  JOIN session_races sr ON r.session_race_id = sr.id
                  WHERE sr.session_id = s.id
                    AND r.created_at >= $1
              )
              AND NOT EXISTS (
                  SELECT 1 FROM session_participants sp
                  WHERE sp.session_id = s.id
                    AND (sp.joined_at >= $1 OR sp.left_at >= $1)
              )
              AND NOT EXISTS (
                  SELECT 1 FROM session_race_participations srp
                  JOIN session_races sr ON srp.session_race_id = sr.id
                  WHERE sr.session_id = s.id
                    AND srp.skipped_at >= $1
              )
            "#,
            [window_start.into()],
        ))
        .all(&txn),
    )
    .await?
    .into_iter()
    .map(|r| r.id)
    .collect();

    if stale_ids.is_empty() {
        db_txn(txn.commit()).await?;
        return Ok(0);
    }

    // Drop each closing session's unresolved pending races and notify the
    // affected users, before the batch status flip (ADR-0037 / ADR-0038).
    for id in &stale_ids {
        let session_id = SessionId::from_db(id)?;
        close_session_and_drop_pending(&txn, &session_id).await?;
    }

    // Mark all still-active participants of the stale sessions as left.
    db_query(
        session_participants::Entity::update_many()
            .col_expr(session_participants::Column::LeftAt, Expr::value(now))
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.is_in(stale_ids.clone()))
                    .add(session_participants::Column::LeftAt.is_null()),
            )
            .exec(&txn),
    )
    .await?;

    // Close the stale sessions in one statement (seaorm.md § 1 — the
    // exemplar for the set-based-update rule names this exact cleanup).
    let result = db_query(
        sessions::Entity::update_many()
            .col_expr(sessions::Column::Status, Expr::value(SessionStatus::Closed))
            .filter(sessions::Column::Id.is_in(stale_ids))
            .exec(&txn),
    )
    .await?;

    db_txn(txn.commit()).await?;

    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        domain::SessionRaceId,
        entities::{notifications as notifications_entity, session_race_participations},
        services::sessions::{next_track, skip_pending_race},
        test_helpers::{
            backdate_participant, backdate_session, create_user, insert_participant,
            insert_race_participation, insert_run, insert_session, insert_session_race,
            seed_game_data, seed_tracks_for_test, setup_db,
        },
    };

    // ── transfer_host_or_close ───────────────────────────────────────

    #[tokio::test]
    async fn test_transfer_host_leaves_with_successor() {
        let db = setup_db().await;
        let host = create_user(&db, "host").await;
        let user2 = create_user(&db, "user2").await;
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
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
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
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
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
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
        let session_id = insert_session(&db, &host, SessionStatus::Active).await;
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

        let updated = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.host_id, user2_id.to_string());
        assert_eq!(updated.status, SessionStatus::Active);
    }

    #[tokio::test]
    async fn test_host_transfer_closes_session_when_last_participant_leaves() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let updated = sessions::Entity::find_by_id(session.id)
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, SessionStatus::Closed);
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
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(user2_id)),
            )
            .one(&db)
            .await
            .unwrap()
            .unwrap();
        assert!(user2_row.left_at.is_some());

        let host_row = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(host_id)),
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
    async fn test_create_with_parseable_but_unsupported_ruleset_is_bad_request() {
        // `default` / `least_played` / `round_robin` are valid
        // `SessionRuleset` variants (the enum carries all four per the
        // compliance plan), but only `Random` is wired through the service.
        // Until the others land, the service rejects them at the gate with
        // a 400 carrying the offending name — distinct from the
        // "Invalid ruleset" message produced by `FromStr` on garbage input.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;

        for ruleset in ["default", "least_played", "round_robin"] {
            let result = create_session(&db, &host_id, ruleset).await;
            let Err(err) = result else {
                panic!("expected BadRequest for {ruleset}, got Ok(_)");
            };
            match err {
                Error::BadRequest { client, .. } => {
                    assert!(
                        client.contains(ruleset) && client.contains("not yet supported"),
                        "expected gate message for {ruleset}, got {client:?}",
                    );
                }
                other => panic!("expected BadRequest for {ruleset}, got {other:?}"),
            }
        }
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
        assert_eq!(s1_summary.host_username.as_ref(), "host");
        assert_eq!(s1_summary.race_number, 1);

        let s2_summary = summaries.iter().find(|s| s.id != s1.id).unwrap();
        assert_eq!(s2_summary.participant_count, 1);
        assert_eq!(s2_summary.host_username.as_ref(), "user2");
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

    // ── close_stale_sessions (race-derived sweeper, ADR-0035) ────────────

    /// Fetch a participant row by (session, user).
    async fn participant_row(
        db: &DatabaseConnection,
        session_id: &SessionId,
        user_id: &UserId,
    ) -> session_participants::Model {
        session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session_id))
                    .add(session_participants::Column::UserId.eq(user_id)),
            )
            .one(db)
            .await
            .unwrap()
            .unwrap()
    }

    /// Fetch a session's current status.
    async fn session_status(db: &DatabaseConnection, session_id: &SessionId) -> SessionStatus {
        sessions::Entity::find_by_id(session_id)
            .one(db)
            .await
            .unwrap()
            .unwrap()
            .status
    }

    #[tokio::test]
    async fn test_stale_cleanup_marks_participants_as_left() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;

        // A session created over an hour ago with no races and no recent
        // join/leave/run/skip is stale.
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        insert_participant(&db, &session_id, &user_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        // Backdate the joins too — a join within the window would itself keep
        // the session alive under the ADR-0037 activity predicate.
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;
        backdate_participant(&db, &session_id, &user_id, Some(two_hours_ago), None).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 1, "the stale session is closed");

        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Closed
        );
        assert!(
            participant_row(&db, &session_id, &host_id)
                .await
                .left_at
                .is_some(),
            "host participant marked as left"
        );
        assert!(
            participant_row(&db, &session_id, &user_id)
                .await
                .left_at
                .is_some(),
            "user participant marked as left"
        );
    }

    #[tokio::test]
    async fn test_close_stale_keeps_recent_bootstrap_session() {
        // A brand-new session with no races yet is alive for its first hour
        // from its own creation timestamp.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "fresh session is not stale");
        assert_eq!(
            session_status(&db, &session.id).await,
            SessionStatus::Active
        );
    }

    #[tokio::test]
    async fn test_close_stale_closes_old_bootstrap_session() {
        // A session created over an hour ago that never picked a track is
        // stale — the bootstrap window has elapsed.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 1);
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Closed
        );
    }

    #[tokio::test]
    async fn test_close_stale_keeps_session_with_recent_race() {
        // Even though the session was created over an hour ago, a race
        // created within the window keeps it alive.
        let db = setup_db().await;
        crate::test_helpers::seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        // Backdate the join so the recent race is the *only* live signal —
        // isolates the race clause of the activity predicate.
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;
        // A race created just now — well inside the window.
        insert_session_race(&db, &session_id, 1, 1, Utc::now().naive_utc()).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "a recent race keeps the session alive");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Active
        );
    }

    #[tokio::test]
    async fn test_close_stale_closes_session_with_all_races_expired() {
        // The session has a race, but it was created over an hour ago — the
        // race has expired and nothing keeps the session alive.
        let db = setup_db().await;
        crate::test_helpers::seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let three_hours_ago = (Utc::now() - chrono::Duration::hours(3)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, three_hours_ago).await;
        backdate_participant(&db, &session_id, &host_id, Some(three_hours_ago), None).await;
        insert_session_race(
            &db,
            &session_id,
            1,
            1,
            (Utc::now() - chrono::Duration::hours(2)).naive_utc(),
        )
        .await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 1, "all races expired → session closed");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Closed
        );
    }

    // ── Monotonic joined_at + lockout decoupling (ADR-0035) ──────────────

    #[tokio::test]
    async fn test_rejoin_does_not_advance_joined_at() {
        // `joined_at` is monotonic — leave/rejoin clears `left_at` but never
        // touches `joined_at`, regardless of how long the user was gone.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();

        let original_joined = participant_row(&db, &session.id, &user_b).await.joined_at;

        leave_session(&db, &session.id, &user_b).await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();

        let row = participant_row(&db, &session.id, &user_b).await;
        assert!(row.left_at.is_none(), "left_at cleared on rejoin");
        assert_eq!(
            row.joined_at, original_joined,
            "rejoin must never advance joined_at"
        );
    }

    #[tokio::test]
    async fn test_user_in_stale_session_can_create_fresh_session() {
        // The user still has an active participant row in a session that has
        // gone stale (created over an hour ago, no races) but that the
        // sweeper has not yet flipped to `closed`. Lockout must not depend on
        // sweep timing — the user can immediately create a new session.
        let db = setup_db().await;
        let user_id = create_user(&db, "user").await;
        let stale_id = insert_session(&db, &user_id, SessionStatus::Active).await;
        insert_participant(&db, &stale_id, &user_id, None).await;
        backdate_session(
            &db,
            &stale_id,
            (Utc::now() - chrono::Duration::hours(2)).naive_utc(),
        )
        .await;

        // Sanity: the stale session is still `active` in the DB.
        assert_eq!(session_status(&db, &stale_id).await, SessionStatus::Active);

        create_session(&db, &user_id, "random")
            .await
            .expect("user in a stale session may create a fresh one");

        // The dangling row in the stale session was settled — this pins
        // `settle_dangling_participation`'s direct effect, not just the
        // side effect that the new-session INSERT didn't collide.
        assert!(
            participant_row(&db, &stale_id, &user_id)
                .await
                .left_at
                .is_some(),
            "the dangling row in the stale session is settled"
        );
    }

    #[tokio::test]
    async fn test_user_in_stale_session_can_join_fresh_session() {
        let db = setup_db().await;
        let user_id = create_user(&db, "user").await;
        let other_host = create_user(&db, "other").await;
        let stale_id = insert_session(&db, &user_id, SessionStatus::Active).await;
        insert_participant(&db, &stale_id, &user_id, None).await;
        backdate_session(
            &db,
            &stale_id,
            (Utc::now() - chrono::Duration::hours(2)).naive_utc(),
        )
        .await;

        let fresh = create_session(&db, &other_host, "random").await.unwrap();
        join_session(&db, &fresh.id, &user_id)
            .await
            .expect("user in a stale session may join a fresh one");

        assert!(
            participant_row(&db, &stale_id, &user_id)
                .await
                .left_at
                .is_some(),
            "the dangling row in the stale session is settled"
        );
    }

    // ── Extended sweeper predicate: per-signal liveness (ADR-0037) ───────
    //
    // Each test backdates the session past the window and isolates one
    // activity signal within the window, asserting the session stays alive.
    // The all-signals-stale case is `test_stale_cleanup_marks_participants_as_left`.

    #[tokio::test]
    async fn test_close_stale_keeps_session_with_recent_run() {
        // A run submitted within the window keeps the session alive even
        // though the session and its race are both older than the window.
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, two_hours_ago).await;
        // Run created just now — well inside the window.
        insert_run(&db, &race_id, &host_id, 1).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "a recent run keeps the session alive");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Active
        );
    }

    #[tokio::test]
    async fn test_close_stale_keeps_session_with_recent_join() {
        // A participant who joined within the window keeps the session alive.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        backdate_session(
            &db,
            &session_id,
            (Utc::now() - chrono::Duration::hours(2)).naive_utc(),
        )
        .await;
        // `insert_participant` stamps `joined_at = now` — a recent join.
        insert_participant(&db, &session_id, &host_id, None).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "a recent join keeps the session alive");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Active
        );
    }

    #[tokio::test]
    async fn test_close_stale_keeps_session_with_recent_leave() {
        // A participant who left within the window keeps the session alive —
        // a recent leave is still recent activity.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        // Joined long ago, left just now.
        backdate_participant(
            &db,
            &session_id,
            &host_id,
            Some(two_hours_ago),
            Some(Utc::now().naive_utc()),
        )
        .await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "a recent leave keeps the session alive");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Active
        );
    }

    #[tokio::test]
    async fn test_close_stale_keeps_session_with_recent_skip() {
        // A pending race skipped within the window keeps the session alive.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, two_hours_ago).await;
        // Participation skipped just now — the skip is the only live signal.
        insert_race_participation(&db, &race_id, &host_id, Some(Utc::now().naive_utc())).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 0, "a recent skip keeps the session alive");
        assert_eq!(
            session_status(&db, &session_id).await,
            SessionStatus::Active
        );
    }

    // ── Drop-on-close + notification trigger (ADR-0037 / ADR-0038) ───────

    /// Fetch a (race, user) participation row.
    async fn participation_row(
        db: &DatabaseConnection,
        session_race_id: &SessionRaceId,
        user_id: &UserId,
    ) -> session_race_participations::Model {
        session_race_participations::Entity::find()
            .filter(
                Condition::all()
                    .add(session_race_participations::Column::SessionRaceId.eq(session_race_id))
                    .add(session_race_participations::Column::UserId.eq(user_id)),
            )
            .one(db)
            .await
            .unwrap()
            .unwrap()
    }

    /// Fetch all of a user's notification rows.
    async fn notification_rows(
        db: &DatabaseConnection,
        user_id: &UserId,
    ) -> Vec<notifications_entity::Model> {
        notifications_entity::Entity::find()
            .filter(notifications_entity::Column::UserId.eq(user_id))
            .all(db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn test_leave_close_drops_unresolved_pending() {
        // The solo host leaves with an unresolved pending race — the inline
        // close stamps `dropped_at` on the participation row.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let row = participation_row(&db, &race.id, &host_id).await;
        assert!(
            row.dropped_at.is_some(),
            "unresolved pending row is dropped"
        );
        assert!(row.skipped_at.is_none(), "drop does not touch skipped_at");
    }

    #[tokio::test]
    async fn test_close_does_not_drop_skipped_rows() {
        // A row already resolved by an explicit skip is not re-stamped as
        // dropped — skip beats drop, the states stay mutually exclusive.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();
        skip_pending_race(&db, &session.id, &race.id, &host_id)
            .await
            .unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let row = participation_row(&db, &race.id, &host_id).await;
        assert!(row.skipped_at.is_some(), "skipped_at preserved");
        assert!(
            row.dropped_at.is_none(),
            "an already-skipped row is not dropped"
        );
        assert!(
            notification_rows(&db, &host_id).await.is_empty(),
            "a skipped-only close drops nothing, so notifies nobody"
        );
    }

    #[tokio::test]
    async fn test_close_does_not_drop_submitted_rows() {
        // A row resolved by a submitted run is not dropped.
        let db = setup_db().await;
        seed_game_data(&db).await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        let race = next_track(&db, &session.id, &host_id).await.unwrap();
        insert_run(&db, &race.id, &host_id, race.track_id).await;

        leave_session(&db, &session.id, &host_id).await.unwrap();

        let row = participation_row(&db, &race.id, &host_id).await;
        assert!(
            row.dropped_at.is_none(),
            "a submitted (raced) row is not dropped"
        );
        assert!(
            notification_rows(&db, &host_id).await.is_empty(),
            "nothing unresolved, so no drop notification"
        );
    }

    #[tokio::test]
    async fn test_close_records_one_notification_per_affected_user() {
        // A two-user session with two unresolved races for each user. When
        // the last participant leaves, each user gets exactly one
        // notification carrying their own drop count.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user2_id = create_user(&db, "user2").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user2_id).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();
        next_track(&db, &session.id, &host_id).await.unwrap();

        // user2 leaves first (session stays open), then host leaves (closes).
        leave_session(&db, &session.id, &user2_id).await.unwrap();
        leave_session(&db, &session.id, &host_id).await.unwrap();

        for user in [&host_id, &user2_id] {
            let rows = notification_rows(&db, user).await;
            assert_eq!(rows.len(), 1, "exactly one notification per affected user");
            assert_eq!(rows[0].kind, "pending_races_dropped");
            let payload: crate::services::notifications::NotificationPayload =
                serde_json::from_str(&rows[0].payload).unwrap();
            assert_eq!(
                payload,
                crate::services::notifications::NotificationPayload::PendingRacesDropped {
                    session_id: session.id,
                    dropped_count: 2,
                },
                "payload carries the session and the per-user drop count"
            );
        }
    }

    #[tokio::test]
    async fn test_close_empty_session_records_no_notifications() {
        // A session with no unresolved pending races (here: no races at all)
        // closes without recording any notification.
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();

        leave_session(&db, &session.id, &host_id).await.unwrap();

        assert_eq!(
            session_status(&db, &session.id).await,
            SessionStatus::Closed
        );
        assert!(
            notification_rows(&db, &host_id).await.is_empty(),
            "closing an empty session notifies nobody"
        );
    }

    #[tokio::test]
    async fn test_sweeper_drops_pending_and_notifies() {
        // The abandoned-session path: the stale-session sweeper closes the
        // session, drops the unresolved pending row, and notifies the user.
        let db = setup_db().await;
        seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let session_id = insert_session(&db, &host_id, SessionStatus::Active).await;
        insert_participant(&db, &session_id, &host_id, None).await;
        backdate_session(&db, &session_id, two_hours_ago).await;
        backdate_participant(&db, &session_id, &host_id, Some(two_hours_ago), None).await;
        let race_id = insert_session_race(&db, &session_id, 1, 1, two_hours_ago).await;
        insert_race_participation(&db, &race_id, &host_id, None).await;

        let closed = close_stale_sessions(&db).await.unwrap();
        assert_eq!(closed, 1, "the abandoned session is swept closed");

        let row = participation_row(&db, &race_id, &host_id).await;
        assert!(row.dropped_at.is_some(), "sweeper drops the pending row");

        let rows = notification_rows(&db, &host_id).await;
        assert_eq!(rows.len(), 1, "sweeper records the drop notification");
        assert_eq!(rows[0].kind, "pending_races_dropped");
    }
}

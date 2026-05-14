//! Session lifecycle: create / join / leave / list / close-stale.
//!
//! Operates on the session as an entity — existence, membership, host role.
//! Detail aggregation lives in [`super::detail`]; race orchestration in
//! [`super::races`].

use chrono::{DateTime, NaiveDateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, Condition, ConnectionTrait,
    DatabaseConnection, EntityTrait, FromQueryResult, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait, sea_query::Expr,
};
use uuid::Uuid;

use super::{
    detail::{SessionDetail, get_session_detail},
    types::REJOIN_GRACE_MINUTES,
};
use crate::{
    domain::{
        SessionId, UserId, Username,
        enums::{SessionRuleset, SessionStatus},
    },
    entities::{session_participants, sessions},
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
#[tracing::instrument(skip(db), fields(user_id = %user_id))]
pub async fn get_active_session_id(
    db: &impl ConnectionTrait,
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

    row.map(|r| SessionId::from_db(&r.session_id)).transpose()
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

    // `last_activity_at` and `joined_at` are application-managed (not handled
    // by `before_save` — see `entities/sessions_behavior.rs` for why), so
    // capture `now` here and stamp them by hand. `sessions.created_at` is
    // populated by `ActiveModelBehavior::before_save`.
    let now = Utc::now().naive_utc();
    let session_id = SessionId::new_v4();

    let txn = db.begin().await?;

    sessions::ActiveModel {
        id: Set(session_id.into()),
        host_id: Set(user_id.into()),
        ruleset: Set(parsed),
        least_played_drink_category: Set(None),
        status: Set(SessionStatus::Active),
        created_at: NotSet,
        last_activity_at: Set(now),
    }
    .insert(&txn)
    .await?;

    session_participants::ActiveModel {
        id: Set(Uuid::new_v4().to_string()),
        session_id: Set(session_id.into()),
        user_id: Set(user_id.into()),
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
    /// Most recent activity timestamp, used by the stale-session cleanup.
    pub last_activity_at: DateTime<Utc>,
}

/// Row shape returned by the list-sessions JOIN query.
#[derive(Debug, FromQueryResult)]
struct SessionSummaryRow {
    id: String,
    host_username: String,
    participant_count: i64,
    race_count: i64,
    ruleset: SessionRuleset,
    last_activity_at: NaiveDateTime,
}

/// List active sessions sorted by `last_activity_at` DESC.
/// Uses a single JOIN query instead of N+1 queries.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db))]
pub async fn list_active_sessions(db: &impl ConnectionTrait) -> Result<Vec<SessionSummary>, Error> {
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

    rows.into_iter()
        .map(|r| {
            Ok(SessionSummary {
                id: SessionId::from_db(&r.id)?,
                host_username: Username::from_db(r.host_username, "users.username")?,
                participant_count: r.participant_count,
                race_number: r.race_count.max(1),
                ruleset: r.ruleset,
                last_activity_at: r.last_activity_at.and_utc(),
            })
        })
        .collect()
}

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
#[tracing::instrument(skip(db), fields(session_id = %session_id, user_id = %user_id))]
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
                session_id: Set(session_id.into()),
                user_id: Set(user_id.into()),
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
            Some(new_host) => Ok(HostDisposition::TransferredTo(UserId::from_db(
                &new_host.user_id,
            )?)),
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
#[tracing::instrument(skip(db), fields(session_id = %session_id, user_id = %user_id))]
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

    // Lift to typed before comparing — matches the pattern in
    // `session_context.rs::require_host` and surfaces a malformed UUID in
    // `sessions.host_id` as 500 rather than a silent false-negative compare.
    let host_id = UserId::from_db(&session.host_id)?;
    let is_host_leaving = host_id == *user_id;
    let mut active_session: sessions::ActiveModel = session.into();
    let disposition = transfer_host_or_close(&txn, session_id, user_id, is_host_leaving).await?;

    match disposition {
        HostDisposition::TransferredTo(new_host_id) => {
            active_session.host_id = Set((&new_host_id).into());
        }
        HostDisposition::SessionClosed => {
            active_session.status = Set(SessionStatus::Closed);
        }
        HostDisposition::NoChange => {}
    }

    active_session.last_activity_at = Set(now);
    active_session.update(&txn).await?;

    txn.commit().await?;

    Ok(())
}

/// Close sessions that have had no activity for over an hour.
///
/// Also marks all remaining active participants as left, preventing
/// users from being soft-locked out of creating/joining new sessions.
/// Returns the number of sessions closed.
///
/// The cleanup runs as two set-based `UPDATE`s in one transaction
/// (`seaorm.md` § 1) — one to flip the matching session rows to
/// `Closed`, one to settle their still-active participants. The list
/// of stale ids is fetched once up front so both `UPDATE`s can scope
/// to the same set without re-querying.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures on any of the SELECT or
/// UPDATE statements that drive the cleanup.
#[tracing::instrument(skip(db))]
pub async fn close_stale_sessions(db: &DatabaseConnection) -> Result<u64, Error> {
    let one_hour_ago = (Utc::now() - chrono::Duration::hours(1)).naive_utc();
    let now = Utc::now().naive_utc();

    let txn = db.begin().await?;

    // Capture the stale ids once. We can't derive them from the sessions
    // `update_many`'s result (it returns a count, not a row set), and using
    // a subquery in the participants filter pulls the same SELECT into the
    // engine implicitly. The explicit form keeps the filter expressions
    // readable and short-circuits cleanly when the cleanup has nothing to
    // do (the common case).
    let stale_ids: Vec<String> = sessions::Entity::find()
        .select_only()
        .column(sessions::Column::Id)
        .filter(
            Condition::all()
                .add(sessions::Column::Status.eq(SessionStatus::Active))
                .add(sessions::Column::LastActivityAt.lt(one_hour_ago)),
        )
        .into_tuple()
        .all(&txn)
        .await?;

    if stale_ids.is_empty() {
        txn.commit().await?;
        return Ok(0);
    }

    // Mark all still-active participants of the stale sessions as left.
    session_participants::Entity::update_many()
        .col_expr(session_participants::Column::LeftAt, Expr::value(now))
        .filter(
            Condition::all()
                .add(session_participants::Column::SessionId.is_in(stale_ids.clone()))
                .add(session_participants::Column::LeftAt.is_null()),
        )
        .exec(&txn)
        .await?;

    // Close the stale sessions in one statement (seaorm.md § 1 — the
    // exemplar for the set-based-update rule names this exact cleanup).
    let result = sessions::Entity::update_many()
        .col_expr(sessions::Column::Status, Expr::value(SessionStatus::Closed))
        .filter(sessions::Column::Id.is_in(stale_ids))
        .exec(&txn)
        .await?;

    txn.commit().await?;

    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        services::sessions::get_pending_races,
        test_helpers::{
            backdate_participant, create_user, insert_participant, insert_session, setup_db,
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

    #[tokio::test]
    async fn test_stale_cleanup_marks_participants_as_left() {
        let db = setup_db().await;
        let host_id = create_user(&db, "host").await;
        let user_id = create_user(&db, "user").await;

        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_id).await.unwrap();

        // Backdate last_activity_at past the stale threshold
        let two_hours_ago = (Utc::now() - chrono::Duration::hours(2)).naive_utc();
        let s = sessions::Entity::find_by_id(session.id)
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

    // ── Rejoin grace (verifies join_session's grace-window logic by
    //    inspecting downstream pending state) ───────────────────────────

    #[tokio::test]
    async fn test_rejoin_within_grace_preserves_pending() {
        let db = setup_db().await;
        crate::test_helpers::seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        crate::services::sessions::next_track(&db, &session.id, &host_id)
            .await
            .unwrap();

        // Capture B's joined_at, then leave + backdate left_at to 3 min ago.
        let original_joined = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(user_b)),
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
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(user_b)),
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
        use crate::entities::session_race_participations;

        let db = setup_db().await;
        crate::test_helpers::seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        crate::services::sessions::next_track(&db, &session.id, &host_id)
            .await
            .unwrap();

        let original_joined = session_participants::Entity::find()
            .filter(
                Condition::all()
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(user_b)),
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
                    .add(session_participants::Column::SessionId.eq(session.id))
                    .add(session_participants::Column::UserId.eq(user_b)),
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
            .filter(session_race_participations::Column::UserId.eq(user_b))
            .count(&db)
            .await
            .unwrap();
        assert_eq!(count, 1, "forfeited participation row remains in DB");
    }

    #[tokio::test]
    async fn test_multiple_short_flaps_within_grace_preserve_pending() {
        let db = setup_db().await;
        crate::test_helpers::seed_tracks_for_test(&db).await;
        let host_id = create_user(&db, "host").await;
        let user_b = create_user(&db, "b").await;
        let session = create_session(&db, &host_id, "random").await.unwrap();
        join_session(&db, &session.id, &user_b).await.unwrap();
        crate::services::sessions::next_track(&db, &session.id, &host_id)
            .await
            .unwrap();

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
}

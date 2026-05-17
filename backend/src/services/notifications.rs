//! Notifications service (ADR-0038).
//!
//! A per-user inbox of asynchronous events. Each event materializes one
//! `notifications` row inside the same transaction as the triggering write —
//! no worker, no queue, no async dispatch (ADR-0038 § Generation pattern).
//!
//! The module exposes:
//! - [`NotificationPayload`] — the closed, serde-tagged enum of event kinds.
//! - [`record`] — the generic typed constructor every event site calls.
//! - [`record_pending_drops`] — the ADR-0037 consumer (pending races dropped
//!   on session close).
//! - [`list_notifications`] / [`unread_count`] / [`mark_all_read`] — the read
//!   and dismiss surface backing the three `/me/notifications` endpoints.
//!
//! For MVP the enum carries a single variant (`PendingRacesDropped`); the
//! other kinds named in ADR-0038 (`H2hLeadChanged`, `TrackRecordLost`,
//! `LeaderboardRankChanged`) are deliberately deferred to future cups.

use chrono::{DateTime, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, QuerySelect, Set, sea_query::Expr,
};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{NotificationId, SessionId, UserId},
    entities::notifications,
    error::Error,
    timeout::db_query,
};

/// Hard cap on rows returned by [`list_notifications`].
///
/// ADR-0038 § API surface specifies cursor pagination per ADR-0032 for
/// `GET /me/notifications`. No cursor-pagination infrastructure exists in the
/// codebase yet — `GET /runs` (the ADR-0032 exemplar) also still uses a flat
/// `LIMIT 100` cap. This endpoint follows that precedent rather than building
/// keyset pagination for one endpoint in isolation; revisiting it project-wide
/// is tracked as a follow-up. At friend-group scale (ADR-0038 estimates
/// ~18k notification rows/year across ~10 users) a 100-row cap comfortably
/// covers the unread inbox and a deep history scroll.
const NOTIFICATIONS_PAGE_LIMIT: u64 = 100;

/// Closed set of notification event kinds, with serde-tagged serialization.
///
/// `kind` is the serde tag — lifted into the `notifications.kind` column for
/// indexing — and the rest of each variant body is the JSON payload stored in
/// `notifications.payload`. Adding a kind is adding a variant: the compiler
/// then surfaces every site that pattern-matches.
///
/// MVP carries only [`Self::PendingRacesDropped`]. The future variants from
/// ADR-0038 § Rust modeling land with their respective cups.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NotificationPayload {
    /// The user's session closed while they still had unresolved pending
    /// races; those races were dropped (ADR-0037).
    PendingRacesDropped {
        /// The session that closed.
        session_id: SessionId,
        /// How many of the user's pending races were dropped.
        dropped_count: u32,
    },
}

impl NotificationPayload {
    /// The `snake_case` discriminator stored in the `kind` column.
    ///
    /// Mirrors the serde tag — keep the two in lockstep when adding variants.
    const fn kind_str(&self) -> &'static str {
        match self {
            Self::PendingRacesDropped { .. } => "pending_races_dropped",
        }
    }
}

/// Wire view of a single notification — the JSON shape returned by
/// `GET /me/notifications`.
///
/// The stored `kind` column is intentionally not surfaced separately: it is
/// already present as the serde tag inside `payload`.
#[derive(Debug, Serialize)]
pub struct NotificationView {
    /// Stable UUID of the notification row.
    pub id: NotificationId,
    /// When the event was recorded, UTC.
    pub created_at: DateTime<Utc>,
    /// When the user dismissed it, or `None` while unread.
    pub read_at: Option<DateTime<Utc>>,
    /// Kind-tagged structured payload.
    pub payload: NotificationPayload,
}

/// Parse a stored notification row into its wire view.
///
/// A `payload` column that doesn't deserialize as a [`NotificationPayload`]
/// is data corruption (hand-edited row, or a write from a future schema) and
/// surfaces as `Internal`.
fn row_into_view(row: &notifications::Model) -> Result<NotificationView, Error> {
    let payload: NotificationPayload = serde_json::from_str(&row.payload).map_err(|e| {
        Error::Internal(anyhow::Error::new(e).context("Deserializing notification payload"))
    })?;
    Ok(NotificationView {
        id: NotificationId::from_db(&row.id)?,
        created_at: row.created_at.and_utc(),
        read_at: row.read_at.map(|t| t.and_utc()),
        payload,
    })
}

/// Record a notification for `user_id`.
///
/// The INSERT runs on whatever connection / transaction handle is passed: an
/// event site that already holds a transaction passes it so the notification
/// is atomic with the triggering write (ADR-0038 § Atomicity).
///
/// # Errors
///
/// Returns `Internal` if the payload fails to serialize (should not happen
/// for the closed enum) or for unexpected DB failures on the INSERT.
#[tracing::instrument(skip(txn, payload), fields(user_id = %user_id, kind = payload.kind_str()))]
pub async fn record(
    txn: &impl ConnectionTrait,
    user_id: &UserId,
    payload: &NotificationPayload,
) -> Result<(), Error> {
    let payload_json = serde_json::to_string(payload).map_err(|e| {
        Error::Internal(anyhow::Error::new(e).context("Serializing notification payload"))
    })?;

    db_query(
        notifications::ActiveModel {
            id: Set(NotificationId::new_v4().into()),
            user_id: Set(user_id.into()),
            kind: Set(payload.kind_str().to_string()),
            payload: Set(payload_json),
            created_at: NotSet,
            read_at: Set(None),
        }
        .insert(txn),
    )
    .await?;

    Ok(())
}

/// Record a [`NotificationPayload::PendingRacesDropped`] for one affected
/// user. Called once per user inside the session-close transaction (ADR-0037).
///
/// # Errors
///
/// Propagates the errors of [`record`].
#[tracing::instrument(
    skip(txn),
    fields(user_id = %user_id, session_id = %session_id, dropped_count),
)]
pub async fn record_pending_drops(
    txn: &impl ConnectionTrait,
    user_id: &UserId,
    session_id: &SessionId,
    dropped_count: u32,
) -> Result<(), Error> {
    record(
        txn,
        user_id,
        &NotificationPayload::PendingRacesDropped {
            session_id: *session_id,
            dropped_count,
        },
    )
    .await
}

/// List a user's notifications, newest first.
///
/// Unread-only by default; `include_read = true` returns the full history.
/// Capped at [`NOTIFICATIONS_PAGE_LIMIT`] rows.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures or a corrupt stored payload.
#[tracing::instrument(skip(db), fields(user_id = %user_id, include_read))]
pub async fn list_notifications(
    db: &impl ConnectionTrait,
    user_id: &UserId,
    include_read: bool,
) -> Result<Vec<NotificationView>, Error> {
    let mut query = notifications::Entity::find().filter(notifications::Column::UserId.eq(user_id));
    if !include_read {
        query = query.filter(notifications::Column::ReadAt.is_null());
    }

    let rows = db_query(
        query
            // Compound ordering keeps a stable sequence when two rows share a
            // `created_at` (sub-millisecond inserts) — `id` is the tie-break.
            .order_by_desc(notifications::Column::CreatedAt)
            .order_by_desc(notifications::Column::Id)
            .limit(NOTIFICATIONS_PAGE_LIMIT)
            .all(db),
    )
    .await?;

    rows.iter().map(row_into_view).collect()
}

/// Count a user's unread notifications. Backs the home-screen badge poll.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(user_id = %user_id))]
pub async fn unread_count(db: &impl ConnectionTrait, user_id: &UserId) -> Result<u64, Error> {
    let count = db_query(
        notifications::Entity::find()
            .filter(notifications::Column::UserId.eq(user_id))
            .filter(notifications::Column::ReadAt.is_null())
            .count(db),
    )
    .await?;
    Ok(count)
}

/// Mark every unread notification for `user_id` as read.
///
/// One set-based UPDATE (`seaorm.md` § 1) scoped to the requesting user, so
/// it never touches another user's inbox. Returns the number of rows flipped.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip(db), fields(user_id = %user_id))]
pub async fn mark_all_read(db: &impl ConnectionTrait, user_id: &UserId) -> Result<u64, Error> {
    let now = Utc::now().naive_utc();
    let result = db_query(
        notifications::Entity::update_many()
            .col_expr(notifications::Column::ReadAt, Expr::value(now))
            .filter(notifications::Column::UserId.eq(user_id))
            .filter(notifications::Column::ReadAt.is_null())
            .exec(db),
    )
    .await?;
    Ok(result.rows_affected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_user, setup_db};

    #[test]
    fn test_pending_races_dropped_round_trips_through_json() {
        let session_id = SessionId::new_v4();
        let payload = NotificationPayload::PendingRacesDropped {
            session_id,
            dropped_count: 3,
        };
        let json = serde_json::to_string(&payload).expect("serialize");
        let back: NotificationPayload = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(payload, back);
    }

    #[test]
    fn test_pending_races_dropped_json_carries_kind_tag() {
        let payload = NotificationPayload::PendingRacesDropped {
            session_id: SessionId::new_v4(),
            dropped_count: 2,
        };
        let value: serde_json::Value = serde_json::to_value(&payload).expect("serialize to value");
        assert_eq!(value["kind"], "pending_races_dropped");
        assert_eq!(value["dropped_count"], 2);
        assert_eq!(payload.kind_str(), "pending_races_dropped");
    }

    #[tokio::test]
    async fn test_record_inserts_a_row_with_kind_and_payload() {
        let db = setup_db().await;
        let user = create_user(&db, "user").await;
        let session_id = SessionId::new_v4();

        record(
            &db,
            &user,
            &NotificationPayload::PendingRacesDropped {
                session_id,
                dropped_count: 4,
            },
        )
        .await
        .expect("record succeeds");

        let rows = notifications::Entity::find()
            .filter(notifications::Column::UserId.eq(user))
            .all(&db)
            .await
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].kind, "pending_races_dropped");
        assert!(rows[0].read_at.is_none(), "fresh notification is unread");

        let stored: NotificationPayload = serde_json::from_str(&rows[0].payload).unwrap();
        assert_eq!(
            stored,
            NotificationPayload::PendingRacesDropped {
                session_id,
                dropped_count: 4,
            }
        );
    }

    #[tokio::test]
    async fn test_list_returns_unread_newest_first() {
        let db = setup_db().await;
        let user = create_user(&db, "user").await;

        // Three notifications; insert order is the chronological order.
        for n in 1..=3u32 {
            record(
                &db,
                &user,
                &NotificationPayload::PendingRacesDropped {
                    session_id: SessionId::new_v4(),
                    dropped_count: n,
                },
            )
            .await
            .unwrap();
            // `created_at` is millisecond-resolution; nudge so ordering is
            // unambiguous rather than relying on the `id` tie-break.
            tokio::time::sleep(std::time::Duration::from_millis(2)).await;
        }

        let list = list_notifications(&db, &user, false).await.unwrap();
        assert_eq!(list.len(), 3);
        let counts: Vec<u32> = list
            .iter()
            .map(|n| match n.payload {
                NotificationPayload::PendingRacesDropped { dropped_count, .. } => dropped_count,
            })
            .collect();
        assert_eq!(counts, vec![3, 2, 1], "newest first");
    }

    #[tokio::test]
    async fn test_list_excludes_read_unless_include_read() {
        let db = setup_db().await;
        let user = create_user(&db, "user").await;
        record(
            &db,
            &user,
            &NotificationPayload::PendingRacesDropped {
                session_id: SessionId::new_v4(),
                dropped_count: 1,
            },
        )
        .await
        .unwrap();

        mark_all_read(&db, &user).await.unwrap();

        let unread_only = list_notifications(&db, &user, false).await.unwrap();
        assert!(unread_only.is_empty(), "read rows excluded by default");

        let all = list_notifications(&db, &user, true).await.unwrap();
        assert_eq!(all.len(), 1, "include_read returns read rows");
        assert!(all[0].read_at.is_some());
    }

    #[tokio::test]
    async fn test_unread_count_tracks_unread_rows() {
        let db = setup_db().await;
        let user = create_user(&db, "user").await;
        assert_eq!(unread_count(&db, &user).await.unwrap(), 0);

        for _ in 0..2 {
            record(
                &db,
                &user,
                &NotificationPayload::PendingRacesDropped {
                    session_id: SessionId::new_v4(),
                    dropped_count: 1,
                },
            )
            .await
            .unwrap();
        }
        assert_eq!(unread_count(&db, &user).await.unwrap(), 2);

        mark_all_read(&db, &user).await.unwrap();
        assert_eq!(unread_count(&db, &user).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn test_mark_all_read_does_not_bleed_across_users() {
        let db = setup_db().await;
        let alice = create_user(&db, "alice").await;
        let bob = create_user(&db, "bob").await;
        for user in [&alice, &bob] {
            record(
                &db,
                user,
                &NotificationPayload::PendingRacesDropped {
                    session_id: SessionId::new_v4(),
                    dropped_count: 1,
                },
            )
            .await
            .unwrap();
        }

        let flipped = mark_all_read(&db, &alice).await.unwrap();
        assert_eq!(flipped, 1, "only alice's row flips");
        assert_eq!(unread_count(&db, &alice).await.unwrap(), 0);
        assert_eq!(
            unread_count(&db, &bob).await.unwrap(),
            1,
            "bob's inbox is untouched"
        );
    }

    #[tokio::test]
    async fn test_record_pending_drops_writes_expected_payload() {
        let db = setup_db().await;
        let user = create_user(&db, "user").await;
        let session_id = SessionId::new_v4();

        record_pending_drops(&db, &user, &session_id, 5)
            .await
            .expect("record_pending_drops succeeds");

        let list = list_notifications(&db, &user, false).await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(
            list[0].payload,
            NotificationPayload::PendingRacesDropped {
                session_id,
                dropped_count: 5,
            }
        );
    }
}

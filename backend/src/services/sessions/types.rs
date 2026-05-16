//! Shared DTOs and constants for the sessions submodules.
//!
//! Holds items that are produced by [`super::races`] mutations / queries and
//! consumed by [`super::detail`] aggregation (or vice versa). Living at a
//! layer below both submodules breaks what would otherwise be a
//! `detail` ↔ `races` cycle on [`SessionRaceInfo`] / [`get_pending_races`],
//! and a `races` ↔ `lifecycle` cycle on [`RACE_WINDOW_HOURS`].
//!
//! [`get_pending_races`]: super::races::get_pending_races

use chrono::{DateTime, Utc};

use crate::domain::{ImagePath, SessionRaceId, UserId, Username};

/// The single timeout that anchors session lifetime (ADR-0035).
///
/// A `session_races` row is submittable for this many hours from its
/// `created_at`; after that the race is expired and drops out of every
/// pending list. The same window doubles as the session bootstrap grace:
/// a brand-new session with no race chosen yet is considered alive for
/// this long from its own `created_at`.
///
/// Consumed by [`super::lifecycle`] (the stale-session sweeper and the two
/// "are you in a session" liveness predicates) and [`super::races`]
/// (`get_pending_races`). Defined here so neither submodule depends on the
/// other for it.
pub const RACE_WINDOW_HOURS: i64 = 1;

/// Submission info for a single participant in a race.
#[derive(serde::Serialize, Clone)]
pub struct RaceSubmission {
    /// Participant whose submission this is.
    pub user_id: UserId,
    /// Cached username for display (saves a JOIN on the read path).
    pub username: Username,
    /// Total race time in milliseconds.
    pub track_time: i32,
    /// `true` if the run was disqualified per the drink rule.
    pub disqualified: bool,
}

/// Info about a single race in the session (returned on create / skip / poll).
#[derive(serde::Serialize, Clone)]
pub struct SessionRaceInfo {
    /// Stable UUID of the race row.
    pub id: SessionRaceId,
    /// 1-indexed position of this race within the session.
    pub race_number: i32,
    /// FK to `tracks.id`.
    pub track_id: i32,
    /// Cached track name (saves a JOIN on the read path).
    pub track_name: String,
    /// Cached parent-cup name for display.
    pub cup_name: String,
    /// Relative image path for the track's preview thumbnail.
    pub image_path: ImagePath,
    /// Race-creation timestamp (when `next_track` was called), UTC.
    pub created_at: DateTime<Utc>,
    /// Per-participant submissions; empty until runs come in.
    pub submissions: Vec<RaceSubmission>,
}

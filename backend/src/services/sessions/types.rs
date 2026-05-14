//! Shared DTOs and constants for the sessions submodules.
//!
//! Holds items that are produced by [`super::races`] mutations / queries and
//! consumed by [`super::detail`] aggregation (or vice versa). Living at a
//! layer below both submodules breaks what would otherwise be a
//! `detail` ↔ `races` cycle on [`SessionRaceInfo`] / [`get_pending_races`],
//! and a `races` → `lifecycle` cycle on [`REJOIN_GRACE_MINUTES`].
//!
//! [`get_pending_races`]: super::races::get_pending_races

use chrono::{DateTime, Utc};

use crate::domain::{ImagePath, SessionRaceId, UserId, Username};

/// Submission info for a single participant in a race.
#[derive(serde::Serialize, Clone)]
pub struct RaceSubmission {
    pub user_id: UserId,
    pub username: Username,
    pub track_time: i32,
    pub disqualified: bool,
}

/// Info about a single race in the session (returned on create / skip / poll).
#[derive(serde::Serialize, Clone)]
pub struct SessionRaceInfo {
    pub id: SessionRaceId,
    pub race_number: i32,
    pub track_id: i32,
    pub track_name: String,
    pub cup_name: String,
    pub image_path: ImagePath,
    pub created_at: DateTime<Utc>,
    pub submissions: Vec<RaceSubmission>,
}

/// Grace window for "rejoin without losing pre-leave pending races."
///
/// Within this window of `left_at`, rejoining preserves `joined_at` (and
/// therefore preserves access to pre-leave pending races, per the §3 grace
/// semantics). After this window, `joined_at` is reset to `NOW()`, forfeiting
/// any pre-gap pending records.
///
/// Defined here (not in [`super::lifecycle`], where it semantically
/// originates) because both [`super::lifecycle::join_session`] and the
/// pending-races query in [`super::races`] consume it; living at the shared
/// layer keeps the submodule dependency graph acyclic.
pub const REJOIN_GRACE_MINUTES: i64 = 5;

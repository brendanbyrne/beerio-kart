//! Session services — split by concern.
//!
//! - [`lifecycle`]: create, join, leave, host transfer, close-stale, listing.
//! - [`detail`]: aggregated read for the polling endpoint
//!   (`get_session_detail`, `list_races`) plus the shared `SessionRaceInfo` /
//!   `RaceSubmission` / `RaceInfo` / `ParticipantInfo` / `SessionDetail` DTOs.
//! - [`races`]: race orchestration (next/skip track, pending races,
//!   skip-pending).
//!
//! All public items are re-exported here so external callers continue to use
//! `crate::services::sessions::<name>`.

mod detail;
mod lifecycle;
mod races;

pub use detail::*;
pub use lifecycle::*;
pub use races::*;

//! Session services — split by concern.
//!
//! - [`types`]: shared DTOs and constants (`SessionRaceInfo`,
//!   `RaceSubmission`, `RACE_WINDOW_HOURS`). Lives below the other
//!   submodules so the dependency graph stays acyclic.
//! - [`lifecycle`]: create, join, leave, host transfer, close-stale, listing.
//! - [`detail`]: aggregated read for the polling endpoint
//!   (`get_session_detail`, `list_races`) plus its read-only DTOs
//!   (`SessionDetail`, `ParticipantInfo`, `RaceInfo`).
//! - [`races`]: race orchestration (next/skip track, pending races,
//!   skip-pending).
//!
//! All public items are re-exported here so external callers continue to use
//! `crate::services::sessions::<name>`.

mod detail;
mod lifecycle;
mod races;
mod types;

pub use detail::*;
pub use lifecycle::*;
pub use races::*;
pub use types::*;

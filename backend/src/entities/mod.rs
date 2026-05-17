//! Hand-written `SeaORM` entities.
//!
//! Schema-as-code source of truth lives in `migration/`; entities mirror that
//! shape and are edited alongside the migration in the same PR. See
//! `docs/coding-standards/seaorm.md` § 6.

pub mod prelude;

pub mod bodies;
pub mod characters;
pub mod cups;
pub mod drink_types;
pub mod gliders;
pub mod notifications;
pub mod run_flags;
pub mod runs;
pub mod session_participants;
pub mod session_race_participations;
pub mod session_races;
pub mod sessions;
pub mod tracks;
pub mod users;
pub mod wheels;

// Sibling `ActiveModelBehavior` impls for entities with `created_at` /
// `updated_at` timestamps. Centralizes the timestamp-stamping logic so
// service code never sets these by hand. See `docs/coding-standards/seaorm.md`
// § 1 (and PR-E1 / Issue #137 for the migration that introduced this split).
mod drink_types_behavior;
mod notifications_behavior;
mod run_flags_behavior;
mod runs_behavior;
mod session_race_participations_behavior;
mod session_races_behavior;
mod sessions_behavior;
mod users_behavior;

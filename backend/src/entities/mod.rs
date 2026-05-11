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
pub mod run_flags;
pub mod runs;
pub mod session_participants;
pub mod session_race_participations;
pub mod session_races;
pub mod sessions;
pub mod tracks;
pub mod users;
pub mod wheels;

/// Authentication primitives: password hashing/verification, JWT issuance.
pub mod auth;
/// Cross-service helpers (lookup-or-404, etc.) that don't fit one resource.
pub mod helpers;
/// Run-recording service: submission + read-side queries.
pub mod runs;
/// Shared session-context type passed through nested service calls.
pub mod session_context;
/// Session-lifecycle, race-orchestration, and detail-read services.
pub mod sessions;
/// User-profile reads and updates.
pub mod users;

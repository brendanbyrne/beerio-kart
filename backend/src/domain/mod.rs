/// String-backed game enums (`SessionStatus`, `SessionRuleset`, `DrinkCategory`, `RunFlagReason`).
pub mod enums;
/// UUID- and integer-backed ID newtypes per [`rust.md` § 2].
pub mod ids;
/// Numeric domain types ([`LapTimeMs`], [`RaceTimeMs`]) with non-zero validation.
pub mod numeric;
/// Per-session character/vehicle/glider race setup chosen at session creation.
pub mod race_setup;
/// Validated string newtypes (`Username`, `EmailAddress`, etc.).
pub mod strings;

pub use ids::{
    BodyId, CharacterId, CupId, DrinkTypeId, GliderId, NotificationId, RunFlagId, RunId, SessionId,
    SessionParticipantId, SessionRaceId, TrackId, UserId, WheelId,
};
pub(crate) use numeric::assert_lap_sum;
pub use numeric::{LapTimeMs, MAX_TIME_MS, MIN_TIME_MS, RaceTimeMs};
pub use strings::{
    DrinkTypeName, EmailAddress, ImagePath, Password, PasswordHash, RunNotes, Username,
};

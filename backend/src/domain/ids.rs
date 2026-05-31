//! Domain ID newtypes.
//!
//! Each ID column on the database has a corresponding newtype here so the
//! compiler can catch "wrong-id-in-this-arg" bugs. UUID-backed columns wrap
//! [`uuid::Uuid`]; integer-backed columns wrap [`i32`]. Per
//! `docs/coding-standards/seaorm.md` § 6, entities keep SeaORM-default
//! primitives (`String` / `i32`) and conversion happens at the entity↔service
//! boundary inside the service layer.
//!
//! The [`nutype`] macro provides the derives. The `Serialize` / `Deserialize`
//! derives delegate transparently to the inner type, so the wire format is
//! unchanged: a UUID-backed ID serializes as the canonical hyphenated UUID
//! string, an integer-backed ID as a JSON number. Deserialize is the parsing
//! step the standards (`rust.md` § 2) describe as "parse, don't validate" —
//! a malformed UUID at the HTTP boundary becomes a 422 from axum's JSON
//! extractor before the handler runs.
//!
//! The manual [`From`] impls let `UserId` (etc.) flow through `SeaORM` call
//! sites without explicit unwrapping. Both owned and borrowed forms are
//! covered so callers can pick whichever reads best at the use site:
//!
//! - `From<Self>` / `From<&Self> for sea_orm::Value` — for
//!   `Column::Foo.eq(id)` filter expressions and parameter slots in
//!   `find_by_statement` raw SQL.
//! - `From<Self>` / `From<&Self> for String` (UUID newtypes) and
//!   `for i32` (integer newtypes) — for `find_by_id(id)` on entities whose
//!   primary-key `ValueType` is the matching primitive.
//!
//! UUID newtypes also carry a [`new_v4`](UserId::new_v4) constructor that
//! generates a fresh random UUID, and [`parse_db_id`] is the boundary helper
//! that reads an entity's `String` ID column into a newtype, mapping a bad
//! UUID in the database to [`Error::Internal`] (corruption — should never
//! happen, but we'd rather surface a clear 500 than silently return wrong
//! data).
//!
//! [`new_v4`]: UserId::new_v4

use nutype::nutype;
use sea_orm::Value;

use crate::error::Error;

/// Parse a UUID column read from the database into a typed newtype.
///
/// Used at the entity↔service boundary when materializing a domain object
/// from a `SeaORM` `Model`. A failure here means a malformed UUID slipped into
/// the database (data corruption or hand-edited rows), so it surfaces as
/// [`Error::Internal`] with the column name in the context.
///
/// # Errors
///
/// Returns [`Error::Internal`] if `s` is not a valid UUID string. The
/// resulting `anyhow::Error` chain carries the uuid parse error as its source
/// plus a static context naming the column (e.g. `"Invalid UUID in users.id"`).
pub(crate) fn parse_db_id<T>(s: &str, column: &'static str) -> Result<T, Error>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    s.parse::<T>().map_err(|e| {
        Error::Internal(anyhow::Error::new(e).context(format!("Invalid UUID in {column}")))
    })
}

// ── UUID-backed ID newtypes ─────────────────────────────────────────────
//
// Stored on disk as TEXT (canonical hyphenated form). The `nutype` macro
// gives us the standard set of derives — see the module docs for what each
// brings. The manual `From` impls below adapt the type to SeaORM's
// primary-key and filter-value APIs.

macro_rules! uuid_id_newtype {
    ($name:ident, $column_label:literal) => {
        #[nutype(derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            Display,
            AsRef,
            Serialize,
            Deserialize,
            FromStr,
            From,
        ))]
        pub struct $name(uuid::Uuid);

        impl $name {
            /// Generate a fresh random ID (UUID v4).
            #[must_use]
            pub fn new_v4() -> Self {
                Self::new(uuid::Uuid::new_v4())
            }

            /// Parse a UUID column from a `SeaORM` entity model.
            ///
            /// Convenience wrapper around [`parse_db_id`] that names the
            /// column for the error message.
            ///
            /// # Errors
            ///
            /// Returns [`Error::Internal`] if the stored string isn't a
            /// valid UUID. Data corruption only — should never fire in
            /// normal operation.
            pub fn from_db(s: &str) -> Result<Self, Error> {
                parse_db_id(s, $column_label)
            }
        }

        impl From<&$name> for Value {
            fn from(id: &$name) -> Self {
                Value::String(Some(Box::new(id.as_ref().to_string())))
            }
        }

        impl From<$name> for Value {
            fn from(id: $name) -> Self {
                Value::from(&id)
            }
        }

        impl From<&$name> for String {
            fn from(id: &$name) -> Self {
                id.as_ref().to_string()
            }
        }

        impl From<$name> for String {
            fn from(id: $name) -> Self {
                id.as_ref().to_string()
            }
        }
    };
}

uuid_id_newtype!(UserId, "users.id");
uuid_id_newtype!(SessionId, "sessions.id");
uuid_id_newtype!(RunId, "runs.id");
uuid_id_newtype!(SessionRaceId, "session_races.id");
uuid_id_newtype!(SessionParticipantId, "session_participants.id");
uuid_id_newtype!(RunFlagId, "run_flags.id");
uuid_id_newtype!(DrinkTypeId, "drink_types.id");
uuid_id_newtype!(NotificationId, "notifications.id");

// ── Integer-backed ID newtypes ──────────────────────────────────────────
//
// Stored on disk as INTEGER. Lookup-table IDs (tracks, characters, etc.)
// are pre-seeded small integers (1..=N), not auto-incremented surrogates —
// see `data-model.md` and ADR 0002.

macro_rules! i32_id_newtype {
    ($name:ident) => {
        #[nutype(derive(
            Debug,
            Clone,
            Copy,
            PartialEq,
            Eq,
            Hash,
            Display,
            AsRef,
            Serialize,
            Deserialize,
            FromStr,
            From,
        ))]
        pub struct $name(i32);

        impl From<&$name> for Value {
            fn from(id: &$name) -> Self {
                Value::Int(Some(*id.as_ref()))
            }
        }

        impl From<$name> for Value {
            fn from(id: $name) -> Self {
                Value::Int(Some(*id.as_ref()))
            }
        }

        impl From<&$name> for i32 {
            fn from(id: &$name) -> Self {
                *id.as_ref()
            }
        }

        impl From<$name> for i32 {
            fn from(id: $name) -> Self {
                *id.as_ref()
            }
        }
    };
}

i32_id_newtype!(TrackId);
i32_id_newtype!(CharacterId);
i32_id_newtype!(BodyId);
i32_id_newtype!(WheelId);
i32_id_newtype!(GliderId);
i32_id_newtype!(CupId);

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use uuid::Uuid;

    use super::*;

    #[test]
    fn test_user_id_serializes_as_bare_uuid_string() {
        let raw = Uuid::new_v4();
        let id = UserId::new(raw);
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{raw}\""));
    }

    #[test]
    fn test_user_id_deserializes_from_bare_uuid_string() {
        let raw = Uuid::new_v4();
        let json = format!("\"{raw}\"");
        let id: UserId = serde_json::from_str(&json).unwrap();
        assert_eq!(*id.as_ref(), raw);
    }

    #[test]
    fn test_user_id_deserialize_rejects_non_uuid_string() {
        // The whole point of wrapping `Uuid` instead of `String` is that the
        // boundary deserializer parses; a non-UUID payload must fail.
        let json = "\"not-a-uuid\"";
        let result: Result<UserId, _> = serde_json::from_str(json);
        result.expect_err("non-UUID string must fail deserialization");
    }

    #[test]
    fn test_id_types_construction_smoke() {
        // Construction smoke test — NOT a test of the type-distinctness
        // property. `UserId` and `SessionId` share an inner type (`Uuid`) but
        // are distinct *types*; that one can't be passed where the other is
        // expected is enforced by the compiler, not by this runtime test (a
        // `trybuild` compile-fail case is the way to assert that mechanically).
        // What this pins: each type's `new` / `as_ref` round-trips the inner
        // UUID, and crossing types requires an explicit detour through it.
        let raw = Uuid::new_v4();
        let user_id = UserId::new(raw);
        let session_id = SessionId::new(*user_id.as_ref());
        assert_eq!(*user_id.as_ref(), raw);
        assert_eq!(*session_id.as_ref(), raw);
    }

    #[test]
    fn test_user_id_hash_and_eq() {
        use std::collections::HashSet;

        let raw = Uuid::new_v4();
        let mut set = HashSet::new();
        set.insert(UserId::new(raw));
        assert!(set.contains(&UserId::new(raw)));
        assert!(!set.contains(&UserId::new(Uuid::new_v4())));
    }

    #[test]
    fn test_user_id_into_sea_orm_value() {
        // `&UserId` and `UserId` both convert to `sea_orm::Value::String`
        // so `Column::Foo.eq(...)` and `find_by_id(...)` accept newtypes
        // without ceremony. The textual form is the canonical hyphenated
        // UUID — matching the column type (`Text`) on disk.
        let raw = Uuid::new_v4();
        let owned = UserId::new(raw);

        let by_ref: Value = (&owned).into();
        let by_value: Value = owned.into();

        let expected = Value::String(Some(Box::new(raw.to_string())));
        assert_eq!(by_ref, expected);
        assert_eq!(by_value, expected);
    }

    #[test]
    fn test_track_id_into_sea_orm_value() {
        let id = TrackId::new(42);

        let by_ref: Value = (&id).into();
        let by_value: Value = id.into();

        assert_eq!(by_ref, Value::Int(Some(42)));
        assert_eq!(by_value, Value::Int(Some(42)));
    }

    #[test]
    fn test_user_id_into_string_canonical_form() {
        let raw = Uuid::new_v4();
        let id = UserId::new(raw);
        let s: String = (&id).into();
        assert_eq!(s, raw.to_string());
    }

    #[test]
    fn test_track_id_into_i32() {
        let id = TrackId::new(7);
        let n: i32 = (&id).into();
        assert_eq!(n, 7);
    }

    #[test]
    fn test_user_id_from_db_round_trip() {
        let raw = Uuid::new_v4();
        let stored: String = UserId::new(raw).into();
        let recovered = UserId::from_db(&stored).expect("valid UUID parses");
        assert_eq!(*recovered.as_ref(), raw);
    }

    #[test]
    fn test_user_id_from_db_rejects_malformed_uuid() {
        let err = UserId::from_db("not-a-uuid").unwrap_err();
        let Error::Internal(chain) = err else {
            panic!("expected Internal, got something else");
        };
        let rendered = format!("{chain:#}");
        assert!(
            rendered.contains("users.id"),
            "context should name the column: {rendered}"
        );
    }

    // Property test: UserId round-trips through serde JSON for any UUID input.
    // The whole "parse, don't validate" story rests on this being lossless —
    // if a deserialize-then-serialize cycle ever differs, the wire contract
    // has silently broken. proptest generates 256 random UUIDs by default.
    proptest! {
        #[test]
        fn test_user_id_serde_round_trip(bytes in any::<[u8; 16]>()) {
            let raw = Uuid::from_bytes(bytes);
            let id = UserId::new(raw);
            let json = serde_json::to_string(&id).unwrap();
            let recovered: UserId = serde_json::from_str(&json).unwrap();
            prop_assert_eq!(id, recovered);
        }
    }
}

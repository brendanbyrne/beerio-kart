use serde::{Deserialize, Serialize};

/// Generate a string-backed newtype for an ID.
///
/// The newtype is transparent for serde (so it serializes as a bare string),
/// implements `Display`, `AsRef<str>`, `Deref<Target = str>`, and conversion
/// from `String` / `&str`. `Deref` lets call sites treat the newtype as a
/// `&str` for read-only operations (e.g. passing into SeaORM filters that
/// expect `&str`).
macro_rules! string_id_newtype {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl std::ops::Deref for $name {
            type Target = str;

            fn deref(&self) -> &str {
                &self.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }
    };
}

string_id_newtype!(UserId);
string_id_newtype!(SessionId);
string_id_newtype!(RunId);
string_id_newtype!(SessionRaceId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn user_id_round_trip() {
        let id = UserId::new("abc");
        assert_eq!(id.as_str(), "abc");
        assert_eq!(id.to_string(), "abc");
        assert_eq!(id.clone().into_string(), "abc".to_string());
    }

    #[test]
    fn session_id_deref_to_str() {
        let id = SessionId::new("xyz-123");
        // Deref<Target = str> lets us use &str methods directly.
        assert_eq!(id.len(), 7);
        assert!(id.starts_with("xyz"));
        // AsRef<str> for API ergonomics.
        let as_ref: &str = id.as_ref();
        assert_eq!(as_ref, "xyz-123");
    }

    #[test]
    fn from_string_and_str() {
        let a: RunId = "r1".into();
        let b: RunId = String::from("r1").into();
        assert_eq!(a, b);
    }

    #[test]
    fn serde_is_transparent() {
        let id = UserId::new("u-9");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"u-9\"");
        let parsed: UserId = serde_json::from_str("\"u-9\"").unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn different_newtypes_are_not_interchangeable() {
        // Compile-time property: UserId and SessionId do not unify. Documented
        // here so future refactors don't unknowingly reach for `From<UserId>
        // for SessionId` or similar. If you need to convert between them,
        // it should be deliberate via `.as_str()` or `.into_string()`.
        let user = UserId::new("u");
        let session: SessionId = SessionId::new(user.as_str());
        assert_eq!(session.as_str(), "u");
    }

    #[test]
    fn hash_and_eq_work() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SessionRaceId::new("sr-1"));
        assert!(set.contains(&SessionRaceId::new("sr-1")));
        assert!(!set.contains(&SessionRaceId::new("sr-2")));
    }
}

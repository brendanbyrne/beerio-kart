use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

/// Lifecycle state of a session.
///
/// Stored in the DB as the strings returned by `as_str`. An unknown value read
/// from the DB is treated as `Internal` (corruption or schema drift), not user
/// input, so it bubbles up as a 500.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    Active,
    Closed,
}

impl SessionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Closed => "closed",
        }
    }
}

impl fmt::Display for SessionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for SessionStatus {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "closed" => Ok(Self::Closed),
            other => Err(AppError::Internal(format!(
                "Unknown session status: {other}"
            ))),
        }
    }
}

/// Session scheduling ruleset. Only `Random` is supported in Phase 3; future
/// variants (e.g. `RoundRobin`) belong here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Ruleset {
    Random,
}

impl Ruleset {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Random => "random",
        }
    }
}

impl fmt::Display for Ruleset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Ruleset {
    type Err = AppError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "random" => Ok(Self::Random),
            other => Err(AppError::BadRequest(format!("Invalid ruleset: '{other}'"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_status_parses_known_values() {
        assert_eq!(
            SessionStatus::from_str("active").unwrap(),
            SessionStatus::Active
        );
        assert_eq!(
            SessionStatus::from_str("closed").unwrap(),
            SessionStatus::Closed
        );
    }

    #[test]
    fn session_status_unknown_is_internal_error() {
        let err = SessionStatus::from_str("nonsense").unwrap_err();
        match err {
            AppError::Internal(msg) => assert!(msg.contains("nonsense")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }

    #[test]
    fn session_status_round_trip() {
        for status in [SessionStatus::Active, SessionStatus::Closed] {
            let s = status.as_str();
            assert_eq!(SessionStatus::from_str(s).unwrap(), status);
            assert_eq!(status.to_string(), s);
        }
    }

    #[test]
    fn session_status_serde_lowercase() {
        let json = serde_json::to_string(&SessionStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: SessionStatus = serde_json::from_str("\"closed\"").unwrap();
        assert_eq!(parsed, SessionStatus::Closed);
    }

    #[test]
    fn ruleset_parses_known_values() {
        assert_eq!(Ruleset::from_str("random").unwrap(), Ruleset::Random);
    }

    #[test]
    fn ruleset_unknown_is_bad_request() {
        // Unknown ruleset is user input (from API), so it's a 400, not a 500 —
        // deliberately different from SessionStatus, which is read from the DB.
        let err = Ruleset::from_str("spiral").unwrap_err();
        match err {
            AppError::BadRequest(msg) => assert!(msg.contains("spiral")),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn ruleset_round_trip() {
        let s = Ruleset::Random.as_str();
        assert_eq!(Ruleset::from_str(s).unwrap(), Ruleset::Random);
        assert_eq!(Ruleset::Random.to_string(), s);
    }
}

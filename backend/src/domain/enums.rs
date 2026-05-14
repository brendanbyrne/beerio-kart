//! String-valued domain enums backed by [`sea_orm::DeriveActiveEnum`].
//!
//! Each variant carries a `string_value` that is what the DB column stores
//! (TEXT) and what serde emits on the wire (a bare lowercase / `snake_case`
//! string — matching what the frontend already sends and reads). The
//! [`sea_orm::DeriveActiveEnum`] derive lets these types be used directly
//! as the column type on an entity ([`crate::entities::sessions`],
//! [`crate::entities::run_flags`]), so the entity↔service boundary keeps
//! its newtype-on-the-Rust-side, primitive-in-the-DB shape — without the
//! ad-hoc `as_str` / `to_string` plumbing the previous string-typed model
//! needed at every call site.
//!
//! See `coding-standards/rust.md` § 2 (parse-don't-validate) and § 14
//! (`snake_case` wire format), and `coding-standards/seaorm.md` § 5
//! (enumerations).

use std::str::FromStr;

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::error::Error;

/// Lifecycle state of a session.
///
/// Stored in `sessions.status` as the lowercase strings declared below.
/// Wire shape mirrors the storage shape via `#[serde(rename_all = "lowercase")]`.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "closed")]
    Closed,
}

/// How a session selects tracks. Stored in `sessions.ruleset` as the
/// `snake_case` strings declared below. See `docs/data-model.md` § Session
/// Rulesets for the per-variant behavior.
///
/// Only [`Self::Random`] is wired through `create_session` today — the
/// other variants are defined so the type is complete (per the
/// compliance plan), but the service layer rejects them with a
/// `BadRequest("Ruleset … is not yet supported")` until their product
/// behavior lands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum SessionRuleset {
    #[sea_orm(string_value = "random")]
    Random,
    #[sea_orm(string_value = "default")]
    Default,
    #[sea_orm(string_value = "least_played")]
    LeastPlayed,
    #[sea_orm(string_value = "round_robin")]
    RoundRobin,
}

impl FromStr for SessionRuleset {
    type Err = Error;

    /// Parse a ruleset string submitted by a client. Unknown values are
    /// `Error::BadRequest` (client-input fault), not `Error::Internal`.
    ///
    /// Kept as a manual impl rather than going through serde because the
    /// route DTO holds `ruleset: String` — see issue [#146] for the typed
    /// Json extractor error-envelope mismatch this currently avoids.
    ///
    /// [#146]: https://github.com/brendanbyrne/beerio-kart/issues/146
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "random" => Ok(Self::Random),
            "default" => Ok(Self::Default),
            "least_played" => Ok(Self::LeastPlayed),
            "round_robin" => Ok(Self::RoundRobin),
            other => Err(Error::bad_request(format!("Invalid ruleset: '{other}'"))),
        }
    }
}

/// Whether a drink contains alcohol.
///
/// Stored on `drink_types.alcoholic` as a boolean, *and* on
/// `sessions.least_played_drink_category` as a TEXT — this enum maps the
/// latter. The boolean column stays a bool: there are only ever two
/// values, so the enum-vs-bool trade lands on bool for that case.
///
/// See `docs/data-model.md` notes: the frontend renders `non_alcoholic`
/// as the hyphenated `"non-alcoholic"`; the wire and DB use the
/// underscored form.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum DrinkCategory {
    #[sea_orm(string_value = "alcoholic")]
    Alcoholic,
    #[sea_orm(string_value = "non_alcoholic")]
    NonAlcoholic,
}

/// Why a run was flagged. Stored on `run_flags.reason` as the `snake_case`
/// strings declared below.
///
/// Five user-initiated variants (chosen from a dropdown when a user flags
/// their own run) plus one auto-generated variant emitted by the
/// record-without-photo detection. See `docs/data-model.md` § `run_flags`
/// for the spelled-out display text the frontend renders for each.
///
/// No write path consumes this yet — the type is defined here so the
/// `run_flags.reason` column is enum-typed end-to-end the moment the
/// flag-creation flow lands.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, EnumIter, DeriveActiveEnum, Serialize, Deserialize,
)]
#[sea_orm(rs_type = "String", db_type = "Text")]
#[serde(rename_all = "snake_case")]
pub enum RunFlagReason {
    #[sea_orm(string_value = "time_is_incorrect")]
    TimeIsIncorrect,
    #[sea_orm(string_value = "wrong_track")]
    WrongTrack,
    #[sea_orm(string_value = "wrong_race_setup")]
    WrongRaceSetup,
    #[sea_orm(string_value = "wrong_drink_type")]
    WrongDrinkType,
    #[sea_orm(string_value = "other")]
    Other,
    #[sea_orm(string_value = "record_requires_photo_verification")]
    RecordRequiresPhotoVerification,
}

#[cfg(test)]
mod tests {
    use sea_orm::{ActiveEnum, Iterable};

    use super::*;

    #[test]
    fn test_session_status_string_values_round_trip_through_sea_orm() {
        // Spot-check both variants. `to_value()` is the DB-bound form
        // produced by `DeriveActiveEnum`; `try_from_value` is the parse-
        // from-DB form. A mismatch here would mean the entity column
        // would silently fail to load on read.
        for status in SessionStatus::iter() {
            let value = status.to_value();
            let parsed = SessionStatus::try_from_value(&value).expect("round-trip");
            assert_eq!(parsed, status);
        }
    }

    #[test]
    fn test_session_status_serde_uses_lowercase_string() {
        let json = serde_json::to_string(&SessionStatus::Active).unwrap();
        assert_eq!(json, "\"active\"");
        let parsed: SessionStatus = serde_json::from_str("\"closed\"").unwrap();
        assert_eq!(parsed, SessionStatus::Closed);
    }

    #[test]
    fn test_session_ruleset_parses_each_known_variant() {
        assert_eq!(
            SessionRuleset::from_str("random").unwrap(),
            SessionRuleset::Random,
        );
        assert_eq!(
            SessionRuleset::from_str("default").unwrap(),
            SessionRuleset::Default,
        );
        assert_eq!(
            SessionRuleset::from_str("least_played").unwrap(),
            SessionRuleset::LeastPlayed,
        );
        assert_eq!(
            SessionRuleset::from_str("round_robin").unwrap(),
            SessionRuleset::RoundRobin,
        );
    }

    #[test]
    fn test_session_ruleset_unknown_is_bad_request_with_input_value() {
        // Unknown ruleset is client input (from JSON body), so it surfaces
        // as a 400 with the offending value in the message — distinct from
        // a DB-read failure which would be Internal.
        let err = SessionRuleset::from_str("spiral").unwrap_err();
        match err {
            Error::BadRequest { client, .. } => assert!(client.contains("spiral")),
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_session_ruleset_string_values_round_trip_through_sea_orm() {
        for ruleset in SessionRuleset::iter() {
            let value = ruleset.to_value();
            let parsed = SessionRuleset::try_from_value(&value).expect("round-trip");
            assert_eq!(parsed, ruleset);
        }
    }

    #[test]
    fn test_session_ruleset_serde_emits_snake_case() {
        assert_eq!(
            serde_json::to_string(&SessionRuleset::Random).unwrap(),
            "\"random\"",
        );
        assert_eq!(
            serde_json::to_string(&SessionRuleset::LeastPlayed).unwrap(),
            "\"least_played\"",
        );
        assert_eq!(
            serde_json::to_string(&SessionRuleset::RoundRobin).unwrap(),
            "\"round_robin\"",
        );
    }

    #[test]
    fn test_drink_category_round_trip_through_sea_orm_and_serde() {
        for category in DrinkCategory::iter() {
            let value = category.to_value();
            let parsed = DrinkCategory::try_from_value(&value).expect("round-trip");
            assert_eq!(parsed, category);

            let json = serde_json::to_string(&category).unwrap();
            let recovered: DrinkCategory = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, category);
        }

        // Spot-check the snake_case spelling the schema commits to.
        assert_eq!(
            serde_json::to_string(&DrinkCategory::NonAlcoholic).unwrap(),
            "\"non_alcoholic\"",
        );
    }

    #[test]
    fn test_run_flag_reason_round_trip_through_sea_orm_and_serde() {
        for reason in RunFlagReason::iter() {
            let value = reason.to_value();
            let parsed = RunFlagReason::try_from_value(&value).expect("round-trip");
            assert_eq!(parsed, reason);

            let json = serde_json::to_string(&reason).unwrap();
            let recovered: RunFlagReason = serde_json::from_str(&json).unwrap();
            assert_eq!(recovered, reason);
        }

        // Verify the auto-generated variant's spelling — the only multi-word
        // reason where a typo would silently break a future feature.
        assert_eq!(
            serde_json::to_string(&RunFlagReason::RecordRequiresPhotoVerification).unwrap(),
            "\"record_requires_photo_verification\"",
        );
    }
}

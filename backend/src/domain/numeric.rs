//! Validated numeric newtypes for run times.
//!
//! Companion to [`super::strings`]. Where `strings.rs` makes
//! "unvalidated-string-in-this-arg" a compile error, this module makes
//! "out-of-range-or-zero-time-in-this-arg" a compile error. Each newtype
//! wraps an `i32` and runs bound validation in its constructor, so a
//! downstream caller holding a [`RaceTimeMs`] or [`LapTimeMs`] can trust
//! the value is in the inclusive range `1..=600_000` (1 ms to ten
//! minutes) without re-checking. See `coding-standards/rust.md` § 2.
//!
//! The `i32` inner type matches the DB column type (`runs.track_time`
//! and the three `runs.lap*_time` columns are `INTEGER`) and the
//! entity-stays-primitive boundary rule in
//! `coding-standards/seaorm.md` § 6. Conversion happens in the service
//! layer at the boundary — input parsing into typed values via the
//! `TryFrom<i32>` impl, output unwrap to `i32` via the `AsRef<i32>` impl
//! at the entity write.
//!
//! The lap-sum invariant ("the three lap times must add exactly to the
//! total race time") is captured by [`assert_lap_sum`], which takes the
//! typed values and is the single place that contract is enforced.

use nutype::nutype;

use crate::error::Error;

/// Inclusive lower bound (in milliseconds) on a single race time or a
/// single lap time: 1 ms.
///
/// Zero and negative values are not legal race times — submitting `0`
/// for a track-time field would imply the run took no time, which the
/// game can't produce.
pub const MIN_TIME_MS: i32 = 1;

/// Inclusive upper bound (in milliseconds) on a single race time or a
/// single lap time: 600,000 ms = ten minutes.
///
/// Public so route DTOs and error messages can spell the limit out the
/// same way the constructor enforces it. The matching lower bound is
/// [`MIN_TIME_MS`].
pub const MAX_TIME_MS: i32 = 600_000;

/// Total elapsed time for one full race, in milliseconds.
///
/// Constructed via `TryFrom<i32>`: values outside the inclusive range
/// from [`MIN_TIME_MS`] up to [`MAX_TIME_MS`] (i.e. zero, negative, or
/// longer than ten minutes) are rejected by the nutype `validate`
/// machinery.
#[nutype(
    validate(greater_or_equal = MIN_TIME_MS, less_or_equal = MAX_TIME_MS),
    derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Display,
        AsRef,
        Serialize,
        Deserialize,
        TryFrom,
    )
)]
pub struct RaceTimeMs(i32);

/// Time for a single lap, in milliseconds.
///
/// Same bounds as [`RaceTimeMs`]: a single lap can in principle be the
/// entire race (e.g. the player took the full ten minutes on lap 1), so
/// the lap upper bound matches the race upper bound. The sum of three
/// `LapTimeMs` values equals the race's `RaceTimeMs` by construction —
/// see [`assert_lap_sum`].
#[nutype(
    validate(greater_or_equal = MIN_TIME_MS, less_or_equal = MAX_TIME_MS),
    derive(
        Debug,
        Clone,
        Copy,
        PartialEq,
        Eq,
        PartialOrd,
        Ord,
        Hash,
        Display,
        AsRef,
        Serialize,
        Deserialize,
        TryFrom,
    )
)]
pub struct LapTimeMs(i32);

/// Verify that three lap times sum exactly to the total race time.
///
/// This is the canonical lap-sum invariant — every run-creation path
/// funnels through this function so the rule is enforced in exactly one
/// place. Working over typed values rather than raw `i32`s means an
/// upstream caller can't accidentally pass garbage: each `LapTimeMs` /
/// `RaceTimeMs` already lives in `1..=600_000`.
///
/// Sum overflow can't happen: three lap times each capped at
/// [`MAX_TIME_MS`] sum to at most `1_800_000`, comfortably inside
/// `i32::MAX`.
///
/// # Errors
///
/// Returns [`Error::BadRequest`] if the laps don't sum to `total`. The
/// message names which direction the sum is off (`over by` when the
/// laps exceed the total, `under by` when they fall short), so the
/// client can show the user whether to add or subtract.
pub(crate) fn assert_lap_sum(laps: [LapTimeMs; 3], total: RaceTimeMs) -> Result<(), Error> {
    let sum: i32 = laps.iter().map(|l| *l.as_ref()).sum();
    let total_inner = *total.as_ref();
    if sum != total_inner {
        let signed_diff = sum - total_inner;
        let direction = if signed_diff > 0 { "over" } else { "under" };
        let magnitude = signed_diff.abs();
        return Err(Error::bad_request(format!(
            "Lap times must add up to total time ({direction} by {magnitude}ms)"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    // ── RaceTimeMs ───────────────────────────────────────────────────

    #[test]
    fn test_race_time_accepts_typical_two_minute_run() {
        // 2:00.000 — middle of the road, well inside bounds.
        assert!(RaceTimeMs::try_from(120_000).is_ok());
    }

    #[test]
    fn test_race_time_accepts_one_ms_lower_bound() {
        // Technically valid; lap-sum check is independent of the bound.
        assert!(RaceTimeMs::try_from(1).is_ok());
    }

    #[test]
    fn test_race_time_accepts_max() {
        assert!(RaceTimeMs::try_from(MAX_TIME_MS).is_ok());
    }

    #[test]
    fn test_race_time_rejects_zero() {
        assert!(RaceTimeMs::try_from(0).is_err());
    }

    #[test]
    fn test_race_time_rejects_negative() {
        assert!(RaceTimeMs::try_from(-1).is_err());
    }

    #[test]
    fn test_race_time_rejects_over_ten_minutes() {
        assert!(RaceTimeMs::try_from(MAX_TIME_MS + 1).is_err());
    }

    // ── LapTimeMs ────────────────────────────────────────────────────

    #[test]
    fn test_lap_time_accepts_typical() {
        assert!(LapTimeMs::try_from(40_000).is_ok());
    }

    #[test]
    fn test_lap_time_rejects_zero() {
        assert!(LapTimeMs::try_from(0).is_err());
    }

    #[test]
    fn test_lap_time_rejects_negative() {
        assert!(LapTimeMs::try_from(-1).is_err());
    }

    #[test]
    fn test_lap_time_rejects_over_max() {
        assert!(LapTimeMs::try_from(MAX_TIME_MS + 1).is_err());
    }

    // ── assert_lap_sum happy path ────────────────────────────────────

    #[test]
    fn test_assert_lap_sum_accepts_matching_laps_and_total() {
        let laps = [
            LapTimeMs::try_from(40_000).unwrap(),
            LapTimeMs::try_from(39_000).unwrap(),
            LapTimeMs::try_from(41_000).unwrap(),
        ];
        let total = RaceTimeMs::try_from(120_000).unwrap();
        assert!(assert_lap_sum(laps, total).is_ok());
    }

    #[test]
    fn test_assert_lap_sum_three_one_ms_laps_match_three_ms_total() {
        // Edge case: the minimum-valued tuple still goes through.
        let laps = [
            LapTimeMs::try_from(1).unwrap(),
            LapTimeMs::try_from(1).unwrap(),
            LapTimeMs::try_from(1).unwrap(),
        ];
        let total = RaceTimeMs::try_from(3).unwrap();
        assert!(assert_lap_sum(laps, total).is_ok());
    }

    // ── assert_lap_sum unhappy path ──────────────────────────────────

    #[test]
    fn test_assert_lap_sum_rejects_undersum_and_carries_signed_diff() {
        // laps sum to 60_000, total is 120_000 → sum < total → "under".
        let laps = [
            LapTimeMs::try_from(20_000).unwrap(),
            LapTimeMs::try_from(20_000).unwrap(),
            LapTimeMs::try_from(20_000).unwrap(),
        ];
        let total = RaceTimeMs::try_from(120_000).unwrap();
        let err = assert_lap_sum(laps, total).unwrap_err();
        match err {
            Error::BadRequest { client, .. } => {
                assert!(client.contains("60000"), "message missing diff: {client}");
                assert!(client.contains("add up to total time"));
                assert!(
                    client.contains("under"),
                    "expected `under` direction in message: {client}",
                );
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_assert_lap_sum_rejects_oversum_and_says_over() {
        // laps sum to 90_000, total is 60_000 → sum > total → "over".
        // Complementary to the `under` test above so a regression that
        // flips the sign on `signed_diff` would fail one of the two.
        let laps = [
            LapTimeMs::try_from(30_000).unwrap(),
            LapTimeMs::try_from(30_000).unwrap(),
            LapTimeMs::try_from(30_000).unwrap(),
        ];
        let total = RaceTimeMs::try_from(60_000).unwrap();
        let err = assert_lap_sum(laps, total).unwrap_err();
        match err {
            Error::BadRequest { client, .. } => {
                assert!(client.contains("30000"), "message missing diff: {client}");
                assert!(
                    client.contains("over"),
                    "expected `over` direction in message: {client}",
                );
                assert!(
                    !client.contains("under"),
                    "should not contain `under` for an oversum: {client}",
                );
            }
            other => panic!("expected BadRequest, got {other:?}"),
        }
    }

    #[test]
    fn test_assert_lap_sum_rejects_off_by_one_ms() {
        // Tight boundary — the lap sum is one ms below the total. This
        // exists specifically so a future refactor can't quietly soften
        // the equality to an `abs(sum - total) <= 1` near-match.
        let laps = [
            LapTimeMs::try_from(40_000).unwrap(),
            LapTimeMs::try_from(39_000).unwrap(),
            LapTimeMs::try_from(40_999).unwrap(),
        ];
        let total = RaceTimeMs::try_from(120_000).unwrap();
        assert!(assert_lap_sum(laps, total).is_err());
    }

    // ── Property: any valid (laps, total) tuple where the laps sum to
    //    the total via construction round-trips through assert_lap_sum.
    proptest! {
        #[test]
        fn test_assert_lap_sum_round_trip_on_valid_construction(
            l1 in 1i32..=200_000,
            l2 in 1i32..=200_000,
            l3 in 1i32..=200_000,
        ) {
            // Pre-condition: the sum must be a valid RaceTimeMs. With
            // each lap capped at 200_000, the max sum is 600_000 — the
            // RaceTimeMs upper bound — so the construction is always
            // accepted.
            let total_value = l1 + l2 + l3;
            let laps = [
                LapTimeMs::try_from(l1).unwrap(),
                LapTimeMs::try_from(l2).unwrap(),
                LapTimeMs::try_from(l3).unwrap(),
            ];
            let total = RaceTimeMs::try_from(total_value).unwrap();
            prop_assert!(assert_lap_sum(laps, total).is_ok());
        }
    }
}

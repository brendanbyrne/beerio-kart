use std::sync::Arc;

use anyhow::Context;

/// Shared application configuration loaded from environment variables.
///
/// Wrapped in `Arc` and stored in Axum's state so all handlers can access it
/// without cloning the inner data on every request.
#[derive(Clone, Debug)]
pub struct Config {
    pub jwt_secret: String,
    /// How long access tokens are valid (minutes). Short-lived because they
    /// can't be revoked — if one leaks, it expires quickly. Stored as `i64`
    /// because `chrono::TimeDelta::minutes` takes `i64`.
    pub jwt_access_expiry_minutes: i64,
    /// How long refresh tokens are valid (days). Longer-lived because they're
    /// stored in an `HttpOnly` cookie (no JavaScript access) and can be revoked
    /// by bumping `refresh_token_version` in the database. Stored as `i64`
    /// because `chrono::TimeDelta::days` takes `i64`.
    pub jwt_refresh_expiry_days: i64,
    pub admin_user_id: Option<String>,
    /// Controls the `Secure` flag on the refresh cookie. Must be `false` for
    /// local `http://localhost` development, `true` in production behind HTTPS.
    pub cookie_secure: bool,

    // Request-level limits applied via Tower middleware. All four are env-tunable
    // so they can be adjusted from Unraid without a redeploy. See `tokio.md` § 12.
    /// Hard ceiling on how long any single request can take, end-to-end. After
    /// this elapses the response becomes 408. Doesn't replace the per-call
    /// timeouts inside service code — defense in depth.
    pub request_timeout_seconds: u64,
    /// Cap on concurrent in-flight requests across the whole router. Requests
    /// past this limit wait for a permit; this is upstream of the handler.
    pub request_concurrency_limit: usize,
    /// Max request body size in bytes. Rejected with 413 before the body is
    /// fully read. Default matches the 10 MiB upload cap from `design.md`.
    pub max_request_body_bytes: usize,
    /// Per-peer-IP rate limit, in requests per minute. Used as both the
    /// sustained rate and the burst capacity (`per_second = max(1, n / 60)`,
    /// `burst_size = n`). Excess requests get 429.
    pub rate_limit_per_minute: u32,
}

impl Config {
    /// Load configuration from environment variables.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `JWT_SECRET` is unset (no signing key means unsigned
    /// or weak tokens), or if `JWT_ACCESS_EXPIRY_MINUTES` / `JWT_REFRESH_EXPIRY_DAYS`
    /// is set to a non-positive value or fails to parse as an `i64`.
    pub fn from_env() -> anyhow::Result<Arc<Self>> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .context("JWT_SECRET must be set — it's the signing key for auth tokens")?;

        // Token expiry fields are i64 to match chrono::TimeDelta's argument
        // type. A zero or negative expiry would mint already-expired tokens,
        // so reject those at load time rather than silently shipping them.
        let jwt_access_expiry_minutes = std::env::var("JWT_ACCESS_EXPIRY_MINUTES")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(15);

        let jwt_refresh_expiry_days = std::env::var("JWT_REFRESH_EXPIRY_DAYS")
            .ok()
            .and_then(|v| v.parse::<i64>().ok())
            .filter(|&v| v > 0)
            .unwrap_or(7);

        let admin_user_id = std::env::var("ADMIN_USER_ID").ok();

        let cookie_secure = std::env::var("COOKIE_SECURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        let request_timeout_seconds = parse_positive_env("REQUEST_TIMEOUT_SECONDS", 30);
        let request_concurrency_limit = parse_positive_env("REQUEST_CONCURRENCY_LIMIT", 100);
        let max_request_body_bytes = parse_positive_env("MAX_REQUEST_BODY_BYTES", 10 * 1024 * 1024);
        let rate_limit_per_minute = parse_positive_env("RATE_LIMIT_PER_MINUTE", 60);

        Ok(Arc::new(Self {
            jwt_secret,
            jwt_access_expiry_minutes,
            jwt_refresh_expiry_days,
            admin_user_id,
            cookie_secure,
            request_timeout_seconds,
            request_concurrency_limit,
            max_request_body_bytes,
            rate_limit_per_minute,
        }))
    }
}

/// Parse a positive integer from the named env var, falling back to `default`
/// on missing, unparseable, or zero values. Used for limits where zero or
/// negative would silently disable the protection.
fn parse_positive_env<T>(name: &str, default: T) -> T
where
    T: std::str::FromStr + PartialOrd + From<u8>,
{
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .filter(|v| *v > T::from(0u8))
        .unwrap_or(default)
}

/// Convert `rate_limit_per_minute` into the millisecond interval between token
/// replenishments expected by `tower_governor::GovernorConfigBuilder`.
///
/// The builder's `per_second` / `per_millisecond` set the *interval between
/// replenishments*, not the rate — see `tower_governor-0.7.0/src/governor.rs:183`:
/// "Set the interval after which one element of the quota is replenished".
/// At sub-second rates that interval needs ms granularity (e.g. 120/min = 500
/// ms between tokens), so this PR uses `per_millisecond` exclusively rather
/// than the `per_second` form, which silently rounds to 1 s for any value
/// `per_minute > 60`.
///
/// Clamp to `max(1)` on both the divisor (zero would div-by-zero) and the
/// result (a sub-millisecond interval is meaningless on real hardware).
#[must_use]
pub fn governor_period_ms(per_minute: u32) -> u64 {
    (60_000_u64 / u64::from(per_minute.max(1))).max(1)
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::default_60_per_min(60, 1000)]
    #[case::tight_10_per_min(10, 6000)]
    #[case::loose_120_per_min(120, 500)]
    #[case::sustained_30_per_min(30, 2000)]
    #[case::zero_clamps(0, 60_000)]
    fn governor_period_ms_maps_per_minute_to_interval(
        #[case] per_minute: u32,
        #[case] expected_ms: u64,
    ) {
        assert_eq!(governor_period_ms(per_minute), expected_ms);
    }
}

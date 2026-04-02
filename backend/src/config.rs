use std::sync::Arc;

/// Shared application configuration loaded from environment variables.
///
/// Wrapped in `Arc` and stored in Axum's state so all handlers can access it
/// without cloning the inner data on every request.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub jwt_secret: String,
    /// How long access tokens are valid (minutes). Short-lived because they
    /// can't be revoked — if one leaks, it expires quickly.
    pub jwt_access_expiry_minutes: u64,
    /// How long refresh tokens are valid (days). Longer-lived because they're
    /// stored in an HttpOnly cookie (no JavaScript access) and can be revoked
    /// by bumping `refresh_token_version` in the database.
    pub jwt_refresh_expiry_days: u64,
    pub admin_user_id: Option<String>,
    /// Controls the `Secure` flag on the refresh cookie. Must be `false` for
    /// local `http://localhost` development, `true` in production behind HTTPS.
    pub cookie_secure: bool,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// Panics if `JWT_SECRET` is not set — this is intentional. Running without
    /// a signing key would silently produce unsigned or weak tokens.
    pub fn from_env() -> Arc<Self> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set — it's the signing key for auth tokens");

        let jwt_access_expiry_minutes = std::env::var("JWT_ACCESS_EXPIRY_MINUTES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(15);

        let jwt_refresh_expiry_days = std::env::var("JWT_REFRESH_EXPIRY_DAYS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(7);

        let admin_user_id = std::env::var("ADMIN_USER_ID").ok();

        let cookie_secure = std::env::var("COOKIE_SECURE")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(true);

        Arc::new(Self {
            jwt_secret,
            jwt_access_expiry_minutes,
            jwt_refresh_expiry_days,
            admin_user_id,
            cookie_secure,
        })
    }
}

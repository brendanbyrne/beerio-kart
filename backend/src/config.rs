use std::sync::Arc;

/// Shared application configuration loaded from environment variables.
///
/// Wrapped in `Arc` and stored in Axum's state so all handlers can access it
/// without cloning the inner data on every request.
#[derive(Clone, Debug)]
pub struct AppConfig {
    pub jwt_secret: String,
    pub jwt_expiry_hours: u64,
    pub admin_user_id: Option<String>,
}

impl AppConfig {
    /// Load configuration from environment variables.
    ///
    /// Panics if `JWT_SECRET` is not set — this is intentional. Running without
    /// a signing key would silently produce unsigned or weak tokens.
    pub fn from_env() -> Arc<Self> {
        let jwt_secret = std::env::var("JWT_SECRET")
            .expect("JWT_SECRET must be set — it's the signing key for auth tokens");

        let jwt_expiry_hours = std::env::var("JWT_EXPIRY_HOURS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(24);

        let admin_user_id = std::env::var("ADMIN_USER_ID").ok();

        Arc::new(Self {
            jwt_secret,
            jwt_expiry_hours,
            admin_user_id,
        })
    }
}

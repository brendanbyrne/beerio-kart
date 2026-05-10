use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{TimeDelta, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{config::AppConfig, error::AppError};

/// Claims for short-lived access tokens (sent in response body, stored in JS memory).
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessClaims {
    /// User ID (UUID string)
    pub sub: String,
    /// Username — included so the frontend can display it without an extra API call
    pub username: String,
    /// Expiry as a Unix timestamp (seconds)
    pub exp: i64,
    /// Issued-at as a Unix timestamp
    pub iat: i64,
    /// Must be "access" — prevents a refresh token from being used as an access token
    pub token_type: String,
}

/// Claims for long-lived refresh tokens (stored in an HttpOnly cookie).
/// Contains `refresh_token_version` which must match the DB value — this is
/// how we revoke all refresh tokens for a user (e.g., on logout or password change).
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// User ID (UUID string)
    pub sub: String,
    /// Must match the user's `refresh_token_version` in the database.
    /// Bumping this value in the DB invalidates all existing refresh tokens.
    pub refresh_token_version: i32,
    /// Expiry as a Unix timestamp (seconds)
    pub exp: i64,
    /// Issued-at as a Unix timestamp
    pub iat: i64,
    /// Must be "refresh"
    pub token_type: String,
}

/// Hash a plaintext password using Argon2id (the recommended variant).
///
/// Returns the PHC-format hash string that includes the salt, algorithm
/// parameters, and hash — everything needed to verify later.
///
/// Argon2 is deliberately CPU/memory-hard (50–200 ms per hash). The work
/// runs on Tokio's blocking pool via `spawn_blocking` so it never stalls an
/// async worker, and `limiter` caps concurrent hashes (see
/// `coding-standards/tokio.md` § 2 and § 12).
pub async fn hash_password(limiter: &Semaphore, password: String) -> Result<String, AppError> {
    let _permit = limiter.acquire().await.map_err(|e| {
        AppError::Internal(anyhow::Error::new(e).context("argon2 semaphore closed"))
    })?;
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut rand_core::OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(AppError::from)
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::Error::new(e).context("argon2 hash task panicked")))?
}

/// Verify a plaintext password against a stored Argon2id hash.
///
/// Like `hash_password`, this offloads the verify to the blocking pool and
/// is gated by the shared semaphore.
pub async fn verify_password(
    limiter: &Semaphore,
    password: String,
    hash: String,
) -> Result<bool, AppError> {
    let _permit = limiter.acquire().await.map_err(|e| {
        AppError::Internal(anyhow::Error::new(e).context("argon2 semaphore closed"))
    })?;
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(AppError::from)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| AppError::Internal(anyhow::Error::new(e).context("argon2 verify task panicked")))?
}

/// Create a short-lived access token for the given user.
pub fn create_access_token(
    user_id: &str,
    username: &str,
    config: &AppConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expiry = now + TimeDelta::minutes(config.jwt_access_expiry_minutes as i64);

    let claims = AccessClaims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp: expiry.timestamp(),
        iat: now.timestamp(),
        token_type: "access".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

/// Create a long-lived refresh token for the given user.
pub fn create_refresh_token(
    user_id: &str,
    refresh_token_version: i32,
    config: &AppConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expiry = now + TimeDelta::days(config.jwt_refresh_expiry_days as i64);

    let claims = RefreshClaims {
        sub: user_id.to_string(),
        refresh_token_version,
        exp: expiry.timestamp(),
        iat: now.timestamp(),
        token_type: "refresh".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

/// Validate an access token and return its claims.
pub fn validate_access_token(
    token: &str,
    config: &AppConfig,
) -> Result<AccessClaims, jsonwebtoken::errors::Error> {
    let token_data = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )?;
    Ok(token_data.claims)
}

/// Validate a refresh token and return its claims.
pub fn validate_refresh_token(
    token: &str,
    config: &AppConfig,
) -> Result<RefreshClaims, jsonwebtoken::errors::Error> {
    let token_data = decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )?;
    Ok(token_data.claims)
}

/// Build the `Set-Cookie` header value for a refresh token.
pub fn refresh_cookie(token: &str, max_age_seconds: i64, config: &AppConfig) -> String {
    let secure = if config.cookie_secure { "Secure; " } else { "" };
    format!(
        "refresh_token={token}; HttpOnly; {secure}SameSite=Lax; Path=/api/v1/auth/refresh; Max-Age={max_age_seconds}"
    )
}

/// Build a `Set-Cookie` header value that clears the refresh token cookie.
pub fn clear_refresh_cookie(config: &AppConfig) -> String {
    refresh_cookie("", 0, config)
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        },
        time::{Duration, Instant},
    };

    use super::*;
    use crate::ARGON2_MAX_CONCURRENT;

    fn test_config() -> Arc<AppConfig> {
        Arc::new(AppConfig {
            jwt_secret: "test-secret-key-for-unit-tests".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
        })
    }

    fn test_limiter() -> Arc<Semaphore> {
        Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT))
    }

    #[tokio::test]
    async fn test_hash_password_produces_argon2id_hash() {
        let limiter = test_limiter();
        let hash = hash_password(&limiter, "mysecretpassword".to_string())
            .await
            .unwrap();
        assert!(
            hash.starts_with("$argon2id$"),
            "Expected argon2id hash, got: {hash}"
        );
    }

    #[tokio::test]
    async fn test_hash_password_produces_different_hashes_for_same_input() {
        let limiter = test_limiter();
        let hash1 = hash_password(&limiter, "samepassword".to_string())
            .await
            .unwrap();
        let hash2 = hash_password(&limiter, "samepassword".to_string())
            .await
            .unwrap();
        assert_ne!(hash1, hash2);
    }

    #[tokio::test]
    async fn test_verify_password_correct() {
        let limiter = test_limiter();
        let hash = hash_password(&limiter, "correctpassword".to_string())
            .await
            .unwrap();
        assert!(
            verify_password(&limiter, "correctpassword".to_string(), hash)
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn test_verify_password_wrong() {
        let limiter = test_limiter();
        let hash = hash_password(&limiter, "correctpassword".to_string())
            .await
            .unwrap();
        assert!(
            !verify_password(&limiter, "wrongpassword".to_string(), hash)
                .await
                .unwrap()
        );
    }

    /// Confirm the semaphore actually serializes concurrent hashes beyond
    /// its capacity. Two assertions:
    ///
    /// 1. A polling observer must see the limiter saturated at some point.
    ///    Without the semaphore wired up, `available_permits()` would never
    ///    drop, so this catches accidental removal of the limiter call.
    /// 2. With PERMITS=2 and TASKS=6, the slowest task waits through two
    ///    earlier "waves" before its turn — completion time must spread out
    ///    by a multiple of the per-hash latency. Without bounded
    ///    concurrency, all six would run on the blocking pool and finish at
    ///    near-identical times.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn test_argon2_limiter_caps_concurrent_hashes() {
        const PERMITS: usize = 2;
        const TASKS: usize = 6;

        let limiter = Arc::new(Semaphore::new(PERMITS));
        let max_in_flight = Arc::new(AtomicUsize::new(0));

        // Observer polls the semaphore at ~1 ms cadence and records the
        // peak in-flight count. Aborts when the test finishes.
        let observer_handle = {
            let limiter = limiter.clone();
            let max_in_flight = max_in_flight.clone();
            tokio::spawn(async move {
                loop {
                    let in_flight = PERMITS - limiter.available_permits();
                    let prev = max_in_flight.load(Ordering::Relaxed);
                    if in_flight > prev {
                        max_in_flight.store(in_flight, Ordering::Relaxed);
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
        };

        let start = Instant::now();
        let mut handles = Vec::with_capacity(TASKS);
        for _ in 0..TASKS {
            let limiter = limiter.clone();
            handles.push(tokio::spawn(async move {
                hash_password(&limiter, "password123".to_string())
                    .await
                    .unwrap();
                start.elapsed()
            }));
        }

        let mut elapsed = Vec::with_capacity(TASKS);
        for h in handles {
            elapsed.push(h.await.unwrap());
        }
        observer_handle.abort();
        elapsed.sort();

        let observed = max_in_flight.load(Ordering::Relaxed);
        let fastest = elapsed[0];
        let slowest = *elapsed.last().unwrap();

        assert_eq!(
            observed, PERMITS,
            "expected limiter to saturate at PERMITS={PERMITS} during the \
             run (max observed = {observed})"
        );
        assert!(
            slowest >= fastest * 2,
            "slowest {slowest:?} expected ≥ 2× fastest {fastest:?} \
             (PERMITS={PERMITS}, TASKS={TASKS}; without the limiter, all \
             tasks would finish at roughly the same time)"
        );
    }

    #[test]
    fn test_create_and_validate_access_token() {
        let config = test_config();
        let token = create_access_token("user-123", "testuser", &config).unwrap();
        let claims = validate_access_token(&token, &config).unwrap();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.token_type, "access");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_create_and_validate_refresh_token() {
        let config = test_config();
        let token = create_refresh_token("user-123", 0, &config).unwrap();
        let claims = validate_refresh_token(&token, &config).unwrap();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.refresh_token_version, 0);
        assert_eq!(claims.token_type, "refresh");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_validate_access_token_with_wrong_secret() {
        let config = test_config();
        let token = create_access_token("user-123", "testuser", &config).unwrap();

        let wrong_config = Arc::new(AppConfig {
            jwt_secret: "wrong-secret".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
        });

        assert!(validate_access_token(&token, &wrong_config).is_err());
    }

    #[test]
    fn test_validate_token_garbage_input() {
        let config = test_config();
        assert!(validate_access_token("not.a.valid.jwt", &config).is_err());
    }

    #[test]
    fn test_access_token_contains_correct_expiry_window() {
        let config = Arc::new(AppConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_access_expiry_minutes: 30,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
        });
        let token = create_access_token("user-1", "alice", &config).unwrap();
        let claims = validate_access_token(&token, &config).unwrap();

        let duration_secs = claims.exp - claims.iat;
        // Should be 30 minutes = 1800 seconds (allow 5s tolerance)
        assert!((duration_secs - 1800).unsigned_abs() < 5);
    }

    #[test]
    fn test_refresh_cookie_format() {
        let config = Arc::new(AppConfig {
            jwt_secret: "s".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: true,
        });
        let cookie = refresh_cookie("tok123", 3600, &config);
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("Secure"));
        assert!(cookie.contains("SameSite=Lax"));
        assert!(cookie.contains("Path=/api/v1/auth/refresh"));
        assert!(cookie.contains("Max-Age=3600"));
    }

    #[test]
    fn test_refresh_cookie_no_secure_in_dev() {
        let config = Arc::new(AppConfig {
            jwt_secret: "s".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
        });
        let cookie = refresh_cookie("tok123", 3600, &config);
        assert!(!cookie.contains("Secure"));
    }
}

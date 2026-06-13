use argon2::{
    Argon2,
    // Alias the argon2 PHC parser type so it doesn't collide with our
    // domain newtype `PasswordHash` (which represents a validated stored
    // hash string, not a parsed argon2 value).
    password_hash::{PasswordHash as Argon2Hash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{TimeDelta, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use tokio::sync::Semaphore;

use crate::{
    config::Config,
    domain::{Password, PasswordHash, UserId, Username},
    entities::refresh_tokens,
    error::Error,
    timeout::db_query,
};

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

/// Claims for long-lived refresh tokens (stored in an `HttpOnly` cookie).
///
/// `refresh_token_version` is the global per-user revoke-all primitive (bumped
/// on logout / password change). `jti` + `family_id` add per-token identity for
/// rotation with reuse detection (ADR-0040): each login starts a `family_id`,
/// each refresh mints a successor token with a fresh `jti` in that family, and a
/// `refresh_tokens` row keyed by `jti` records whether the token has been used.
///
/// Deliberately carries **no `username`** — that is what makes a refresh token
/// fail to deserialize as `AccessClaims`, so a refresh cookie can never
/// authenticate as an access token (see `middleware::auth`).
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshClaims {
    /// User ID (UUID string)
    pub sub: String,
    /// Unique token id (UUID string) — the primary key of the token's
    /// `refresh_tokens` row. Distinct on every minted token, so two tokens are
    /// never byte-identical.
    pub jti: String,
    /// Identifies the chain of tokens descended from one login. Stable across a
    /// family's rotations; reuse detection revokes by `family_id`.
    pub family_id: String,
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
///
/// # Errors
///
/// Returns `Error::Hash` if Argon2 rejects the input (extremely unusual —
/// e.g., RNG failure during salt generation); `Error::Internal` if the
/// blocking task panics or the limiter semaphore is closed.
#[tracing::instrument(skip(limiter, password))]
pub async fn hash_password(limiter: &Semaphore, password: Password) -> Result<PasswordHash, Error> {
    let _permit = limiter
        .acquire()
        .await
        .map_err(|e| Error::Internal(anyhow::Error::new(e).context("Argon2 semaphore closed")))?;
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut rand_core::OsRng);
        let plaintext = password.into_inner();
        let hash_string = Argon2::default()
            .hash_password(plaintext.as_bytes(), &salt)
            .map(|h| h.to_string())
            .map_err(Error::from)?;
        // The argon2 crate just produced this string — it's guaranteed to
        // satisfy our PHC prefix check, so the `TryFrom` cannot fail in
        // practice. Map any divergence to Internal rather than swallow it.
        PasswordHash::try_from(hash_string).map_err(|e| {
            Error::Internal(
                anyhow::Error::msg(e.to_string())
                    .context("argon2 produced a hash that doesn't match the PHC prefix"),
            )
        })
    })
    .await
    .map_err(|e| Error::Internal(anyhow::Error::new(e).context("Argon2 hash task panicked")))?
}

/// Verify a plaintext password against a stored Argon2id hash.
///
/// Like `hash_password`, this offloads the verify to the blocking pool and
/// is gated by the shared semaphore.
///
/// # Errors
///
/// Returns `Error::Hash` if `hash` doesn't parse as a valid Argon2 string;
/// `Error::Internal` if the blocking task panics or the semaphore is closed.
/// Note: a wrong password is `Ok(false)`, not an error.
#[tracing::instrument(skip(limiter, password, hash))]
pub async fn verify_password(
    limiter: &Semaphore,
    password: String,
    hash: PasswordHash,
) -> Result<bool, Error> {
    let _permit = limiter
        .acquire()
        .await
        .map_err(|e| Error::Internal(anyhow::Error::new(e).context("Argon2 semaphore closed")))?;
    tokio::task::spawn_blocking(move || {
        // PHC parse runs inside the blocking pool: this is the strict check
        // (full structure, base64 fields, params) that complements our
        // newtype's prefix-only constructor invariant.
        let parsed = Argon2Hash::new(hash.as_ref()).map_err(Error::from)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|e| Error::Internal(anyhow::Error::new(e).context("Argon2 verify task panicked")))?
}

/// Create a short-lived access token for the given user.
///
/// Takes `&Username` so callers feed a value that has already passed the
/// boundary check (either freshly built from a request or recovered from
/// the DB via [`Username::from_db`]). The claim still serializes as a
/// bare string thanks to `Username`'s transparent serde, so the JWT
/// payload is unchanged.
///
/// # Errors
///
/// Returns `jsonwebtoken::errors::Error` if HMAC signing fails — in practice
/// only possible if `config.jwt_secret` is empty or otherwise unusable.
pub fn create_access_token(
    user_id: &UserId,
    username: &Username,
    config: &Config,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expiry = now + TimeDelta::minutes(config.jwt_access_expiry_minutes);

    let claims = AccessClaims {
        sub: user_id.into(),
        username: username.as_ref().to_string(),
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

/// Create a long-lived refresh token for the given user, in the given family
/// and with the given `jti`.
///
/// Pure JWT minting — the caller is responsible for the matching `refresh_tokens`
/// row (insert on issue, lookup on refresh). `family_id` is fresh on
/// register/login (a new family) and reused on rotation (the successor stays in
/// the family); `jti` is unique on every call.
///
/// # Errors
///
/// Returns `jsonwebtoken::errors::Error` if HMAC signing fails — in practice
/// only possible if `config.jwt_secret` is empty or otherwise unusable.
pub fn create_refresh_token(
    user_id: &UserId,
    refresh_token_version: i32,
    family_id: &str,
    jti: &str,
    config: &Config,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expiry = now + TimeDelta::days(config.jwt_refresh_expiry_days);

    let claims = RefreshClaims {
        sub: user_id.into(),
        jti: jti.to_string(),
        family_id: family_id.to_string(),
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
///
/// # Errors
///
/// Returns `jsonwebtoken::errors::Error` if the JWT signature is invalid,
/// the token is expired or malformed, or the algorithm doesn't match HS256.
pub fn validate_access_token(
    token: &str,
    config: &Config,
) -> Result<AccessClaims, jsonwebtoken::errors::Error> {
    let token_data = decode::<AccessClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )?;
    Ok(token_data.claims)
}

/// Validate a refresh token and return its claims.
///
/// # Errors
///
/// Returns `jsonwebtoken::errors::Error` if the JWT signature is invalid,
/// the token is expired or malformed, or the algorithm doesn't match HS256.
/// Callers should additionally check `claims.token_type == "refresh"`.
pub fn validate_refresh_token(
    token: &str,
    config: &Config,
) -> Result<RefreshClaims, jsonwebtoken::errors::Error> {
    let token_data = decode::<RefreshClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::new(jsonwebtoken::Algorithm::HS256),
    )?;
    Ok(token_data.claims)
}

/// Build the `Set-Cookie` header value for a refresh token.
#[must_use]
pub fn refresh_cookie(token: &str, max_age_seconds: i64, config: &Config) -> String {
    let secure = if config.cookie_secure { "Secure; " } else { "" };
    format!(
        "refresh_token={token}; HttpOnly; {secure}SameSite=Lax; Path=/api/v1/auth/refresh; Max-Age={max_age_seconds}"
    )
}

/// Build a `Set-Cookie` header value that clears the refresh token cookie.
#[must_use]
pub fn clear_refresh_cookie(config: &Config) -> String {
    refresh_cookie("", 0, config)
}

/// Delete expired `refresh_tokens` rows, returning the count deleted.
///
/// Background DB hygiene (ADR-0040 § Row lifecycle), run periodically from
/// `main`. Once a token's JWT `exp` has passed it is rejected at decode before
/// its row is ever consulted, so an expired row carries no remaining
/// reuse-detection value — it's safe to drop.
///
/// # Errors
///
/// Returns [`Error::Timeout`] if the delete exceeds the query budget, or a
/// mapped `DbErr` for any other database failure.
pub async fn prune_refresh_tokens(db: &DatabaseConnection) -> Result<u64, Error> {
    let now = Utc::now().naive_utc();
    let result = db_query(
        refresh_tokens::Entity::delete_many()
            .filter(refresh_tokens::Column::ExpiresAt.lt(now))
            .exec(db),
    )
    .await?;
    Ok(result.rows_affected)
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

    use uuid::Uuid;

    use super::*;
    use crate::ARGON2_MAX_CONCURRENT;

    fn test_config() -> Arc<Config> {
        Arc::new(Config {
            jwt_secret: "test-secret-key-for-unit-tests".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
            refresh_grace_seconds: 10,
            request_timeout_seconds: 30,
            request_concurrency_limit: 100,
            max_request_body_bytes: 10 * 1024 * 1024,
            rate_limit_per_minute: 60,
        })
    }

    fn test_limiter() -> Arc<Semaphore> {
        Arc::new(Semaphore::new(ARGON2_MAX_CONCURRENT))
    }

    fn password(s: &str) -> Password {
        Password::try_from(s.to_string()).expect("test password must be 8-128 chars")
    }

    fn username(s: &str) -> Username {
        Username::try_from(s.to_string()).expect("test username must be 1-30 chars")
    }

    #[tokio::test]
    async fn test_hash_password_produces_argon2id_hash() {
        let limiter = test_limiter();
        let hash = hash_password(&limiter, password("mysecretpassword"))
            .await
            .unwrap();
        assert!(
            hash.as_ref().starts_with("$argon2id$"),
            "Expected argon2id hash, got: {}",
            hash.as_ref()
        );
    }

    #[tokio::test]
    async fn test_hash_password_produces_different_hashes_for_same_input() {
        let limiter = test_limiter();
        let hash1 = hash_password(&limiter, password("samepassword"))
            .await
            .unwrap();
        let hash2 = hash_password(&limiter, password("samepassword"))
            .await
            .unwrap();
        assert_ne!(hash1.as_ref(), hash2.as_ref());
    }

    #[tokio::test]
    async fn test_verify_password_correct() {
        let limiter = test_limiter();
        let hash = hash_password(&limiter, password("correctpassword"))
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
        let hash = hash_password(&limiter, password("correctpassword"))
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
                hash_password(&limiter, password("password123"))
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
        let user_id = UserId::new(Uuid::new_v4());
        let token = create_access_token(&user_id, &username("testuser"), &config).unwrap();
        let claims = validate_access_token(&token, &config).unwrap();

        assert_eq!(claims.sub, user_id.as_ref().to_string());
        assert_eq!(claims.username, "testuser");
        assert_eq!(claims.token_type, "access");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_create_and_validate_refresh_token() {
        let config = test_config();
        let user_id = UserId::new(Uuid::new_v4());
        let family_id = Uuid::new_v4().to_string();
        let jti = Uuid::new_v4().to_string();
        let token = create_refresh_token(&user_id, 0, &family_id, &jti, &config).unwrap();
        let claims = validate_refresh_token(&token, &config).unwrap();

        assert_eq!(claims.sub, user_id.as_ref().to_string());
        assert_eq!(claims.refresh_token_version, 0);
        assert_eq!(claims.token_type, "refresh");
        // `jti` and `family_id` round-trip through the JWT (ADR-0040) — they are
        // what the refresh path looks the token's `refresh_tokens` row up by.
        assert_eq!(claims.jti, jti);
        assert_eq!(claims.family_id, family_id);
        assert!(claims.exp > claims.iat);
    }

    #[tokio::test]
    async fn test_prune_refresh_tokens_deletes_only_expired_rows() {
        use sea_orm::{ActiveModelTrait, ActiveValue::NotSet, EntityTrait, Set};

        use crate::{
            entities::refresh_tokens,
            test_helpers::{create_user, setup_db},
        };

        let db = setup_db().await;
        let user_id = create_user(&db, "pruner").await;
        let now = Utc::now().naive_utc();

        // One already-expired row and one still-live row for the same user.
        let expired = refresh_tokens::ActiveModel {
            id: Set(Uuid::new_v4().to_string()),
            user_id: Set(user_id.to_string()),
            family_id: Set(Uuid::new_v4().to_string()),
            used_at: Set(None),
            expires_at: Set(now - TimeDelta::days(1)),
            created_at: NotSet,
        };
        let live_id = Uuid::new_v4().to_string();
        let live = refresh_tokens::ActiveModel {
            id: Set(live_id.clone()),
            user_id: Set(user_id.to_string()),
            family_id: Set(Uuid::new_v4().to_string()),
            used_at: Set(None),
            expires_at: Set(now + TimeDelta::days(1)),
            created_at: NotSet,
        };
        expired.insert(&db).await.unwrap();
        live.insert(&db).await.unwrap();

        let deleted = prune_refresh_tokens(&db).await.unwrap();
        assert_eq!(deleted, 1, "only the expired row should be pruned");

        // The live row survives; the expired one is gone.
        let remaining = refresh_tokens::Entity::find().all(&db).await.unwrap();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].id, live_id);
    }

    #[test]
    fn test_validate_access_token_with_wrong_secret() {
        let config = test_config();
        let user_id = UserId::new(Uuid::new_v4());
        let token = create_access_token(&user_id, &username("testuser"), &config).unwrap();

        let wrong_config = Arc::new(Config {
            jwt_secret: "wrong-secret".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
            refresh_grace_seconds: 10,
            request_timeout_seconds: 30,
            request_concurrency_limit: 100,
            max_request_body_bytes: 10 * 1024 * 1024,
            rate_limit_per_minute: 60,
        });

        let err = validate_access_token(&token, &wrong_config).unwrap_err();
        // Pin the kind: a token signed with a different secret must fail
        // *signature* verification, not some other parse error (testing.md § 1).
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::InvalidSignature
        ));
    }

    #[test]
    fn test_validate_token_garbage_input() {
        let config = test_config();
        // Single-failure-mode case (testing.md § 1's exception): any malformed
        // token is rejected, and the exact jsonwebtoken ErrorKind here is an
        // artifact of this particular string (a Base64 decode error on the `.`),
        // not a behavioral contract worth pinning — unlike the wrong-secret test
        // above, where InvalidSignature is the stable, name-matching cause.
        validate_access_token("not.a.valid.jwt", &config).unwrap_err();
    }

    #[test]
    fn test_access_token_contains_correct_expiry_window() {
        let config = Arc::new(Config {
            jwt_secret: "test-secret".to_string(),
            jwt_access_expiry_minutes: 30,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
            refresh_grace_seconds: 10,
            request_timeout_seconds: 30,
            request_concurrency_limit: 100,
            max_request_body_bytes: 10 * 1024 * 1024,
            rate_limit_per_minute: 60,
        });
        let user_id = UserId::new(Uuid::new_v4());
        let token = create_access_token(&user_id, &username("alice"), &config).unwrap();
        let claims = validate_access_token(&token, &config).unwrap();

        let duration_secs = claims.exp - claims.iat;
        // Should be 30 minutes = 1800 seconds (allow 5s tolerance)
        assert!((duration_secs - 1800).unsigned_abs() < 5);
    }

    #[test]
    fn test_refresh_cookie_format() {
        let config = Arc::new(Config {
            jwt_secret: "s".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: true,
            refresh_grace_seconds: 10,
            request_timeout_seconds: 30,
            request_concurrency_limit: 100,
            max_request_body_bytes: 10 * 1024 * 1024,
            rate_limit_per_minute: 60,
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
        let config = Arc::new(Config {
            jwt_secret: "s".to_string(),
            jwt_access_expiry_minutes: 15,
            jwt_refresh_expiry_days: 7,
            admin_user_id: None,
            cookie_secure: false,
            refresh_grace_seconds: 10,
            request_timeout_seconds: 30,
            request_concurrency_limit: 100,
            max_request_body_bytes: 10 * 1024 * 1024,
            rate_limit_per_minute: 60,
        });
        let cookie = refresh_cookie("tok123", 3600, &config);
        assert!(!cookie.contains("Secure"));
    }
}

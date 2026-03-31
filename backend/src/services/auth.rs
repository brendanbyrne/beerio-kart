use argon2::{
    Argon2,
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
};
use chrono::{Duration, Utc};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

use crate::config::AppConfig;

/// JWT claims stored inside each token.
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    /// User ID (UUID string)
    pub sub: String,
    /// Username — included so the frontend can display it without an extra API call
    pub username: String,
    /// Expiry as a Unix timestamp (seconds)
    pub exp: i64,
    /// Issued-at as a Unix timestamp
    pub iat: i64,
}

/// Hash a plaintext password using Argon2id (the recommended variant).
///
/// Returns the PHC-format hash string that includes the salt, algorithm
/// parameters, and hash — everything needed to verify later.
pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut rand_core::OsRng);
    let argon2 = Argon2::default(); // Argon2id with default params
    let hash = argon2.hash_password(password.as_bytes(), &salt)?;
    Ok(hash.to_string())
}

/// Verify a plaintext password against a stored Argon2id hash.
pub fn verify_password(password: &str, hash: &str) -> Result<bool, argon2::password_hash::Error> {
    let parsed = PasswordHash::new(hash)?;
    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok())
}

/// Create a signed JWT for the given user.
pub fn create_token(
    user_id: &str,
    username: &str,
    config: &AppConfig,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now();
    let expiry = now + Duration::hours(config.jwt_expiry_hours as i64);

    let claims = Claims {
        sub: user_id.to_string(),
        username: username.to_string(),
        exp: expiry.timestamp(),
        iat: now.timestamp(),
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

/// Validate a JWT and return its claims, or an error if invalid/expired.
pub fn validate_token(
    token: &str,
    config: &AppConfig,
) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn test_config() -> Arc<AppConfig> {
        Arc::new(AppConfig {
            jwt_secret: "test-secret-key-for-unit-tests".to_string(),
            jwt_expiry_hours: 24,
            admin_user_id: None,
        })
    }

    #[test]
    fn test_hash_password_produces_argon2id_hash() {
        let hash = hash_password("mysecretpassword").unwrap();
        // Argon2id hashes start with $argon2id$
        assert!(
            hash.starts_with("$argon2id$"),
            "Expected argon2id hash, got: {hash}"
        );
    }

    #[test]
    fn test_hash_password_produces_different_hashes_for_same_input() {
        let hash1 = hash_password("samepassword").unwrap();
        let hash2 = hash_password("samepassword").unwrap();
        // Different salts should produce different hashes
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_verify_password_correct() {
        let hash = hash_password("correctpassword").unwrap();
        assert!(verify_password("correctpassword", &hash).unwrap());
    }

    #[test]
    fn test_verify_password_wrong() {
        let hash = hash_password("correctpassword").unwrap();
        assert!(!verify_password("wrongpassword", &hash).unwrap());
    }

    #[test]
    fn test_create_and_validate_token() {
        let config = test_config();
        let token = create_token("user-123", "testuser", &config).unwrap();
        let claims = validate_token(&token, &config).unwrap();

        assert_eq!(claims.sub, "user-123");
        assert_eq!(claims.username, "testuser");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_validate_token_with_wrong_secret() {
        let config = test_config();
        let token = create_token("user-123", "testuser", &config).unwrap();

        let wrong_config = Arc::new(AppConfig {
            jwt_secret: "wrong-secret".to_string(),
            jwt_expiry_hours: 24,
            admin_user_id: None,
        });

        assert!(validate_token(&token, &wrong_config).is_err());
    }

    #[test]
    fn test_validate_token_garbage_input() {
        let config = test_config();
        assert!(validate_token("not.a.valid.jwt", &config).is_err());
    }

    #[test]
    fn test_token_contains_correct_expiry_window() {
        let config = Arc::new(AppConfig {
            jwt_secret: "test-secret".to_string(),
            jwt_expiry_hours: 48,
            admin_user_id: None,
        });
        let token = create_token("user-1", "alice", &config).unwrap();
        let claims = validate_token(&token, &config).unwrap();

        let duration_secs = claims.exp - claims.iat;
        // Should be 48 hours = 172800 seconds (allow 5s tolerance for test execution time)
        assert!((duration_secs - 172800).unsigned_abs() < 5);
    }
}

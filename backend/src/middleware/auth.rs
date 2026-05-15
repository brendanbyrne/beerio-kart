use axum::{extract::FromRequestParts, http::request::Parts};

use crate::{domain::UserId, error::Error};

/// Extractor that validates the JWT from the `Authorization: Bearer <token>`
/// header and makes the authenticated user's info available to handlers.
///
/// Only accepts access tokens (`token_type == "access"`). Refresh tokens are
/// valid JWTs signed with the same key, but they must NOT be usable as access
/// tokens — that would let a stolen refresh cookie bypass the short-lived
/// access token window.
///
/// Usage in a handler:
/// ```ignore
/// async fn protected(user: User) -> impl IntoResponse {
///     format!("Hello, {}", user.username)
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct User {
    /// Authenticated user's UUID parsed from the access token's `sub` claim.
    pub user_id: UserId,
    /// Authenticated user's username copied from the token's `username` claim.
    pub username: String,
}

/// Axum extractor implementation. Pulls `Config` from state and validates
/// the bearer token. Returns 401 if the token is missing, malformed, expired,
/// or not an access token.
impl FromRequestParts<crate::AppState> for User {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| Error::token_invalid("Missing Authorization header"))?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| Error::token_invalid("Invalid Authorization header format"))?;

        // Discriminate expired-vs-invalid via the jsonwebtoken error kind so
        // the frontend can react to `token_expired` by refreshing without
        // re-prompting for credentials. Any other failure mode (signature
        // mismatch, malformed JWT, wrong algorithm) is `token_invalid`.
        let claims = crate::services::auth::validate_access_token(token, &state.config).map_err(
            |e| match e.kind() {
                jsonwebtoken::errors::ErrorKind::ExpiredSignature => Error::token_expired(),
                _ => Error::token_invalid("Invalid token"),
            },
        )?;

        // Reject refresh tokens used as access tokens
        if claims.token_type != "access" {
            return Err(Error::token_invalid("Invalid token type"));
        }

        // `sub` is mint-controlled by `create_access_token`, which always
        // serializes a real `UserId`. A non-UUID `sub` here means a forged or
        // foreign-issued token slipped past signature validation — treat it
        // as unauthorized, not internal, so we don't leak corruption details.
        // Goes through the typed `FromStr` surface (nutype-derived) rather
        // than the internal `parse_db_id` helper, which is for entity-column
        // reads where the error context should name the column.
        let user_id: UserId = claims
            .sub
            .parse()
            .map_err(|_| Error::token_invalid("Invalid user id in token"))?;

        Ok(Self {
            user_id,
            username: claims.username,
        })
    }
}

/// Extractor for admin-only routes. First validates the JWT (via `User`),
/// then checks the user ID against the `ADMIN_USER_ID` env var.
///
/// Returns 403 if the user is authenticated but not the admin.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AdminUser {
    /// Authenticated user's UUID, verified to match `Config::admin_user_id`.
    pub user_id: UserId,
    /// Authenticated user's username copied from the token's `username` claim.
    pub username: String,
}

impl FromRequestParts<crate::AppState> for AdminUser {
    type Rejection = Error;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        // First, authenticate normally
        let auth_user = User::from_request_parts(parts, state).await?;

        // Then check admin status
        let admin_id = state
            .config
            .admin_user_id
            .as_ref()
            .ok_or_else(Error::admin_required)?;

        if auth_user.user_id != *admin_id {
            return Err(Error::admin_required());
        }

        Ok(Self {
            user_id: auth_user.user_id,
            username: auth_user.username,
        })
    }
}

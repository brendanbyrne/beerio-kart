use axum::{
    extract::FromRequestParts,
    http::{StatusCode, request::Parts},
};

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
/// async fn protected(user: AuthUser) -> impl IntoResponse {
///     format!("Hello, {}", user.username)
/// }
/// ```
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub username: String,
}

/// Axum extractor implementation. Pulls `AppConfig` from state and validates
/// the bearer token. Returns 401 if the token is missing, malformed, expired,
/// or not an access token.
impl FromRequestParts<crate::AppState> for AuthUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or((StatusCode::UNAUTHORIZED, "Missing Authorization header"))?;

        let token = auth_header.strip_prefix("Bearer ").ok_or((
            StatusCode::UNAUTHORIZED,
            "Invalid Authorization header format",
        ))?;

        let claims = crate::services::auth::validate_access_token(token, &state.config)
            .map_err(|_| (StatusCode::UNAUTHORIZED, "Invalid or expired token"))?;

        // Reject refresh tokens used as access tokens
        if claims.token_type != "access" {
            return Err((StatusCode::UNAUTHORIZED, "Invalid token type"));
        }

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
        })
    }
}

/// Extractor for admin-only routes. First validates the JWT (via `AuthUser`),
/// then checks the user ID against the `ADMIN_USER_ID` env var.
///
/// Returns 403 if the user is authenticated but not the admin.
#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AdminUser {
    pub user_id: String,
    pub username: String,
}

impl FromRequestParts<crate::AppState> for AdminUser {
    type Rejection = (StatusCode, &'static str);

    async fn from_request_parts(
        parts: &mut Parts,
        state: &crate::AppState,
    ) -> Result<Self, Self::Rejection> {
        // First, authenticate normally
        let auth_user = AuthUser::from_request_parts(parts, state).await?;

        // Then check admin status
        let admin_id = state
            .config
            .admin_user_id
            .as_ref()
            .ok_or((StatusCode::FORBIDDEN, "Admin access not configured"))?;

        if auth_user.user_id != *admin_id {
            return Err((StatusCode::FORBIDDEN, "Admin access required"));
        }

        Ok(AdminUser {
            user_id: auth_user.user_id,
            username: auth_user.username,
        })
    }
}

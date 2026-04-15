use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::entities::users;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::auth as auth_service;

// ── Request / Response types ────────────────────────────────────────

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub access_token: String,
    pub user: UserInfo,
}

#[derive(Serialize)]
pub struct RefreshResponse {
    pub access_token: String,
}

#[derive(Serialize)]
pub struct UserInfo {
    pub id: String,
    pub username: String,
}

// ── Helpers ────────────────────────────────────────────────────────

/// Build response headers that set the refresh token cookie.
fn make_refresh_headers(
    refresh_token: &str,
    config: &crate::config::AppConfig,
) -> Result<HeaderMap, AppError> {
    let max_age_seconds = config.jwt_refresh_expiry_days as i64 * 86400;
    let cookie = auth_service::refresh_cookie(refresh_token, max_age_seconds, config);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| AppError::Internal("Failed to build Set-Cookie header".to_string()))?,
    );
    Ok(headers)
}

/// Extract the `refresh_token` value from the Cookie header.
fn extract_refresh_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|pair| {
            let pair = pair.trim();
            pair.strip_prefix("refresh_token=").map(|v| v.to_string())
        })
}

// ── Handlers ────────────────────────────────────────────────────────

/// POST /api/v1/auth/register
///
/// Creates a new user account. Returns an access token in the response body
/// and sets a refresh token as an HttpOnly cookie.
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let username = body.username.trim();
    if username.is_empty() || username.chars().count() > 30 {
        return Err(AppError::BadRequest(
            "Username must be 1-30 characters".into(),
        ));
    }

    if body.password.len() < 8 || body.password.len() > 128 {
        return Err(AppError::BadRequest(
            "Password must be 8-128 characters".into(),
        ));
    }

    // Check if username already exists (nicer error than a DB constraint violation)
    let existing = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&state.db)
        .await?;

    if existing.is_some() {
        return Err(AppError::Conflict("Username already taken".into()));
    }

    // Hash password
    let password_hash = auth_service::hash_password(&body.password)?;

    // Generate user ID and timestamps
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().naive_utc();

    // Insert user
    let new_user = users::ActiveModel {
        id: Set(user_id.clone()),
        username: Set(username.to_string()),
        email: Set(None),
        password_hash: Set(password_hash),
        preferred_character_id: Set(None),
        preferred_body_id: Set(None),
        preferred_wheel_id: Set(None),
        preferred_glider_id: Set(None),
        preferred_drink_type_id: Set(None),
        refresh_token_version: Set(0),
        created_at: Set(now),
        updated_at: Set(now),
    };

    new_user.insert(&state.db).await?;

    // Generate tokens
    let access_token = auth_service::create_access_token(&user_id, username, &state.config)?;
    let refresh_token = auth_service::create_refresh_token(&user_id, 0, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((
        StatusCode::CREATED,
        headers,
        Json(AuthResponse {
            access_token,
            user: UserInfo {
                id: user_id,
                username: username.to_string(),
            },
        }),
    ))
}

/// POST /api/v1/auth/login
///
/// Authenticates with username + password. Returns an access token in the
/// response body and sets a refresh token as an HttpOnly cookie.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    let username = body.username.trim();

    let user = match users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&state.db)
        .await?
    {
        Some(u) => u,
        None => {
            // Hash a dummy password so the timing is similar to the "wrong password"
            // path. Prevents username enumeration via response-time analysis.
            let _ = auth_service::verify_password(
                "dummy",
                "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
            return Err(AppError::Unauthorized(
                "Invalid username or password".into(),
            ));
        }
    };

    // Verify password
    match auth_service::verify_password(&body.password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return Err(AppError::Unauthorized(
                "Invalid username or password".into(),
            ));
        }
        Err(e) => return Err(AppError::from(e)),
    }

    // Generate tokens
    let access_token = auth_service::create_access_token(&user.id, &user.username, &state.config)?;
    let refresh_token =
        auth_service::create_refresh_token(&user.id, user.refresh_token_version, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((
        headers,
        Json(AuthResponse {
            access_token,
            user: UserInfo {
                id: user.id,
                username: user.username,
            },
        }),
    ))
}

/// POST /api/v1/auth/refresh
///
/// Reads the refresh token from the HttpOnly cookie (NOT from the request body
/// or Authorization header). If valid and the `refresh_token_version` matches
/// the DB, returns a new access token and rotates the refresh cookie.
///
/// "Rotation" means issuing a fresh refresh JWT with a fresh expiry on every
/// successful refresh. This extends the session window without bumping the
/// version (which would invalidate other devices).
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let cookie_value = match extract_refresh_cookie(&headers) {
        Some(v) if !v.is_empty() => v,
        _ => return Err(AppError::Unauthorized("Missing refresh token".into())),
    };

    // Validate the JWT signature and expiry
    let claims = auth_service::validate_refresh_token(&cookie_value, &state.config)
        .map_err(|_| AppError::Unauthorized("Invalid or expired refresh token".into()))?;

    // Reject if token_type is not "refresh"
    if claims.token_type != "refresh" {
        return Err(AppError::Unauthorized("Invalid token type".into()));
    }

    // Look up user and check refresh_token_version
    let user = users::Entity::find_by_id(&claims.sub)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Unauthorized("User not found".into()))?;

    // Version mismatch means the token was revoked (logout or password change)
    if claims.refresh_token_version != user.refresh_token_version {
        return Err(AppError::Unauthorized(
            "Refresh token has been revoked".into(),
        ));
    }

    // Issue new tokens
    let access_token = auth_service::create_access_token(&user.id, &user.username, &state.config)?;

    // Rotate: issue a fresh refresh token with same version but new expiry
    let new_refresh =
        auth_service::create_refresh_token(&user.id, user.refresh_token_version, &state.config)?;
    let resp_headers = make_refresh_headers(&new_refresh, &state.config)?;

    Ok((resp_headers, Json(RefreshResponse { access_token })))
}

/// POST /api/v1/auth/logout
///
/// Requires authentication. Increments `refresh_token_version` in the database,
/// which invalidates ALL refresh tokens for this user across all devices.
/// Also clears the refresh cookie on the current browser.
pub async fn logout(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<impl IntoResponse, AppError> {
    // Look up user to get current version
    let db_user = users::Entity::find_by_id(&user.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::Internal("Authenticated user not found in database".into()))?;

    // Increment version to invalidate all existing refresh tokens
    let new_version = db_user.refresh_token_version + 1;
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.refresh_token_version = Set(new_version);
    active.updated_at = Set(chrono::Utc::now().naive_utc());

    active.update(&state.db).await?;

    // Clear the refresh cookie
    let cookie = auth_service::clear_refresh_cookie(&state.config);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie
            .parse()
            .map_err(|_| AppError::Internal("Failed to build Set-Cookie header".to_string()))?,
    );

    Ok((headers, StatusCode::OK))
}

/// PUT /api/v1/auth/password
///
/// Requires authentication. Validates the current password, updates the hash,
/// and increments `refresh_token_version` to force re-login on all other devices.
/// Returns new tokens for the current session so the user stays logged in.
pub async fn change_password(
    State(state): State<AppState>,
    user: AuthUser,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Validate new password length
    if body.new_password.len() < 8 || body.new_password.len() > 128 {
        return Err(AppError::BadRequest(
            "New password must be 8-128 characters".into(),
        ));
    }

    // Look up user
    let db_user = users::Entity::find_by_id(&user.user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound("User not found".into()))?;

    // Verify current password
    match auth_service::verify_password(&body.current_password, &db_user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return Err(AppError::Unauthorized(
                "Current password is incorrect".into(),
            ));
        }
        Err(e) => return Err(AppError::from(e)),
    }

    // Hash new password
    let new_hash = auth_service::hash_password(&body.new_password)?;

    // Update password and bump version (invalidates all other sessions)
    let new_version = db_user.refresh_token_version + 1;
    let username = db_user.username.clone();
    let user_id = db_user.id.clone();
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.password_hash = Set(new_hash);
    active.refresh_token_version = Set(new_version);
    active.updated_at = Set(chrono::Utc::now().naive_utc());

    active.update(&state.db).await?;

    // Issue new tokens for the current session
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    let refresh_token = auth_service::create_refresh_token(&user_id, new_version, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((headers, Json(RefreshResponse { access_token })))
}

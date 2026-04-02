use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::AppState;
use crate::entities::users;
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

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

// ── Helpers ────────────────────────────────────────────────────────

/// Build response headers that set the refresh token cookie.
fn make_refresh_headers(refresh_token: &str, config: &crate::config::AppConfig) -> HeaderMap {
    let max_age_seconds = config.jwt_refresh_expiry_days as i64 * 86400;
    let cookie = auth_service::refresh_cookie(refresh_token, max_age_seconds, config);
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());
    headers
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
) -> impl IntoResponse {
    // Validate input
    let username = body.username.trim();
    if username.is_empty() || username.chars().count() > 30 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "Username must be 1-30 characters".to_string(),
            }),
        )
            .into_response();
    }

    if body.password.len() < 8 || body.password.len() > 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "Password must be 8-128 characters".to_string(),
            }),
        )
            .into_response();
    }

    // Check if username already exists (nicer error than a DB constraint violation)
    let existing = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&state.db)
        .await;

    match existing {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(ErrorBody {
                    error: "Username already taken".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, "Failed to check username availability");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
        Ok(None) => {} // username is available
    }

    // Hash password
    let password_hash = match auth_service::hash_password(&body.password) {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "Failed to hash password");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Generate user ID and timestamps
    let user_id = uuid::Uuid::new_v4().to_string();
    let now = chrono::Utc::now().to_rfc3339();

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
        created_at: Set(now.clone()),
        updated_at: Set(now),
    };

    if let Err(e) = new_user.insert(&state.db).await {
        error!(error = %e, "Failed to insert user");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: "Internal server error".to_string(),
            }),
        )
            .into_response();
    }

    // Generate tokens
    let access_token = match auth_service::create_access_token(&user_id, username, &state.config) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create access token");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    let refresh_token = match auth_service::create_refresh_token(&user_id, 0, &state.config) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create refresh token");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    let headers = make_refresh_headers(&refresh_token, &state.config);

    (
        StatusCode::CREATED,
        headers,
        Json(AuthResponse {
            access_token,
            user: UserInfo {
                id: user_id,
                username: username.to_string(),
            },
        }),
    )
        .into_response()
}

/// POST /api/v1/auth/login
///
/// Authenticates with username + password. Returns an access token in the
/// response body and sets a refresh token as an HttpOnly cookie.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let username = body.username.trim();

    // Use the same error message for "not found" and "wrong password"
    // to avoid leaking whether a username exists.
    let invalid = || {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "Invalid username or password".to_string(),
            }),
        )
    };

    let user = match users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&state.db)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Hash a dummy password so the timing is similar to the "wrong password"
            // path. Prevents username enumeration via response-time analysis.
            let _ = auth_service::verify_password(
                "dummy",
                "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
            return invalid().into_response();
        }
        Err(e) => {
            error!(error = %e, "Failed to look up user during login");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Verify password
    match auth_service::verify_password(&body.password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => return invalid().into_response(),
        Err(e) => {
            error!(error = %e, "Password verification failed unexpectedly");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Generate tokens
    let access_token =
        match auth_service::create_access_token(&user.id, &user.username, &state.config) {
            Ok(t) => t,
            Err(e) => {
                error!(error = %e, "Failed to create access token during login");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody {
                        error: "Internal server error".to_string(),
                    }),
                )
                    .into_response();
            }
        };

    let refresh_token = match auth_service::create_refresh_token(
        &user.id,
        user.refresh_token_version,
        &state.config,
    ) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create refresh token during login");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    let headers = make_refresh_headers(&refresh_token, &state.config);

    (
        headers,
        Json(AuthResponse {
            access_token,
            user: UserInfo {
                id: user.id,
                username: user.username,
            },
        }),
    )
        .into_response()
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
pub async fn refresh(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    let cookie_value = match extract_refresh_cookie(&headers) {
        Some(v) if !v.is_empty() => v,
        _ => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorBody {
                    error: "Missing refresh token".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Validate the JWT signature and expiry
    let claims = match auth_service::validate_refresh_token(&cookie_value, &state.config) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorBody {
                    error: "Invalid or expired refresh token".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Reject if token_type is not "refresh"
    if claims.token_type != "refresh" {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "Invalid token type".to_string(),
            }),
        )
            .into_response();
    }

    // Look up user and check refresh_token_version
    let user = match users::Entity::find_by_id(&claims.sub).one(&state.db).await {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorBody {
                    error: "User not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, "Failed to look up user during refresh");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Version mismatch means the token was revoked (logout or password change)
    if claims.refresh_token_version != user.refresh_token_version {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorBody {
                error: "Refresh token has been revoked".to_string(),
            }),
        )
            .into_response();
    }

    // Issue new tokens
    let access_token =
        match auth_service::create_access_token(&user.id, &user.username, &state.config) {
            Ok(t) => t,
            Err(e) => {
                error!(error = %e, "Failed to create access token during refresh");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody {
                        error: "Internal server error".to_string(),
                    }),
                )
                    .into_response();
            }
        };

    // Rotate: issue a fresh refresh token with same version but new expiry
    let new_refresh = match auth_service::create_refresh_token(
        &user.id,
        user.refresh_token_version,
        &state.config,
    ) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to rotate refresh token");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    let resp_headers = make_refresh_headers(&new_refresh, &state.config);

    (resp_headers, Json(RefreshResponse { access_token })).into_response()
}

/// POST /api/v1/auth/logout
///
/// Requires authentication. Increments `refresh_token_version` in the database,
/// which invalidates ALL refresh tokens for this user across all devices.
/// Also clears the refresh cookie on the current browser.
pub async fn logout(State(state): State<AppState>, user: AuthUser) -> impl IntoResponse {
    // Look up user to get current version
    let db_user = match users::Entity::find_by_id(&user.user_id)
        .one(&state.db)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) | Err(_) => {
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    // Increment version to invalidate all existing refresh tokens
    let new_version = db_user.refresh_token_version + 1;
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.refresh_token_version = Set(new_version);
    active.updated_at = Set(chrono::Utc::now().to_rfc3339());

    if let Err(e) = active.update(&state.db).await {
        error!(error = %e, "Failed to increment refresh_token_version");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }

    // Clear the refresh cookie
    let cookie = auth_service::clear_refresh_cookie(&state.config);
    let mut headers = HeaderMap::new();
    headers.insert(header::SET_COOKIE, cookie.parse().unwrap());

    (headers, StatusCode::OK).into_response()
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
) -> impl IntoResponse {
    // Validate new password length
    if body.new_password.len() < 8 || body.new_password.len() > 128 {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorBody {
                error: "New password must be 8-128 characters".to_string(),
            }),
        )
            .into_response();
    }

    // Look up user
    let db_user = match users::Entity::find_by_id(&user.user_id)
        .one(&state.db)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorBody {
                    error: "User not found".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, "Failed to look up user for password change");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Verify current password
    match auth_service::verify_password(&body.current_password, &db_user.password_hash) {
        Ok(true) => {}
        Ok(false) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(ErrorBody {
                    error: "Current password is incorrect".to_string(),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!(error = %e, "Password verification failed during change");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    }

    // Hash new password
    let new_hash = match auth_service::hash_password(&body.new_password) {
        Ok(h) => h,
        Err(e) => {
            error!(error = %e, "Failed to hash new password");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Update password and bump version (invalidates all other sessions)
    let new_version = db_user.refresh_token_version + 1;
    let username = db_user.username.clone();
    let user_id = db_user.id.clone();
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.password_hash = Set(new_hash);
    active.refresh_token_version = Set(new_version);
    active.updated_at = Set(chrono::Utc::now().to_rfc3339());

    if let Err(e) = active.update(&state.db).await {
        error!(error = %e, "Failed to update password");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorBody {
                error: "Internal server error".to_string(),
            }),
        )
            .into_response();
    }

    // Issue new tokens for the current session
    let access_token = match auth_service::create_access_token(&user_id, &username, &state.config) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create access token after password change");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    let refresh_token =
        match auth_service::create_refresh_token(&user_id, new_version, &state.config) {
            Ok(t) => t,
            Err(e) => {
                error!(error = %e, "Failed to create refresh token after password change");
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorBody {
                        error: "Internal server error".to_string(),
                    }),
                )
                    .into_response();
            }
        };

    let headers = make_refresh_headers(&refresh_token, &state.config);

    (headers, Json(RefreshResponse { access_token })).into_response()
}

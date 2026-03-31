use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::AppState;
use crate::entities::users;

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

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
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

// ── Handlers ────────────────────────────────────────────────────────

/// POST /api/v1/auth/register
///
/// Creates a new user account and returns a JWT.
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
    let password_hash = match crate::services::auth::hash_password(&body.password) {
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
        preferred_wheels_id: Set(None),
        preferred_glider_id: Set(None),
        preferred_drink_type_id: Set(None),
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

    // Generate JWT
    let token = match crate::services::auth::create_token(&user_id, username, &state.config) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create JWT during registration");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(AuthResponse {
            token,
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
/// Authenticates with username + password and returns a JWT.
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> impl IntoResponse {
    let username = body.username.trim();

    // Look up user — use the same error message for "not found" and "wrong password"
    // to avoid leaking whether a username exists.
    let invalid = (
        StatusCode::UNAUTHORIZED,
        Json(ErrorBody {
            error: "Invalid username or password".to_string(),
        }),
    );

    let user = match users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(&state.db)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            // Hash a dummy password so the timing is similar to the "wrong password"
            // path. Prevents username enumeration via response-time analysis.
            let _ = crate::services::auth::verify_password(
                "dummy",
                "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            );
            return invalid.into_response();
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
    match crate::services::auth::verify_password(&body.password, &user.password_hash) {
        Ok(true) => {}
        Ok(false) => return invalid.into_response(),
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

    // Generate JWT
    let token = match crate::services::auth::create_token(&user.id, &user.username, &state.config) {
        Ok(t) => t,
        Err(e) => {
            error!(error = %e, "Failed to create JWT during login");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorBody {
                    error: "Internal server error".to_string(),
                }),
            )
                .into_response();
        }
    };

    Json(AuthResponse {
        token,
        user: UserInfo {
            id: user.id,
            username: user.username,
        },
    })
    .into_response()
}

/// POST /api/v1/auth/logout
///
/// Client-side logout — the frontend discards the token. This endpoint exists
/// so the API surface matches the design doc and the frontend has a consistent
/// endpoint to call. Server-side token revocation can be added later.
pub async fn logout() -> impl IntoResponse {
    StatusCode::OK
}

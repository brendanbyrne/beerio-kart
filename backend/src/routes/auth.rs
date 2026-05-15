use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter,
    Set,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{Password, PasswordHash, UserId, Username},
    entities::users,
    error::Error,
    extract::Json,
    middleware::auth::User,
    services::auth as auth_service,
    timeout::db_query,
};

// ── Request / Response types ────────────────────────────────────────

/// Body shape for `POST /auth/register`.
#[derive(Deserialize)]
pub struct RegisterRequest {
    /// Desired handle. Subject to `Username` validation at the service boundary.
    pub username: String,
    /// Plaintext password. Hashed server-side; never stored.
    pub password: String,
}

/// Body shape for `POST /auth/login`.
#[derive(Deserialize)]
pub struct LoginRequest {
    /// Account handle.
    pub username: String,
    /// Plaintext password. Verified server-side via Argon2.
    pub password: String,
}

/// Body shape for `PUT /auth/password`.
#[derive(Deserialize)]
pub struct ChangePasswordRequest {
    /// Plaintext current password. Verified server-side before the rotation.
    pub current_password: String,
    /// New plaintext password. Hashed server-side; replaces the old hash.
    pub new_password: String,
}

/// Success-response body for `POST /auth/login` and `POST /auth/register`.
#[derive(Serialize)]
pub struct Response {
    /// Short-lived JWT bearer token to send on subsequent requests.
    pub access_token: String,
    /// Public-facing identity bundle.
    pub user: UserInfo,
}

/// Response body for `POST /auth/refresh`.
#[derive(Serialize)]
pub struct RefreshResponse {
    /// Newly-minted short-lived JWT bearer token.
    pub access_token: String,
}

/// Embedded identity bundle on auth responses.
#[derive(Serialize)]
pub struct UserInfo {
    /// User's stable UUID.
    pub id: UserId,
    /// Display handle.
    pub username: Username,
}

// ── Helpers ────────────────────────────────────────────────────────

/// Build response headers that set the refresh token cookie.
fn make_refresh_headers(
    refresh_token: &str,
    config: &crate::config::Config,
) -> Result<HeaderMap, Error> {
    let max_age_seconds = config.jwt_refresh_expiry_days * 86400;
    let cookie = auth_service::refresh_cookie(refresh_token, max_age_seconds, config);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| {
            Error::Internal(anyhow::Error::new(e).context("Failed to build Set-Cookie header"))
        })?,
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
            pair.strip_prefix("refresh_token=").map(ToString::to_string)
        })
}

// ── Handlers ────────────────────────────────────────────────────────

/// POST /api/v1/auth/register
///
/// Creates a new user account. Returns an access token in the response body
/// and sets a refresh token as an `HttpOnly` cookie.
///
/// # Errors
///
/// Returns `BadRequest` if the username is empty / >30 chars or the password
/// is <8 / >128 chars; `Conflict` if the username is taken; `Internal` for
/// password-hash, token-issue, or DB failures.
#[tracing::instrument(
    skip_all,
    fields(username = %body.username, user_id = tracing::field::Empty),
)]
pub async fn register(
    State(state): State<AppState>,
    Json(body): Json<RegisterRequest>,
) -> Result<impl IntoResponse, Error> {
    let username = Username::try_from(body.username)
        .map_err(|_| Error::bad_request("Username must be 1-30 characters"))?;
    let password = Password::try_from(body.password)
        .map_err(|_| Error::bad_request("Password must be 8-128 characters"))?;

    // Check if username already exists (nicer error than a DB constraint violation)
    let existing = db_query(
        users::Entity::find()
            .filter(users::Column::Username.eq(username.as_ref()))
            .one(&state.db),
    )
    .await?;

    if existing.is_some() {
        return Err(Error::username_taken("Username already taken"));
    }

    // Hash password (offloaded to the blocking pool, capped by argon2_limit)
    let password_hash = auth_service::hash_password(&state.argon2_limit, password).await?;

    let user_id = UserId::new_v4();
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

    // Timestamps are populated by `users::ActiveModelBehavior::before_save`.
    let new_user = users::ActiveModel {
        id: Set((&user_id).into()),
        username: Set(username.as_ref().to_string()),
        email: Set(None),
        password_hash: Set(password_hash.into_inner()),
        preferred_character_id: Set(None),
        preferred_body_id: Set(None),
        preferred_wheel_id: Set(None),
        preferred_glider_id: Set(None),
        preferred_drink_type_id: Set(None),
        refresh_token_version: Set(0),
        created_at: NotSet,
        updated_at: NotSet,
    };

    db_query(new_user.insert(&state.db)).await?;

    // Generate tokens
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    let refresh_token = auth_service::create_refresh_token(&user_id, 0, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((
        StatusCode::CREATED,
        headers,
        Json(Response {
            access_token,
            user: UserInfo {
                id: user_id,
                username,
            },
        }),
    ))
}

/// POST /api/v1/auth/login
///
/// Authenticates with username + password. Returns an access token in the
/// response body and sets a refresh token as an `HttpOnly` cookie.
///
/// # Errors
///
/// Returns `Unauthorized` if the username doesn't exist or the password is
/// wrong (both surfaced as the same generic message to prevent username
/// enumeration); `Internal` for password-verify, token-issue, or DB failures.
#[tracing::instrument(
    skip_all,
    fields(username = %body.username, user_id = tracing::field::Empty),
)]
pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<impl IntoResponse, Error> {
    let username = body.username.trim();

    let Some(user) = db_query(
        users::Entity::find()
            .filter(users::Column::Username.eq(username))
            .one(&state.db),
    )
    .await?
    else {
        // Hash a dummy password so the timing is similar to the "wrong password"
        // path. Prevents username enumeration via response-time analysis. The
        // literal starts with `$argon2id$` so the boundary check passes; the
        // `?` here is purely to keep clippy happy about `expect_used` —
        // construction can't actually fail.
        let dummy_hash = PasswordHash::try_from(
            "$argon2id$v=19$m=19456,t=2,p=1$AAAAAAAAAAAAAAAAAAA$AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"
                .to_string(),
        )
        .map_err(|e| {
            Error::Internal(
                anyhow::Error::msg(e.to_string())
                    .context("Dummy timing-equalization hash failed validation"),
            )
        })?;
        let _ = auth_service::verify_password(&state.argon2_limit, "dummy".to_string(), dummy_hash)
            .await;
        return Err(Error::invalid_credentials());
    };
    let user_id = UserId::from_db(&user.id)?;
    tracing::Span::current().record("user_id", tracing::field::display(&user_id));

    // Verify password (offloaded to the blocking pool, capped by argon2_limit).
    // Stored hash is validated at the boundary; a corrupt value surfaces as
    // Internal (data corruption, not a wrong-password 401).
    let stored_hash = PasswordHash::from_db(user.password_hash.clone())?;
    let password_ok =
        auth_service::verify_password(&state.argon2_limit, body.password, stored_hash).await?;
    if !password_ok {
        return Err(Error::invalid_credentials());
    }

    // Stored username is validated at the boundary for the same reason
    // (mirrors the PasswordHash / ImagePath from_db pattern).
    let stored_username = Username::from_db(user.username, "users.username")?;

    // Generate tokens
    let access_token =
        auth_service::create_access_token(&user_id, &stored_username, &state.config)?;
    let refresh_token =
        auth_service::create_refresh_token(&user_id, user.refresh_token_version, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((
        headers,
        Json(Response {
            access_token,
            user: UserInfo {
                id: user_id,
                username: stored_username,
            },
        }),
    ))
}

/// POST /api/v1/auth/refresh
///
/// Reads the refresh token from the `HttpOnly` cookie (NOT from the request body
/// or Authorization header). If valid and the `refresh_token_version` matches
/// the DB, returns a new access token and rotates the refresh cookie.
///
/// "Rotation" means issuing a fresh refresh JWT with a fresh expiry on every
/// successful refresh. This extends the session window without bumping the
/// version (which would invalidate other devices).
///
/// # Errors
///
/// Returns `Unauthorized` if the refresh cookie is missing, the JWT fails
/// validation, the token type is not `refresh`, the user no longer exists,
/// or the token's `refresh_token_version` doesn't match the DB (revoked via
/// logout or password change). `Internal` for token-issue or DB failures.
#[tracing::instrument(skip_all, fields(user_id = tracing::field::Empty))]
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, Error> {
    let cookie_value = match extract_refresh_cookie(&headers) {
        Some(v) if !v.is_empty() => v,
        _ => return Err(Error::token_invalid("Missing refresh token")),
    };

    // Validate the JWT signature and expiry. Discriminate expired-vs-invalid
    // via the jsonwebtoken error kind so the frontend can react to
    // `token_expired` distinctly from `token_invalid` (the expired-refresh
    // path is a re-login prompt; other failures are revocation or tampering).
    let claims =
        auth_service::validate_refresh_token(&cookie_value, &state.config).map_err(|e| match e
            .kind()
        {
            jsonwebtoken::errors::ErrorKind::ExpiredSignature => Error::token_expired(),
            _ => Error::token_invalid("Invalid refresh token"),
        })?;
    // Record `user_id` here — JWT signature has been verified, so `claims.sub`
    // is trustworthy. Earlier failure modes (wrong token type, user not found,
    // version mismatch) all surface with `user_id` populated, which is exactly
    // the diagnostic info you want for "investigate failed refresh for X".
    tracing::Span::current().record("user_id", tracing::field::display(&claims.sub));

    // Reject if token_type is not "refresh"
    if claims.token_type != "refresh" {
        return Err(Error::token_invalid("Invalid token type"));
    }

    // Look up user and check refresh_token_version
    let user = db_query(users::Entity::find_by_id(&claims.sub).one(&state.db))
        .await?
        .ok_or_else(|| Error::token_invalid("User not found"))?;
    let user_id = UserId::from_db(&user.id)?;
    let username = Username::from_db(user.username, "users.username")?;

    // Version mismatch means the token was revoked (logout or password change)
    if claims.refresh_token_version != user.refresh_token_version {
        return Err(Error::token_invalid("Refresh token has been revoked"));
    }

    // Issue new tokens
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;

    // Rotate: issue a fresh refresh token with same version but new expiry
    let new_refresh =
        auth_service::create_refresh_token(&user_id, user.refresh_token_version, &state.config)?;
    let resp_headers = make_refresh_headers(&new_refresh, &state.config)?;

    Ok((resp_headers, Json(RefreshResponse { access_token })))
}

/// POST /api/v1/auth/logout
///
/// Requires authentication. Increments `refresh_token_version` in the database,
/// which invalidates ALL refresh tokens for this user across all devices.
/// Also clears the refresh cookie on the current browser.
///
/// # Errors
///
/// Returns `Internal` if the authenticated user no longer exists in the DB
/// (invariant violation), or for DB / cookie-build failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn logout(State(state): State<AppState>, user: User) -> Result<impl IntoResponse, Error> {
    // Look up user to get current version
    let db_user = db_query(users::Entity::find_by_id(user.user_id).one(&state.db))
        .await?
        .ok_or_else(|| {
            Error::Internal(anyhow::anyhow!("Authenticated user not found in database"))
        })?;

    // Increment version to invalidate all existing refresh tokens.
    // `updated_at` is bumped by `users::ActiveModelBehavior::before_save`.
    let new_version = db_user.refresh_token_version + 1;
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.refresh_token_version = Set(new_version);

    db_query(active.update(&state.db)).await?;

    // Clear the refresh cookie
    let cookie = auth_service::clear_refresh_cookie(&state.config);
    let mut headers = HeaderMap::new();
    headers.insert(
        header::SET_COOKIE,
        cookie.parse().map_err(|e| {
            Error::Internal(anyhow::Error::new(e).context("Failed to build Set-Cookie header"))
        })?,
    );

    Ok((headers, StatusCode::OK))
}

/// PUT /api/v1/auth/password
///
/// Requires authentication. Validates the current password, updates the hash,
/// and increments `refresh_token_version` to force re-login on all other devices.
/// Returns new tokens for the current session so the user stays logged in.
///
/// # Errors
///
/// Returns `BadRequest` if the new password is <8 / >128 chars; `NotFound`
/// if the authenticated user no longer exists; `Unauthorized` if the current
/// password is wrong; `Internal` for password-hash, token-issue, or DB
/// failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn change_password(
    State(state): State<AppState>,
    user: User,
    Json(body): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, Error> {
    // Validate new password length via the newtype boundary.
    let new_password = Password::try_from(body.new_password)
        .map_err(|_| Error::bad_request("New password must be 8-128 characters"))?;

    // Look up user
    let db_user = db_query(users::Entity::find_by_id(user.user_id).one(&state.db))
        .await?
        .ok_or_else(|| Error::NotFound("User not found".into()))?;

    // Verify current password against the stored hash.
    let stored_hash = PasswordHash::from_db(db_user.password_hash.clone())?;
    let current_ok =
        auth_service::verify_password(&state.argon2_limit, body.current_password, stored_hash)
            .await?;
    if !current_ok {
        return Err(Error::invalid_credentials());
    }

    // Hash new password
    let new_hash = auth_service::hash_password(&state.argon2_limit, new_password).await?;

    // Update password and bump version (invalidates all other sessions).
    // `updated_at` is bumped by `users::ActiveModelBehavior::before_save`.
    let new_version = db_user.refresh_token_version + 1;
    let username = Username::from_db(db_user.username.clone(), "users.username")?;
    let user_id = UserId::from_db(&db_user.id)?;
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.password_hash = Set(new_hash.into_inner());
    active.refresh_token_version = Set(new_version);

    db_query(active.update(&state.db)).await?;

    // Issue new tokens for the current session
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    let refresh_token = auth_service::create_refresh_token(&user_id, new_version, &state.config)?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((headers, Json(RefreshResponse { access_token })))
}

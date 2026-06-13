use axum::{
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::IntoResponse,
};
use chrono::{NaiveDateTime, TimeDelta, Utc};
use sea_orm::{
    ActiveModelTrait, ActiveValue::NotSet, ColumnTrait, ConnectionTrait, EntityTrait,
    IntoActiveModel, QueryFilter, QueryOrder, Set, TransactionTrait, sea_query::Expr,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AppState,
    config::Config,
    domain::{Password, PasswordHash, UserId, Username},
    entities::{refresh_tokens, users},
    error::Error,
    extract::Json,
    middleware::auth::User,
    services::auth as auth_service,
    timeout::{db_query, db_txn},
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

/// Mint a refresh token in the given family and persist its `refresh_tokens`
/// row as the live tip (`used_at = NULL`). Generates a fresh `jti`, so the
/// minted token is byte-distinct from every other (ADR-0040). The row's
/// `expires_at` mirrors the JWT expiry so the prune sweep can drop it once the
/// JWT is dead. Generic over the connection so it works on a plain handle
/// (register / login / password-change start a new family) or inside the
/// refresh transaction (a successor in an existing family).
async fn mint_refresh_in_family<C: ConnectionTrait>(
    conn: &C,
    user_id: &UserId,
    refresh_token_version: i32,
    family_id: &str,
    config: &Config,
) -> Result<String, Error> {
    let jti = Uuid::new_v4().to_string();
    let expires_at = (Utc::now() + TimeDelta::days(config.jwt_refresh_expiry_days)).naive_utc();

    // `created_at` is stamped by `refresh_tokens::ActiveModelBehavior`.
    db_query(
        refresh_tokens::ActiveModel {
            id: Set(jti.clone()),
            user_id: Set(user_id.into()),
            family_id: Set(family_id.to_string()),
            used_at: Set(None),
            expires_at: Set(expires_at),
            created_at: NotSet,
        }
        .insert(conn),
    )
    .await?;

    Ok(auth_service::create_refresh_token(
        user_id,
        refresh_token_version,
        family_id,
        &jti,
        config,
    )?)
}

/// Re-mint a JWT for the family's current live tip, without rotating. Used when
/// a refresh loses a concurrent race or arrives within the grace window: the
/// successor already exists, so we hand back a token for it rather than
/// revoking the family on a false-positive reuse. No live tip means the family
/// was revoked — surfaced as `token_invalid`.
///
/// Note: the re-minted JWT gets a fresh `exp` (now + TTL) while the tip row's
/// `expires_at` is unchanged, so a reissued token can briefly outlive its row.
/// Harmless — the row is authoritative (the refresh path rejects on
/// `expires_at < now`), so a token whose row was pruned just surfaces as
/// `token_invalid` instead of `token_expired`. Both are clean 401s.
async fn reissue_from_family_tip<C: ConnectionTrait>(
    conn: &C,
    user_id: &UserId,
    refresh_token_version: i32,
    family_id: &str,
    now: NaiveDateTime,
    config: &Config,
) -> Result<String, Error> {
    let tip = db_query(
        refresh_tokens::Entity::find()
            .filter(refresh_tokens::Column::FamilyId.eq(family_id))
            .filter(refresh_tokens::Column::UsedAt.is_null())
            .filter(refresh_tokens::Column::ExpiresAt.gt(now))
            .order_by_desc(refresh_tokens::Column::CreatedAt)
            .one(conn),
    )
    .await?
    .ok_or_else(|| Error::token_invalid("Refresh token has been revoked"))?;

    Ok(auth_service::create_refresh_token(
        user_id,
        refresh_token_version,
        family_id,
        &tip.id,
        config,
    )?)
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

    // Generate tokens. Registration starts a fresh token family (ADR-0040).
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    let family_id = Uuid::new_v4().to_string();
    let refresh_token =
        mint_refresh_in_family(&state.db, &user_id, 0, &family_id, &state.config).await?;
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

    // Generate tokens. Each login starts a new token family (ADR-0040), so a
    // user's devices each get an independent family that rotates on its own.
    let access_token =
        auth_service::create_access_token(&user_id, &stored_username, &state.config)?;
    let family_id = Uuid::new_v4().to_string();
    let refresh_token = mint_refresh_in_family(
        &state.db,
        &user_id,
        user.refresh_token_version,
        &family_id,
        &state.config,
    )
    .await?;
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
/// or Authorization header), then rotates it with **reuse detection** (ADR-0040):
///
/// 1. Validate the JWT (signature + expiry) and the `refresh_token_version`
///    (the global per-user revoke-all, bumped on logout / password change).
/// 2. Look the token's row up by `jti`. A **live** row (`used_at IS NULL`) is
///    rotated: it's marked used and a successor is minted in the same family.
///    An **already-used** row presented past the grace window is **reuse** —
///    the whole family is revoked and the call fails `token_reuse_detected`.
/// 3. A used row *within* the grace window, or a lost rotation race, reissues
///    the family's live successor instead of revoking — the backstop against
///    spuriously logging out concurrent / retried refreshes.
///
/// # Errors
///
/// Returns `Unauthorized` (`token_invalid` / `token_expired` /
/// `token_reuse_detected`) if the cookie is missing, the JWT fails validation,
/// the token type is wrong, the user is gone, the version doesn't match, the
/// row is missing/expired, or reuse is detected. `Internal` / `Timeout` for
/// token-issue or DB failures.
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

    // Version mismatch means every token was revoked (logout or password change)
    if claims.refresh_token_version != user.refresh_token_version {
        return Err(Error::token_invalid("Refresh token has been revoked"));
    }

    let now = Utc::now().naive_utc();
    let grace = TimeDelta::seconds(state.config.refresh_grace_seconds);

    // ── Token-family rotation + reuse detection (ADR-0040) ──
    //
    // The conditional `used_at` claim below is the atomicity primitive: of N
    // concurrent refreshes presenting the same token, exactly one flips
    // `used_at` (rows_affected == 1) and rotates; the losers match 0 rows and
    // reissue the family's live successor instead of revoking on a false
    // positive.
    //
    // It is deliberately the **first** statement in the transaction. SQLite
    // then begins the txn by taking the write lock (the `BEGIN IMMEDIATE`
    // shape), so a losing concurrent refresh *waits* on `busy_timeout` for the
    // winner to commit and then re-evaluates `WHERE used_at IS NULL` against the
    // committed state, cleanly matching 0 rows. The natural read-first order (a
    // deferred read, then an upgrade to write) would instead take a read
    // snapshot that the winner's commit invalidates, so the loser's write fails
    // *immediately* with a `SQLITE_BUSY`-family error (busy_timeout does not
    // retry a snapshot/upgrade conflict) — which would surface as a spurious
    // 500 and log the user out. Measured: read-first errors on ~98% of losers,
    // write-first on 0%. See ADR-0040.
    let txn = db_txn(state.db.begin()).await?;

    let claim = db_query(
        refresh_tokens::Entity::update_many()
            .col_expr(refresh_tokens::Column::UsedAt, Expr::value(now))
            .filter(refresh_tokens::Column::Id.eq(&claims.jti))
            .filter(refresh_tokens::Column::UsedAt.is_null())
            .exec(&txn),
    )
    .await?;

    // With the write lock held, this read is authoritative: our own flip if we
    // won the claim, or the winner's committed state if we lost it.
    let row = db_query(refresh_tokens::Entity::find_by_id(&claims.jti).one(&txn))
        .await?
        // Missing row = the family was revoked (rows deleted) or an unknown
        // `jti`. Either way the token is no longer usable.
        .ok_or_else(|| Error::token_invalid("Refresh token has been revoked"))?;

    // Expiry guard (defensive — the JWT `exp` check above normally rejects
    // first; a reissued token can briefly outlive its row). Returning here drops
    // the txn, rolling back any `used_at` flip we just made on the dead row.
    if row.expires_at < now {
        return Err(Error::token_expired());
    }

    let new_refresh = if claim.rows_affected >= 1 {
        // Won the claim: mint the successor in the same family.
        mint_refresh_in_family(
            &txn,
            &user_id,
            user.refresh_token_version,
            &row.family_id,
            &state.config,
        )
        .await?
    } else {
        // Didn't flip it, so `used_at` is already set — either a retry / racing
        // refresh (within grace) or genuine reuse (past grace).
        match row.used_at {
            Some(used_at) if now.signed_duration_since(used_at) <= grace => {
                // Within the grace window, or a lost rotation race: a retry or a
                // racing refresh that read the row after it was marked used.
                // Reissue the family's live successor rather than revoke.
                reissue_from_family_tip(
                    &txn,
                    &user_id,
                    user.refresh_token_version,
                    &row.family_id,
                    now,
                    &state.config,
                )
                .await?
            }
            Some(_) => {
                // Genuine reuse: a token rotated away from, replayed past the
                // grace window. Revoke the whole family and force re-auth.
                db_query(
                    refresh_tokens::Entity::delete_many()
                        .filter(refresh_tokens::Column::FamilyId.eq(&row.family_id))
                        .exec(&txn),
                )
                .await?;
                tracing::warn!(
                    user_id = %claims.sub,
                    family_id = %row.family_id,
                    "Refresh token reuse detected; revoking token family"
                );
                db_txn(txn.commit()).await?;
                return Err(Error::token_reuse_detected());
            }
            None => {
                // Unreachable while holding the write lock: a 0-row claim means
                // `used_at` is non-null (an unused, unexpired row would have been
                // claimed above). Defensive — treat as revoked/unknown.
                return Err(Error::token_invalid("Refresh token has been revoked"));
            }
        }
    };

    // Mint the access token before committing so a (vanishingly unlikely) JWT
    // failure rolls back the rotation rather than leaving a used token with no
    // issued successor in the caller's hands.
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    db_txn(txn.commit()).await?;

    let resp_headers = make_refresh_headers(&new_refresh, &state.config)?;
    Ok((resp_headers, Json(RefreshResponse { access_token })))
}

/// POST /api/v1/auth/logout
///
/// Requires authentication. Increments `refresh_token_version` in the database
/// (the global revoke-all) AND deletes this user's `refresh_tokens` rows, so
/// every family across all devices is invalidated. Also clears the refresh
/// cookie on the current browser.
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
    let user_id_str = db_user.id.clone();
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.refresh_token_version = Set(new_version);

    db_query(active.update(&state.db)).await?;

    // Clear this user's token families. The version bump already revokes every
    // refresh token; deleting the rows is the matching state cleanup (ADR-0040)
    // so reuse detection isn't tripped by a stale row after a fresh login.
    db_query(
        refresh_tokens::Entity::delete_many()
            .filter(refresh_tokens::Column::UserId.eq(&user_id_str))
            .exec(&state.db),
    )
    .await?;

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
    let user_id_str = db_user.id.clone();
    let mut active: users::ActiveModel = db_user.into_active_model();
    active.password_hash = Set(new_hash.into_inner());
    active.refresh_token_version = Set(new_version);

    db_query(active.update(&state.db)).await?;

    // Clear every existing family (all other devices are now logged out), then
    // start a fresh family for the current session below — so the new cookie
    // we return isn't immediately deleted (ADR-0040).
    db_query(
        refresh_tokens::Entity::delete_many()
            .filter(refresh_tokens::Column::UserId.eq(&user_id_str))
            .exec(&state.db),
    )
    .await?;

    // Issue new tokens for the current session (a new family at the bumped version).
    let access_token = auth_service::create_access_token(&user_id, &username, &state.config)?;
    let family_id = Uuid::new_v4().to_string();
    let refresh_token =
        mint_refresh_in_family(&state.db, &user_id, new_version, &family_id, &state.config).await?;
    let headers = make_refresh_headers(&refresh_token, &state.config)?;

    Ok((headers, Json(RefreshResponse { access_token })))
}

use axum::extract::State;
use chrono::{DateTime, Utc};
use sea_orm::EntityTrait;
use serde::Serialize;

use crate::{
    AppState,
    domain::{BodyId, CharacterId, DrinkTypeId, GliderId, UserId, Username, WheelId},
    entities::users,
    error::Error,
    extract::{Json, Path},
    middleware::auth::User,
    services::users as user_service,
    timeout::db_query,
};

// ── Response types (API contract) ───────────────────────────────────

/// Public-facing user profile shape — what other users see.
///
/// Email and password hash are deliberately not included.
#[derive(Serialize)]
pub struct UserPublicProfile {
    /// User's stable UUID.
    pub id: UserId,
    /// User's chosen handle. Unique case-insensitively.
    pub username: Username,
    /// User's default character pick, if set.
    pub preferred_character_id: Option<CharacterId>,
    /// User's default kart body, if set.
    pub preferred_body_id: Option<BodyId>,
    /// User's default wheel set, if set.
    pub preferred_wheel_id: Option<WheelId>,
    /// User's default glider, if set.
    pub preferred_glider_id: Option<GliderId>,
    /// User's default drink choice, if set.
    pub preferred_drink_type_id: Option<DrinkTypeId>,
    /// Account-creation timestamp, UTC.
    pub created_at: DateTime<Utc>,
}

// ── Handlers ────────────────────────────────────────────────────────

/// GET /api/v1/users — list users (public profile shape).
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_users(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserPublicProfile>>, Error> {
    let all_users = db_query(users::Entity::find().all(&state.db)).await?;
    Ok(Json(
        all_users
            .into_iter()
            .map(|u| {
                Ok::<_, Error>(UserPublicProfile {
                    id: UserId::from_db(&u.id)?,
                    username: Username::from_db(u.username, "users.username")?,
                    preferred_character_id: u.preferred_character_id.map(CharacterId::new),
                    preferred_body_id: u.preferred_body_id.map(BodyId::new),
                    preferred_wheel_id: u.preferred_wheel_id.map(WheelId::new),
                    preferred_glider_id: u.preferred_glider_id.map(GliderId::new),
                    preferred_drink_type_id: u
                        .preferred_drink_type_id
                        .as_deref()
                        .map(DrinkTypeId::from_db)
                        .transpose()?,
                    created_at: u.created_at.and_utc(),
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

/// GET /api/v1/users/:id — get a user's detail profile.
///
/// # Errors
///
/// Returns `NotFound` if `id` doesn't match a user; propagates the errors
/// of [`user_service::build_detail_profile`] for DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, target_user_id = %id))]
pub async fn get_user(
    user: User,
    State(state): State<AppState>,
    Path(id): Path<UserId>,
) -> Result<Json<user_service::UserDetailProfile>, Error> {
    let user = db_query(users::Entity::find_by_id(id).one(&state.db))
        .await?
        .ok_or_else(|| Error::NotFound(format!("User {id} not found")))?;

    let profile = user_service::build_detail_profile(&state.db, user).await?;
    Ok(Json(profile))
}

/// PATCH /api/v1/users/:id — update a user's profile fields.
///
/// # Errors
///
/// Propagates the errors of [`user_service::update_profile`]: `Forbidden`
/// if a user tries to modify another user, `NotFound` if the target user
/// doesn't exist, `BadRequest` for invalid race-setup or drink-type IDs.
#[tracing::instrument(
    skip_all,
    fields(user_id = %auth_user.user_id, target_user_id = %target_user_id),
)]
pub async fn update_user(
    auth_user: User,
    State(state): State<AppState>,
    Path(target_user_id): Path<UserId>,
    Json(req): Json<user_service::UpdateProfileRequest>,
) -> Result<Json<user_service::UserDetailProfile>, Error> {
    let profile =
        user_service::update_profile(&state.db, &auth_user.user_id, &target_user_id, req).await?;
    Ok(Json(profile))
}

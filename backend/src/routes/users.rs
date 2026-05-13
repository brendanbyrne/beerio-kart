use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use sea_orm::EntityTrait;
use serde::Serialize;

use crate::{
    AppState, domain::UserId, entities::users, error::Error, middleware::auth::User,
    services::users as user_service,
};

// ── Response types (API contract) ───────────────────────────────────

#[derive(Serialize)]
pub struct UserPublicProfile {
    pub id: UserId,
    pub username: String,
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
    pub preferred_drink_type_id: Option<String>,
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
    let all_users = users::Entity::find().all(&state.db).await?;
    Ok(Json(
        all_users
            .into_iter()
            .map(|u| UserPublicProfile {
                id: UserId::new(u.id),
                username: u.username,
                preferred_character_id: u.preferred_character_id,
                preferred_body_id: u.preferred_body_id,
                preferred_wheel_id: u.preferred_wheel_id,
                preferred_glider_id: u.preferred_glider_id,
                preferred_drink_type_id: u.preferred_drink_type_id,
                created_at: u.created_at.and_utc(),
            })
            .collect(),
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
    Path(id): Path<String>,
) -> Result<Json<user_service::UserDetailProfile>, Error> {
    let user = users::Entity::find_by_id(&id)
        .one(&state.db)
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
    fields(user_id = %auth_user.user_id, target_user_id = %id),
)]
pub async fn update_user(
    auth_user: User,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<user_service::UpdateProfileRequest>,
) -> Result<Json<user_service::UserDetailProfile>, Error> {
    let target_user_id = UserId::new(id);
    let profile =
        user_service::update_profile(&state.db, &auth_user.user_id, &target_user_id, req).await?;
    Ok(Json(profile))
}

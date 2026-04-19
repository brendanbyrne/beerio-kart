use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use sea_orm::EntityTrait;
use serde::Serialize;

use crate::AppState;
use crate::entities::users;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::users as user_service;

// ── Response types (API contract) ───────────────────────────────────

#[derive(Serialize)]
pub struct UserPublicProfile {
    pub id: String,
    pub username: String,
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
    pub preferred_drink_type_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

// ── Handlers ────────────────────────────────────────────────────────

pub async fn list_users(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserPublicProfile>>, AppError> {
    let all_users = users::Entity::find().all(&state.db).await?;
    Ok(Json(
        all_users
            .into_iter()
            .map(|u| UserPublicProfile {
                id: u.id,
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

pub async fn get_user(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<user_service::UserDetailProfile>, AppError> {
    let user = users::Entity::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    let profile = user_service::build_detail_profile(&state.db, user).await?;
    Ok(Json(profile))
}

pub async fn update_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<user_service::UpdateProfileRequest>,
) -> Result<Json<user_service::UserDetailProfile>, AppError> {
    let profile = user_service::update_profile(&state.db, &auth_user.user_id, &id, req).await?;
    Ok(Json(profile))
}

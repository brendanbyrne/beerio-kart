use axum::{
    Json,
    extract::{Path, State},
};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::entities::{bodies, characters, drink_types, gliders, users, wheels};
use crate::error::AppError;
use crate::middleware::auth::AuthUser;

// ── Response types ───────────────────────────────────────────────────

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

#[derive(Serialize)]
pub struct UserDetailProfile {
    pub id: String,
    pub username: String,
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
    pub preferred_drink_type: Option<DrinkTypeInfo>,
    pub created_at: DateTime<Utc>,
}

#[derive(Serialize)]
pub struct DrinkTypeInfo {
    pub id: String,
    pub name: String,
    pub alcoholic: bool,
}

// ── Request types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    /// Race setup: all four must be provided together, or all omitted.
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
    /// Drink type: independent of race setup.
    /// - Key absent: don't change
    /// - Key present with null: clear the preference
    /// - Key present with value: set the preference
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub preferred_drink_type_id: Option<Option<String>>,
}

/// Deserializer that distinguishes between "key absent" (None) and
/// "key present with null" (Some(None)). Standard serde collapses both
/// to None for Option<Option<T>>.
fn deserialize_optional_field<'de, T, D>(deserializer: D) -> Result<Option<T>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(Some)
}

// ── Handlers ─────────────────────────────────────────────────────────

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
) -> Result<Json<UserDetailProfile>, AppError> {
    let user = users::Entity::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    let drink_type = if let Some(ref dt_id) = user.preferred_drink_type_id {
        drink_types::Entity::find_by_id(dt_id)
            .one(&state.db)
            .await?
            .map(|dt| DrinkTypeInfo {
                id: dt.id,
                name: dt.name,
                alcoholic: dt.alcoholic,
            })
    } else {
        None
    };

    Ok(Json(UserDetailProfile {
        id: user.id,
        username: user.username,
        preferred_character_id: user.preferred_character_id,
        preferred_body_id: user.preferred_body_id,
        preferred_wheel_id: user.preferred_wheel_id,
        preferred_glider_id: user.preferred_glider_id,
        preferred_drink_type: drink_type,
        created_at: user.created_at.and_utc(),
    }))
}

pub async fn update_user(
    auth_user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateProfileRequest>,
) -> Result<Json<UserDetailProfile>, AppError> {
    // Self-only: users can only update their own profile
    if auth_user.user_id != id {
        return Err(AppError::Forbidden(
            "You can only update your own profile".to_string(),
        ));
    }

    let user = users::Entity::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {id} not found")))?;

    let mut active: users::ActiveModel = user.into();

    // Race setup: all-or-nothing validation
    let race_setup_fields = [
        req.preferred_character_id,
        req.preferred_body_id,
        req.preferred_wheel_id,
        req.preferred_glider_id,
    ];
    let provided_count = race_setup_fields.iter().filter(|f| f.is_some()).count();

    if provided_count > 0 && provided_count < 4 {
        return Err(AppError::BadRequest(
            "Race setup must include all four fields (character, body, wheel, glider) or none"
                .to_string(),
        ));
    }

    if provided_count == 4 {
        let char_id = req.preferred_character_id.expect("validated above");
        let body_id = req.preferred_body_id.expect("validated above");
        let wheel_id = req.preferred_wheel_id.expect("validated above");
        let glider_id = req.preferred_glider_id.expect("validated above");

        // Validate FK references exist
        if characters::Entity::find_by_id(char_id)
            .one(&state.db)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!(
                "Character {char_id} not found"
            )));
        }
        if bodies::Entity::find_by_id(body_id)
            .one(&state.db)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!("Body {body_id} not found")));
        }
        if wheels::Entity::find_by_id(wheel_id)
            .one(&state.db)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!("Wheel {wheel_id} not found")));
        }
        if gliders::Entity::find_by_id(glider_id)
            .one(&state.db)
            .await?
            .is_none()
        {
            return Err(AppError::BadRequest(format!(
                "Glider {glider_id} not found"
            )));
        }

        active.preferred_character_id = Set(Some(char_id));
        active.preferred_body_id = Set(Some(body_id));
        active.preferred_wheel_id = Set(Some(wheel_id));
        active.preferred_glider_id = Set(Some(glider_id));
    }

    // Drink type: independent, can be set or cleared
    if let Some(dt_id_option) = req.preferred_drink_type_id {
        if let Some(ref dt_id) = dt_id_option {
            // Validate FK reference exists
            if drink_types::Entity::find_by_id(dt_id)
                .one(&state.db)
                .await?
                .is_none()
            {
                return Err(AppError::BadRequest(format!(
                    "Drink type {dt_id} not found"
                )));
            }
        }
        active.preferred_drink_type_id = Set(dt_id_option);
    }

    active.updated_at = Set(chrono::Utc::now().naive_utc());
    let updated = active.update(&state.db).await?;

    // Fetch drink type details for response
    let drink_type = if let Some(ref dt_id) = updated.preferred_drink_type_id {
        drink_types::Entity::find_by_id(dt_id)
            .one(&state.db)
            .await?
            .map(|dt| DrinkTypeInfo {
                id: dt.id,
                name: dt.name,
                alcoholic: dt.alcoholic,
            })
    } else {
        None
    };

    Ok(Json(UserDetailProfile {
        id: updated.id,
        username: updated.username,
        preferred_character_id: updated.preferred_character_id,
        preferred_body_id: updated.preferred_body_id,
        preferred_wheel_id: updated.preferred_wheel_id,
        preferred_glider_id: updated.preferred_glider_id,
        preferred_drink_type: drink_type,
        created_at: updated.created_at.and_utc(),
    }))
}

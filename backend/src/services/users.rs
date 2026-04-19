use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};
use serde::{Deserialize, Serialize};

use crate::domain::race_setup::RaceSetupUpdate;
use crate::entities::{bodies, characters, drink_types, gliders, users, wheels};
use crate::error::AppError;
use crate::services::helpers;

// ── Types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DrinkTypeInfo {
    pub id: String,
    pub name: String,
    pub alcoholic: bool,
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
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request body for `PUT /users/:id`.
///
/// `preferred_drink_type_id` uses `Option<Option<String>>` to distinguish
/// three states: key absent (don't change), key present with null (clear),
/// key present with value (set).
#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub preferred_character_id: Option<i32>,
    pub preferred_body_id: Option<i32>,
    pub preferred_wheel_id: Option<i32>,
    pub preferred_glider_id: Option<i32>,
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

// ── Service functions ────────────────────────────────────────────────

/// Update a user's profile (preferred race setup and/or drink type).
/// Only the user themselves can update their own profile.
pub async fn update_profile(
    db: &DatabaseConnection,
    actor_user_id: &str,
    target_user_id: &str,
    req: UpdateProfileRequest,
) -> Result<UserDetailProfile, AppError> {
    if actor_user_id != target_user_id {
        return Err(AppError::Forbidden(
            "You can only update your own profile".to_string(),
        ));
    }

    let user = users::Entity::find_by_id(target_user_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("User {target_user_id} not found")))?;

    let mut active: users::ActiveModel = user.into();

    // Race setup: all-or-nothing via RaceSetupUpdate
    if let Some(setup) = RaceSetupUpdate::try_from_optional(
        req.preferred_character_id,
        req.preferred_body_id,
        req.preferred_wheel_id,
        req.preferred_glider_id,
    )? {
        helpers::require_exists::<characters::Entity, _>(db, setup.character_id, "character")
            .await?;
        helpers::require_exists::<bodies::Entity, _>(db, setup.body_id, "body").await?;
        helpers::require_exists::<wheels::Entity, _>(db, setup.wheel_id, "wheel").await?;
        helpers::require_exists::<gliders::Entity, _>(db, setup.glider_id, "glider").await?;

        active.preferred_character_id = Set(Some(setup.character_id));
        active.preferred_body_id = Set(Some(setup.body_id));
        active.preferred_wheel_id = Set(Some(setup.wheel_id));
        active.preferred_glider_id = Set(Some(setup.glider_id));
    }

    // Drink type: independent, can be set or cleared
    if let Some(dt_id_option) = req.preferred_drink_type_id {
        if let Some(ref dt_id) = dt_id_option {
            helpers::require_exists::<drink_types::Entity, _>(db, dt_id.clone(), "drink_type")
                .await?;
        }
        active.preferred_drink_type_id = Set(dt_id_option);
    }

    active.updated_at = Set(chrono::Utc::now().naive_utc());
    let updated = active.update(db).await?;

    build_detail_profile(db, updated).await
}

/// Load drink type details and assemble the full profile response.
pub async fn build_detail_profile(
    db: &DatabaseConnection,
    user: users::Model,
) -> Result<UserDetailProfile, AppError> {
    let drink_type = if let Some(ref dt_id) = user.preferred_drink_type_id {
        drink_types::Entity::find_by_id(dt_id)
            .one(db)
            .await?
            .map(|dt| DrinkTypeInfo {
                id: dt.id,
                name: dt.name,
                alcoholic: dt.alcoholic,
            })
    } else {
        None
    };

    Ok(UserDetailProfile {
        id: user.id,
        username: user.username,
        preferred_character_id: user.preferred_character_id,
        preferred_body_id: user.preferred_body_id,
        preferred_wheel_id: user.preferred_wheel_id,
        preferred_glider_id: user.preferred_glider_id,
        preferred_drink_type: drink_type,
        created_at: user.created_at.and_utc(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_user, seed_game_data, setup_db};

    fn drink_id() -> String {
        crate::drink_type_id::drink_type_uuid("Test Beer")
    }

    #[tokio::test]
    async fn test_update_profile_self_only() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_a = create_user(&db, "alice").await;
        let user_b = create_user(&db, "bob").await;

        let result = update_profile(
            &db,
            &user_a,
            &user_b,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(AppError::Forbidden(_))));
    }

    #[tokio::test]
    async fn test_update_profile_user_not_found() {
        let db = setup_db().await;

        let result = update_profile(
            &db,
            "nonexistent",
            "nonexistent",
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(AppError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_update_profile_full_race_setup() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        let profile = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: Some(1),
                preferred_body_id: Some(1),
                preferred_wheel_id: Some(1),
                preferred_glider_id: Some(1),
                preferred_drink_type_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(profile.preferred_character_id, Some(1));
        assert_eq!(profile.preferred_body_id, Some(1));
        assert_eq!(profile.preferred_wheel_id, Some(1));
        assert_eq!(profile.preferred_glider_id, Some(1));
    }

    #[tokio::test]
    async fn test_update_profile_partial_race_setup_rejected() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        let result = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: Some(1),
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_update_profile_invalid_character_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        let result = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: Some(999),
                preferred_body_id: Some(1),
                preferred_wheel_id: Some(1),
                preferred_glider_id: Some(1),
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_update_profile_invalid_drink_type_id() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        let result = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: Some(Some("bad-uuid".to_string())),
            },
        )
        .await;

        assert!(matches!(result, Err(AppError::BadRequest(_))));
    }

    #[tokio::test]
    async fn test_update_profile_set_drink_type() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        let profile = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: Some(Some(drink_id())),
            },
        )
        .await
        .unwrap();

        assert!(profile.preferred_drink_type.is_some());
        assert_eq!(profile.preferred_drink_type.unwrap().name, "Test Beer");
    }

    #[tokio::test]
    async fn test_update_profile_clear_drink_type() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        // First set it
        update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: Some(Some(drink_id())),
            },
        )
        .await
        .unwrap();

        // Then clear it
        let profile = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: Some(None),
            },
        )
        .await
        .unwrap();

        assert!(profile.preferred_drink_type.is_none());
    }

    #[tokio::test]
    async fn test_update_profile_drink_type_untouched_when_absent() {
        let db = setup_db().await;
        seed_game_data(&db).await;
        let user_id = create_user(&db, "racer").await;

        // Set drink type
        update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: Some(Some(drink_id())),
            },
        )
        .await
        .unwrap();

        // Update race setup only — drink type should be untouched
        let profile = update_profile(
            &db,
            &user_id,
            &user_id,
            UpdateProfileRequest {
                preferred_character_id: Some(1),
                preferred_body_id: Some(1),
                preferred_wheel_id: Some(1),
                preferred_glider_id: Some(1),
                preferred_drink_type_id: None, // absent = don't change
            },
        )
        .await
        .unwrap();

        assert!(profile.preferred_drink_type.is_some());
        assert_eq!(profile.preferred_drink_type.unwrap().name, "Test Beer");
    }
}

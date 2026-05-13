use sea_orm::{ActiveModelTrait, ConnectionTrait, EntityTrait, Set};
use serde::{Deserialize, Serialize};

use crate::{
    domain::{BodyId, CharacterId, DrinkTypeId, GliderId, UserId, WheelId, race_setup},
    entities::{bodies, characters, drink_types, gliders, users, wheels},
    error::Error,
    services::helpers,
};

// ── Types ────────────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DrinkTypeInfo {
    pub id: DrinkTypeId,
    pub name: String,
    pub alcoholic: bool,
}

#[derive(Serialize)]
pub struct UserDetailProfile {
    pub id: UserId,
    pub username: String,
    pub preferred_character_id: Option<CharacterId>,
    pub preferred_body_id: Option<BodyId>,
    pub preferred_wheel_id: Option<WheelId>,
    pub preferred_glider_id: Option<GliderId>,
    pub preferred_drink_type: Option<DrinkTypeInfo>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Request body for `PUT /users/:id`.
///
/// `preferred_drink_type_id` uses `Option<Option<DrinkTypeId>>` to
/// distinguish three states: key absent (don't change), key present with
/// null (clear), key present with value (set).
#[derive(Deserialize)]
pub struct UpdateProfileRequest {
    pub preferred_character_id: Option<CharacterId>,
    pub preferred_body_id: Option<BodyId>,
    pub preferred_wheel_id: Option<WheelId>,
    pub preferred_glider_id: Option<GliderId>,
    #[serde(default, deserialize_with = "deserialize_optional_field")]
    pub preferred_drink_type_id: Option<Option<DrinkTypeId>>,
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
///
/// # Errors
///
/// Returns `Forbidden` if `actor_user_id != target_user_id`; `NotFound` if
/// the target user doesn't exist; `BadRequest` if race setup is provided as
/// a partial set (not all-or-nothing) or any of the character / body / wheel
/// / glider / drink-type IDs doesn't exist; `Internal` for DB failures.
#[tracing::instrument(
    skip(db, req),
    fields(actor_user_id = %actor_user_id, target_user_id = %target_user_id),
)]
pub async fn update_profile(
    db: &impl ConnectionTrait,
    actor_user_id: &UserId,
    target_user_id: &UserId,
    req: UpdateProfileRequest,
) -> Result<UserDetailProfile, Error> {
    if actor_user_id != target_user_id {
        return Err(Error::Forbidden(
            "You can only update your own profile".to_string(),
        ));
    }

    let user = users::Entity::find_by_id(target_user_id)
        .one(db)
        .await?
        .ok_or_else(|| Error::NotFound(format!("User {target_user_id} not found")))?;

    let mut active: users::ActiveModel = user.into();

    // Race setup: all-or-nothing via race_setup::Update
    if let Some(setup) = race_setup::Update::try_from_optional(
        req.preferred_character_id,
        req.preferred_body_id,
        req.preferred_wheel_id,
        req.preferred_glider_id,
    )? {
        helpers::require_exists::<characters::Entity, _>(
            db,
            setup.character_id.into(),
            "character",
        )
        .await?;
        helpers::require_exists::<bodies::Entity, _>(db, setup.body_id.into(), "body").await?;
        helpers::require_exists::<wheels::Entity, _>(db, setup.wheel_id.into(), "wheel").await?;
        helpers::require_exists::<gliders::Entity, _>(db, setup.glider_id.into(), "glider").await?;

        active.preferred_character_id = Set(Some(setup.character_id.into()));
        active.preferred_body_id = Set(Some(setup.body_id.into()));
        active.preferred_wheel_id = Set(Some(setup.wheel_id.into()));
        active.preferred_glider_id = Set(Some(setup.glider_id.into()));
    }

    // Drink type: independent, can be set or cleared
    if let Some(dt_id_option) = req.preferred_drink_type_id {
        if let Some(ref dt_id) = dt_id_option {
            helpers::require_exists::<drink_types::Entity, _>(db, dt_id.into(), "drink_type")
                .await?;
        }
        active.preferred_drink_type_id = Set(dt_id_option.as_ref().map(Into::into));
    }

    // `updated_at` is bumped by `users::ActiveModelBehavior::before_save`.
    let updated = active.update(db).await?;

    build_detail_profile(db, updated).await
}

/// Load drink type details and assemble the full profile response.
///
/// TODO: if profile fetching becomes a hot path, collapse the separate
/// `drink_type` lookup into a JOIN query to avoid the extra round trip.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(level = "debug", skip(db, user), fields(user_id = %user.id))]
pub async fn build_detail_profile(
    db: &impl ConnectionTrait,
    user: users::Model,
) -> Result<UserDetailProfile, Error> {
    let drink_type = if let Some(ref dt_id) = user.preferred_drink_type_id {
        let row = drink_types::Entity::find_by_id(dt_id).one(db).await?;
        row.map(|dt| {
            Ok::<_, Error>(DrinkTypeInfo {
                id: DrinkTypeId::from_db(&dt.id)?,
                name: dt.name,
                alcoholic: dt.alcoholic,
            })
        })
        .transpose()?
    } else {
        None
    };

    Ok(UserDetailProfile {
        id: UserId::from_db(&user.id)?,
        username: user.username,
        preferred_character_id: user.preferred_character_id.map(CharacterId::new),
        preferred_body_id: user.preferred_body_id.map(BodyId::new),
        preferred_wheel_id: user.preferred_wheel_id.map(WheelId::new),
        preferred_glider_id: user.preferred_glider_id.map(GliderId::new),
        preferred_drink_type: drink_type,
        created_at: user.created_at.and_utc(),
    })
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;

    use super::*;
    use crate::test_helpers::{create_user, seed_game_data, setup_db};

    fn drink_id() -> DrinkTypeId {
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

        assert!(matches!(result, Err(Error::Forbidden(_))));
    }

    #[tokio::test]
    async fn test_update_profile_user_not_found() {
        let db = setup_db().await;
        let nonexistent = UserId::new(Uuid::new_v4());

        let result = update_profile(
            &db,
            &nonexistent,
            &nonexistent,
            UpdateProfileRequest {
                preferred_character_id: None,
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(Error::NotFound(_))));
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
                preferred_character_id: Some(CharacterId::new(1)),
                preferred_body_id: Some(BodyId::new(1)),
                preferred_wheel_id: Some(WheelId::new(1)),
                preferred_glider_id: Some(GliderId::new(1)),
                preferred_drink_type_id: None,
            },
        )
        .await
        .unwrap();

        assert_eq!(profile.preferred_character_id, Some(CharacterId::new(1)));
        assert_eq!(profile.preferred_body_id, Some(BodyId::new(1)));
        assert_eq!(profile.preferred_wheel_id, Some(WheelId::new(1)));
        assert_eq!(profile.preferred_glider_id, Some(GliderId::new(1)));
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
                preferred_character_id: Some(CharacterId::new(1)),
                preferred_body_id: None,
                preferred_wheel_id: None,
                preferred_glider_id: None,
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(Error::BadRequest { .. })));
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
                preferred_character_id: Some(CharacterId::new(999)),
                preferred_body_id: Some(BodyId::new(1)),
                preferred_wheel_id: Some(WheelId::new(1)),
                preferred_glider_id: Some(GliderId::new(1)),
                preferred_drink_type_id: None,
            },
        )
        .await;

        assert!(matches!(result, Err(Error::BadRequest { .. })));
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
                // Random unrelated UUID — passes the type-level parse but
                // misses the FK in the database, so the boundary returns 400.
                preferred_drink_type_id: Some(Some(DrinkTypeId::new_v4())),
            },
        )
        .await;

        assert!(matches!(result, Err(Error::BadRequest { .. })));
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
                preferred_character_id: Some(CharacterId::new(1)),
                preferred_body_id: Some(BodyId::new(1)),
                preferred_wheel_id: Some(WheelId::new(1)),
                preferred_glider_id: Some(GliderId::new(1)),
                preferred_drink_type_id: None, // absent = don't change
            },
        )
        .await
        .unwrap();

        assert!(profile.preferred_drink_type.is_some());
        assert_eq!(profile.preferred_drink_type.unwrap().name, "Test Beer");
    }
}

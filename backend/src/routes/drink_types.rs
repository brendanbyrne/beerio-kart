use axum::{
    Json,
    extract::{Path, Query, State},
};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{DrinkTypeId, UserId},
    drink_type_id::drink_type_uuid,
    entities::drink_types,
    error::Error,
    middleware::auth::User,
};

// ── Response types ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DrinkTypeResponse {
    pub id: DrinkTypeId,
    pub name: String,
    pub alcoholic: bool,
    pub created_by: Option<UserId>,
    pub created_at: DateTime<Utc>,
}

impl DrinkTypeResponse {
    /// Parse a `drink_types::Model` into the wire DTO. Fallible because
    /// the `id` and (optional) `created_by` columns are stored as UUID
    /// strings and have to round-trip through `from_db`; a bad UUID in
    /// either column is data corruption and surfaces as `Internal`.
    fn try_from_model(m: drink_types::Model) -> Result<Self, Error> {
        let created_by = m.created_by.as_deref().map(UserId::from_db).transpose()?;
        Ok(Self {
            id: DrinkTypeId::from_db(&m.id)?,
            name: m.name,
            alcoholic: m.alcoholic,
            created_by,
            created_at: m.created_at.and_utc(),
        })
    }
}

// ── Request types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateDrinkTypeRequest {
    pub name: String,
    pub alcoholic: bool,
}

#[derive(Deserialize)]
pub struct Filters {
    pub alcoholic: Option<bool>,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /api/v1/drink-types — create or return an existing drink type.
///
/// Dedup is case-insensitive (the UUID is derived from the uppercased name),
/// so re-submitting an existing name returns the original row with 200 rather
/// than a 409.
///
/// # Errors
///
/// Returns `BadRequest` if the trimmed name is empty or >200 chars;
/// `Internal` for unexpected DB failures.
#[tracing::instrument(
    skip_all,
    fields(user_id = %user.user_id, name = %req.name, alcoholic = req.alcoholic),
)]
pub async fn create_drink_type(
    user: User,
    State(state): State<AppState>,
    Json(req): Json<CreateDrinkTypeRequest>,
) -> Result<Json<DrinkTypeResponse>, Error> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(Error::bad_request("Drink type name cannot be empty"));
    }
    if name.len() > 200 {
        return Err(Error::bad_request(
            "Drink type name must be 200 characters or fewer",
        ));
    }

    // Deterministic UUID from uppercased name — case-insensitive dedup
    let id = drink_type_uuid(&name);

    // Check if this drink type already exists (by UUID)
    if let Some(existing) = drink_types::Entity::find_by_id(id).one(&state.db).await? {
        // Return the existing entry (200, not 409)
        return Ok(Json(DrinkTypeResponse::try_from_model(existing)?));
    }

    // `created_at` is populated by `drink_types::ActiveModelBehavior::before_save`.
    let model = drink_types::ActiveModel {
        id: Set((&id).into()),
        name: Set(name),
        alcoholic: Set(req.alcoholic),
        created_at: NotSet,
        created_by: Set(Some((&user.user_id).into())),
    };

    let inserted = sea_orm::ActiveModelTrait::insert(model, &state.db).await?;

    Ok(Json(DrinkTypeResponse::try_from_model(inserted)?))
}

/// GET /api/v1/drink-types — list drink types. Optional `alcoholic` query
/// filter narrows the result.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(
    skip_all,
    fields(user_id = %user.user_id, alcoholic = ?params.alcoholic),
)]
pub async fn list_drink_types(
    user: User,
    State(state): State<AppState>,
    Query(params): Query<Filters>,
) -> Result<Json<Vec<DrinkTypeResponse>>, Error> {
    let mut query = drink_types::Entity::find();
    if let Some(alcoholic) = params.alcoholic {
        query = query.filter(drink_types::Column::Alcoholic.eq(alcoholic));
    }

    let items = query.all(&state.db).await?;
    Ok(Json(
        items
            .into_iter()
            .map(DrinkTypeResponse::try_from_model)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

/// GET /api/v1/drink-types/:id — get a single drink type.
///
/// # Errors
///
/// Returns `NotFound` if `id` doesn't match a drink type; `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, drink_type_id = %id))]
pub async fn get_drink_type(
    user: User,
    State(state): State<AppState>,
    Path(id): Path<DrinkTypeId>,
) -> Result<Json<DrinkTypeResponse>, Error> {
    let dt = drink_types::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Drink type {id} not found")))?;

    Ok(Json(DrinkTypeResponse::try_from_model(dt)?))
}

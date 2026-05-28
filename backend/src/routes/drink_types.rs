use axum::extract::{Query, State};
use chrono::{DateTime, Utc};
use sea_orm::{ActiveValue::NotSet, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{DrinkTypeId, DrinkTypeName, UserId},
    drink_type_id::drink_type_uuid,
    entities::drink_types,
    error::Error,
    extract::{Json, Path},
    middleware::auth::User,
    timeout::db_query,
};

// ── Response types ───────────────────────────────────────────────────

/// Wire representation of a `drink_types` row.
#[derive(Serialize)]
pub struct DrinkTypeResponse {
    /// Stable UUID, derived from `(uppercased name, alcoholic)` (see
    /// `drink_type_id::drink_type_uuid`) for both seeded and user-created
    /// drinks.
    pub id: DrinkTypeId,
    /// Display name. Unique case-insensitively.
    pub name: String,
    /// `true` for alcoholic drinks, `false` for non-alcoholic.
    pub alcoholic: bool,
    /// User who created this drink. `None` for seeded drinks.
    pub created_by: Option<UserId>,
    /// Row-creation timestamp, UTC.
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

/// Body shape for `POST /drink-types`.
#[derive(Deserialize)]
pub struct CreateDrinkTypeRequest {
    /// Display name. Unique case-insensitively *per alcoholic flag*; an
    /// existing `(name, alcoholic)` match returns 200.
    pub name: String,
    /// `true` for alcoholic drinks, `false` for non-alcoholic.
    pub alcoholic: bool,
}

/// Query filters for `GET /drink-types`.
#[derive(Deserialize)]
pub struct Filters {
    /// If set, only return drinks matching this alcoholic flag.
    pub alcoholic: Option<bool>,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// POST /api/v1/drink-types — create or return an existing drink type.
///
/// Dedup keys on `(name, alcoholic)`: the UUID is derived from the uppercased
/// name plus the alcoholic flag, so re-submitting an existing name *with the
/// same flag* returns the original row with 200 rather than a 409. The
/// alcoholic and non-alcoholic forms of the same name are distinct drinks
/// (e.g. alcoholic vs non-alcoholic "Punch") and coexist.
///
/// # Errors
///
/// Returns `BadRequest` if the trimmed name isn't 1-200 characters;
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
    let name = DrinkTypeName::try_from(req.name)
        .map_err(|_| Error::bad_request("Drink type name must be 1-200 characters"))?;

    // Deterministic UUID from (uppercased name, alcoholic) — case-insensitive
    // dedup that keeps the alcoholic and non-alcoholic forms distinct.
    let id = drink_type_uuid(name.as_ref(), req.alcoholic);

    // Check if this drink type already exists (by UUID)
    if let Some(existing) = db_query(drink_types::Entity::find_by_id(id).one(&state.db)).await? {
        // Return the existing entry (200, not 409)
        return Ok(Json(DrinkTypeResponse::try_from_model(existing)?));
    }

    // `created_at` is populated by `drink_types::ActiveModelBehavior::before_save`.
    let model = drink_types::ActiveModel {
        id: Set((&id).into()),
        name: Set(name.into_inner()),
        alcoholic: Set(req.alcoholic),
        created_at: NotSet,
        created_by: Set(Some((&user.user_id).into())),
    };

    let inserted = db_query(sea_orm::ActiveModelTrait::insert(model, &state.db)).await?;

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

    let items = db_query(query.all(&state.db)).await?;
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
    let dt = db_query(drink_types::Entity::find_by_id(id).one(&state.db))
        .await?
        .ok_or_else(|| Error::NotFound(format!("Drink type {id} not found")))?;

    Ok(Json(DrinkTypeResponse::try_from_model(dt)?))
}

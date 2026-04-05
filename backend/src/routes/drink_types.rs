use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::drink_type_id::drink_type_uuid;
use crate::entities::drink_types;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;

// ── Response types ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct DrinkTypeResponse {
    pub id: String,
    pub name: String,
    pub alcoholic: bool,
    pub created_by: Option<String>,
    pub created_at: String,
}

// ── Request types ────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CreateDrinkTypeRequest {
    pub name: String,
    pub alcoholic: bool,
}

#[derive(Deserialize)]
pub struct DrinkTypesQuery {
    pub alcoholic: Option<bool>,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn create_drink_type(
    user: AuthUser,
    State(state): State<AppState>,
    Json(req): Json<CreateDrinkTypeRequest>,
) -> Result<Json<DrinkTypeResponse>, AppError> {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "Drink type name cannot be empty".to_string(),
        ));
    }

    // Deterministic UUID from uppercased name — case-insensitive dedup
    let id = drink_type_uuid(&name);

    // Check if this drink type already exists (by UUID)
    if let Some(existing) = drink_types::Entity::find_by_id(&id).one(&state.db).await? {
        // Return the existing entry (200, not 409)
        return Ok(Json(DrinkTypeResponse {
            id: existing.id,
            name: existing.name,
            alcoholic: existing.alcoholic,
            created_by: existing.created_by,
            created_at: existing.created_at,
        }));
    }

    let now = chrono::Utc::now().to_rfc3339();
    let model = drink_types::ActiveModel {
        id: Set(id),
        name: Set(name),
        alcoholic: Set(req.alcoholic),
        created_at: Set(now),
        created_by: Set(Some(user.user_id)),
    };

    let inserted = sea_orm::ActiveModelTrait::insert(model, &state.db).await?;

    Ok(Json(DrinkTypeResponse {
        id: inserted.id,
        name: inserted.name,
        alcoholic: inserted.alcoholic,
        created_by: inserted.created_by,
        created_at: inserted.created_at,
    }))
}

pub async fn list_drink_types(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<DrinkTypesQuery>,
) -> Result<Json<Vec<DrinkTypeResponse>>, AppError> {
    let mut query = drink_types::Entity::find();
    if let Some(alcoholic) = params.alcoholic {
        query = query.filter(drink_types::Column::Alcoholic.eq(alcoholic));
    }

    let items = query.all(&state.db).await?;
    Ok(Json(
        items
            .into_iter()
            .map(|dt| DrinkTypeResponse {
                id: dt.id,
                name: dt.name,
                alcoholic: dt.alcoholic,
                created_by: dt.created_by,
                created_at: dt.created_at,
            })
            .collect(),
    ))
}

pub async fn get_drink_type(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DrinkTypeResponse>, AppError> {
    let dt = drink_types::Entity::find_by_id(&id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Drink type {id} not found")))?;

    Ok(Json(DrinkTypeResponse {
        id: dt.id,
        name: dt.name,
        alcoholic: dt.alcoholic,
        created_by: dt.created_by,
        created_at: dt.created_at,
    }))
}

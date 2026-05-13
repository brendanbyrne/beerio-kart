use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{BodyId, CharacterId, CupId, GliderId, TrackId, WheelId},
    entities::{bodies, characters, cups, gliders, tracks, wheels},
    error::Error,
    middleware::auth::User,
};

// ── Response types ───────────────────────────────────────────────────

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CharacterResponse {
    pub id: CharacterId,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BodyResponse {
    pub id: BodyId,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WheelResponse {
    pub id: WheelId,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GliderResponse {
    pub id: GliderId,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CupResponse {
    pub id: CupId,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TrackResponse {
    pub id: TrackId,
    pub name: String,
    pub cup_id: CupId,
    pub position: i32,
    pub image_path: String,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CupWithTracksResponse {
    pub id: CupId,
    pub name: String,
    pub image_path: String,
    pub tracks: Vec<TrackResponse>,
}

impl From<characters::Model> for CharacterResponse {
    fn from(m: characters::Model) -> Self {
        Self {
            id: CharacterId::new(m.id),
            name: m.name,
            image_path: m.image_path,
        }
    }
}

impl From<bodies::Model> for BodyResponse {
    fn from(m: bodies::Model) -> Self {
        Self {
            id: BodyId::new(m.id),
            name: m.name,
            image_path: m.image_path,
        }
    }
}

impl From<wheels::Model> for WheelResponse {
    fn from(m: wheels::Model) -> Self {
        Self {
            id: WheelId::new(m.id),
            name: m.name,
            image_path: m.image_path,
        }
    }
}

impl From<gliders::Model> for GliderResponse {
    fn from(m: gliders::Model) -> Self {
        Self {
            id: GliderId::new(m.id),
            name: m.name,
            image_path: m.image_path,
        }
    }
}

impl From<cups::Model> for CupResponse {
    fn from(m: cups::Model) -> Self {
        Self {
            id: CupId::new(m.id),
            name: m.name,
            image_path: m.image_path,
        }
    }
}

impl From<tracks::Model> for TrackResponse {
    fn from(t: tracks::Model) -> Self {
        Self {
            id: TrackId::new(t.id),
            name: t.name,
            cup_id: CupId::new(t.cup_id),
            position: t.position,
            image_path: t.image_path,
        }
    }
}

#[derive(Deserialize)]
pub struct TracksQuery {
    pub cup_id: Option<CupId>,
}

// ── Handlers ─────────────────────────────────────────────────────────

/// GET /api/v1/characters — list all characters.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_characters(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<CharacterResponse>>, Error> {
    let items = characters::Entity::find().all(&state.db).await?;
    Ok(Json(
        items.into_iter().map(CharacterResponse::from).collect(),
    ))
}

/// GET /api/v1/bodies — list all kart bodies.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_bodies(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<BodyResponse>>, Error> {
    let items = bodies::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(BodyResponse::from).collect()))
}

/// GET /api/v1/wheels — list all wheels.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_wheels(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<WheelResponse>>, Error> {
    let items = wheels::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(WheelResponse::from).collect()))
}

/// GET /api/v1/gliders — list all gliders.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_gliders(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<GliderResponse>>, Error> {
    let items = gliders::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(GliderResponse::from).collect()))
}

/// GET /api/v1/cups — list all cups.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_cups(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<CupResponse>>, Error> {
    let items = cups::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(CupResponse::from).collect()))
}

/// GET /api/v1/cups/:id — get a cup with its tracks.
///
/// # Errors
///
/// Returns `NotFound` if `id` doesn't match a cup; `Internal` for unexpected
/// DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, cup_id = %id))]
pub async fn get_cup(
    user: User,
    State(state): State<AppState>,
    Path(id): Path<CupId>,
) -> Result<Json<CupWithTracksResponse>, Error> {
    let cup = cups::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Cup {id} not found")))?;

    let cup_tracks: Vec<TrackResponse> = tracks::Entity::find()
        .filter(tracks::Column::CupId.eq(id))
        .all(&state.db)
        .await?
        .into_iter()
        .map(TrackResponse::from)
        .collect();

    Ok(Json(CupWithTracksResponse {
        id: CupId::new(cup.id),
        name: cup.name,
        image_path: cup.image_path,
        tracks: cup_tracks,
    }))
}

/// GET /api/v1/tracks — list all tracks. Optional `cup_id` query filter
/// narrows the result to a single cup.
///
/// # Errors
///
/// Returns `Internal` for unexpected DB failures.
#[tracing::instrument(
    skip_all,
    fields(user_id = %user.user_id, cup_id = ?params.cup_id),
)]
pub async fn list_tracks(
    user: User,
    State(state): State<AppState>,
    Query(params): Query<TracksQuery>,
) -> Result<Json<Vec<TrackResponse>>, Error> {
    let mut query = tracks::Entity::find();
    if let Some(cup_id) = params.cup_id {
        query = query.filter(tracks::Column::CupId.eq(cup_id));
    }

    let items = query.all(&state.db).await?;
    Ok(Json(items.into_iter().map(TrackResponse::from).collect()))
}

/// GET /api/v1/tracks/:id — get a single track.
///
/// # Errors
///
/// Returns `NotFound` if `id` doesn't match a track; `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, track_id = %id))]
pub async fn get_track(
    user: User,
    State(state): State<AppState>,
    Path(id): Path<TrackId>,
) -> Result<Json<TrackResponse>, Error> {
    let track = tracks::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| Error::NotFound(format!("Track {id} not found")))?;

    Ok(Json(track.into()))
}

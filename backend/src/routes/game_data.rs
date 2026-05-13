use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{BodyId, CharacterId, CupId, GliderId, ImagePath, TrackId, WheelId},
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
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct BodyResponse {
    pub id: BodyId,
    pub name: String,
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct WheelResponse {
    pub id: WheelId,
    pub name: String,
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct GliderResponse {
    pub id: GliderId,
    pub name: String,
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CupResponse {
    pub id: CupId,
    pub name: String,
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct TrackResponse {
    pub id: TrackId,
    pub name: String,
    pub cup_id: CupId,
    pub position: i32,
    pub image_path: ImagePath,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
pub struct CupWithTracksResponse {
    pub id: CupId,
    pub name: String,
    pub image_path: ImagePath,
    pub tracks: Vec<TrackResponse>,
}

// Entity → DTO conversions are fallible because `ImagePath::from_db` rejects
// empty or pathological values read from a seed row. A failure indicates
// corruption / a mis-edited seed and surfaces as `Internal`, matching the
// `from_db` convention on UUID newtypes in [`crate::domain::ids`].

impl TryFrom<characters::Model> for CharacterResponse {
    type Error = Error;
    fn try_from(m: characters::Model) -> Result<Self, Error> {
        Ok(Self {
            id: CharacterId::new(m.id),
            name: m.name,
            image_path: ImagePath::from_db(m.image_path, "characters.image_path")?,
        })
    }
}

impl TryFrom<bodies::Model> for BodyResponse {
    type Error = Error;
    fn try_from(m: bodies::Model) -> Result<Self, Error> {
        Ok(Self {
            id: BodyId::new(m.id),
            name: m.name,
            image_path: ImagePath::from_db(m.image_path, "bodies.image_path")?,
        })
    }
}

impl TryFrom<wheels::Model> for WheelResponse {
    type Error = Error;
    fn try_from(m: wheels::Model) -> Result<Self, Error> {
        Ok(Self {
            id: WheelId::new(m.id),
            name: m.name,
            image_path: ImagePath::from_db(m.image_path, "wheels.image_path")?,
        })
    }
}

impl TryFrom<gliders::Model> for GliderResponse {
    type Error = Error;
    fn try_from(m: gliders::Model) -> Result<Self, Error> {
        Ok(Self {
            id: GliderId::new(m.id),
            name: m.name,
            image_path: ImagePath::from_db(m.image_path, "gliders.image_path")?,
        })
    }
}

impl TryFrom<cups::Model> for CupResponse {
    type Error = Error;
    fn try_from(m: cups::Model) -> Result<Self, Error> {
        Ok(Self {
            id: CupId::new(m.id),
            name: m.name,
            image_path: ImagePath::from_db(m.image_path, "cups.image_path")?,
        })
    }
}

impl TryFrom<tracks::Model> for TrackResponse {
    type Error = Error;
    fn try_from(t: tracks::Model) -> Result<Self, Error> {
        Ok(Self {
            id: TrackId::new(t.id),
            name: t.name,
            cup_id: CupId::new(t.cup_id),
            position: t.position,
            image_path: ImagePath::from_db(t.image_path, "tracks.image_path")?,
        })
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
        items
            .into_iter()
            .map(CharacterResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
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
    Ok(Json(
        items
            .into_iter()
            .map(BodyResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
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
    Ok(Json(
        items
            .into_iter()
            .map(WheelResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
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
    Ok(Json(
        items
            .into_iter()
            .map(GliderResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
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
    Ok(Json(
        items
            .into_iter()
            .map(CupResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
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
        .map(TrackResponse::try_from)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Json(CupWithTracksResponse {
        id: CupId::new(cup.id),
        name: cup.name,
        image_path: ImagePath::from_db(cup.image_path, "cups.image_path")?,
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
    Ok(Json(
        items
            .into_iter()
            .map(TrackResponse::try_from)
            .collect::<Result<Vec<_>, _>>()?,
    ))
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

    Ok(Json(TrackResponse::try_from(track)?))
}

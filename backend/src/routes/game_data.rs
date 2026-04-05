use axum::{
    Json,
    extract::{Path, Query, State},
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::entities::{bodies, characters, cups, gliders, tracks, wheels};
use crate::error::AppError;
use crate::middleware::auth::AuthUser;

// ── Response types ───────────────────────────────────────────────────

#[derive(Serialize)]
pub struct SimpleItem {
    pub id: i32,
    pub name: String,
    pub image_path: String,
}

#[derive(Serialize)]
pub struct TrackResponse {
    pub id: i32,
    pub name: String,
    pub cup_id: i32,
    pub position: i32,
    pub image_path: String,
}

#[derive(Serialize)]
pub struct CupWithTracksResponse {
    pub id: i32,
    pub name: String,
    pub image_path: String,
    pub tracks: Vec<TrackResponse>,
}

/// Convert any entity Model with (id: i32, name: String, image_path: String) to SimpleItem.
macro_rules! impl_into_simple_item {
    ($($module:ident),+) => {
        $(
            impl From<$module::Model> for SimpleItem {
                fn from(m: $module::Model) -> Self {
                    Self { id: m.id, name: m.name, image_path: m.image_path }
                }
            }
        )+
    };
}

impl_into_simple_item!(characters, bodies, wheels, gliders, cups);

impl From<tracks::Model> for TrackResponse {
    fn from(t: tracks::Model) -> Self {
        Self {
            id: t.id,
            name: t.name,
            cup_id: t.cup_id,
            position: t.position,
            image_path: t.image_path,
        }
    }
}

#[derive(Deserialize)]
pub struct TracksQuery {
    pub cup_id: Option<i32>,
}

// ── Handlers ─────────────────────────────────────────────────────────

pub async fn list_characters(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SimpleItem>>, AppError> {
    let items = characters::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(SimpleItem::from).collect()))
}

pub async fn list_bodies(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SimpleItem>>, AppError> {
    let items = bodies::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(SimpleItem::from).collect()))
}

pub async fn list_wheels(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SimpleItem>>, AppError> {
    let items = wheels::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(SimpleItem::from).collect()))
}

pub async fn list_gliders(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SimpleItem>>, AppError> {
    let items = gliders::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(SimpleItem::from).collect()))
}

pub async fn list_cups(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SimpleItem>>, AppError> {
    let items = cups::Entity::find().all(&state.db).await?;
    Ok(Json(items.into_iter().map(SimpleItem::from).collect()))
}

pub async fn get_cup(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<CupWithTracksResponse>, AppError> {
    let cup = cups::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Cup {id} not found")))?;

    let cup_tracks: Vec<TrackResponse> = tracks::Entity::find()
        .filter(tracks::Column::CupId.eq(id))
        .all(&state.db)
        .await?
        .into_iter()
        .map(TrackResponse::from)
        .collect();

    Ok(Json(CupWithTracksResponse {
        id: cup.id,
        name: cup.name,
        image_path: cup.image_path,
        tracks: cup_tracks,
    }))
}

pub async fn list_tracks(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(params): Query<TracksQuery>,
) -> Result<Json<Vec<TrackResponse>>, AppError> {
    let mut query = tracks::Entity::find();
    if let Some(cup_id) = params.cup_id {
        query = query.filter(tracks::Column::CupId.eq(cup_id));
    }

    let items = query.all(&state.db).await?;
    Ok(Json(items.into_iter().map(TrackResponse::from).collect()))
}

pub async fn get_track(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<TrackResponse>, AppError> {
    let track = tracks::Entity::find_by_id(id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Track {id} not found")))?;

    Ok(Json(track.into()))
}

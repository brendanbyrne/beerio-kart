use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::AppState;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::runs;

/// POST /runs — create a run.
pub async fn create_run(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<runs::CreateRunRequest>,
) -> Result<(StatusCode, Json<runs::RunDetail>), AppError> {
    let detail = runs::create_run(&state.db, &user.user_id, body).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Deserialize)]
pub struct ListRunsQuery {
    pub session_race_id: Option<String>,
    pub user_id: Option<String>,
    pub track_id: Option<i32>,
}

/// GET /runs — list runs with optional filters.
pub async fn list_runs(
    _user: AuthUser,
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<runs::RunDetail>>, AppError> {
    let filters = runs::RunFilters {
        session_race_id: query.session_race_id,
        user_id: query.user_id,
        track_id: query.track_id,
    };
    let results = runs::list_runs(&state.db, filters).await?;
    Ok(Json(results))
}

/// GET /runs/defaults — get defaults for the authenticated user.
pub async fn get_defaults(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<runs::RunDefaults>, AppError> {
    let defaults = runs::get_run_defaults(&state.db, &user.user_id).await?;
    Ok(Json(defaults))
}

/// GET /runs/:id — get a single run.
pub async fn get_run(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<runs::RunDetail>, AppError> {
    let detail = runs::get_run(&state.db, &id).await?;
    Ok(Json(detail))
}

/// DELETE /runs/:id — delete a run. Owner only, active session only.
pub async fn delete_run(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    runs::delete_run(&state.db, &id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

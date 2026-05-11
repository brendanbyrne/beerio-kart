use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{
    AppState,
    domain::{RunId, SessionRaceId, UserId},
    error::Error,
    middleware::auth::User,
    services::runs,
};

/// POST /runs — create a run.
///
/// # Errors
///
/// Propagates the errors of [`runs::create_run`]: `BadRequest` for invalid
/// time fields or unknown FK references, `NotFound` if the session race
/// doesn't exist, `Conflict` if the session is closed, the user already
/// submitted, or an older pending race is blocking, `Forbidden` if the user
/// is not an active participant.
pub async fn create_run(
    user: User,
    State(state): State<AppState>,
    Json(body): Json<runs::CreateRunRequest>,
) -> Result<(StatusCode, Json<runs::RunDetail>), Error> {
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
///
/// # Errors
///
/// Propagates the errors of [`runs::list_runs`] — currently only `Internal`
/// for unexpected DB failures.
pub async fn list_runs(
    _user: User,
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<runs::RunDetail>>, Error> {
    let filters = runs::RunFilters {
        session_race_id: query.session_race_id.map(SessionRaceId::new),
        user_id: query.user_id.map(UserId::new),
        track_id: query.track_id,
    };
    let results = runs::list_runs(&state.db, filters).await?;
    Ok(Json(results))
}

/// GET /runs/defaults — get defaults for the authenticated user.
///
/// # Errors
///
/// Propagates the errors of [`runs::get_run_defaults`] — currently only
/// `Internal` for unexpected DB failures.
pub async fn get_defaults(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<runs::RunDefaults>, Error> {
    let defaults = runs::get_run_defaults(&state.db, &user.user_id).await?;
    Ok(Json(defaults))
}

/// GET /runs/:id — get a single run.
///
/// # Errors
///
/// Propagates the errors of [`runs::get_run`]: `NotFound` if the run doesn't
/// exist.
pub async fn get_run(
    _user: User,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<runs::RunDetail>, Error> {
    let run_id = RunId::new(id);
    let detail = runs::get_run(&state.db, &run_id).await?;
    Ok(Json(detail))
}

/// DELETE /runs/:id — delete a run. Owner only, active session only.
///
/// # Errors
///
/// Propagates the errors of [`runs::delete_run`]: `NotFound` if the run
/// doesn't exist, `Forbidden` if the caller is not the run's owner,
/// `Conflict` if the run's session is closed.
pub async fn delete_run(
    user: User,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, Error> {
    let run_id = RunId::new(id);
    runs::delete_run(&state.db, &run_id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

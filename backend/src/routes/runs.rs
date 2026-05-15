use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{
    AppState,
    domain::{RunId, SessionRaceId, TrackId, UserId},
    error::Error,
    extract::{Json, Path},
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
#[tracing::instrument(
    skip_all,
    fields(user_id = %user.user_id, session_race_id = %body.session_race_id),
)]
pub async fn create_run(
    user: User,
    State(state): State<AppState>,
    Json(body): Json<runs::CreateRunRequest>,
) -> Result<(StatusCode, Json<runs::RunDetail>), Error> {
    let detail = runs::create_run(&state.db, &user.user_id, body).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

/// Query-string filters for `GET /runs`. All four are independent; omitted
/// fields don't constrain the result.
#[derive(Deserialize)]
pub struct ListRunsQuery {
    /// If set, only return runs against this session race.
    pub session_race_id: Option<SessionRaceId>,
    /// If set, only return runs submitted by this user.
    pub user_id: Option<UserId>,
    /// If set, only return runs on this track (regardless of session).
    pub track_id: Option<TrackId>,
}

/// GET /runs — list runs with optional filters.
///
/// # Errors
///
/// Propagates the errors of [`runs::list_runs`] — currently only `Internal`
/// for unexpected DB failures.
#[tracing::instrument(
    skip_all,
    fields(
        user_id = %user.user_id,
        session_race_id = ?query.session_race_id,
        filter_user_id = ?query.user_id,
        track_id = ?query.track_id,
    ),
)]
pub async fn list_runs(
    user: User,
    State(state): State<AppState>,
    Query(query): Query<ListRunsQuery>,
) -> Result<Json<Vec<runs::RunDetail>>, Error> {
    let filters = runs::RunFilters {
        session_race_id: query.session_race_id,
        user_id: query.user_id,
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
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
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
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, run_id = %run_id))]
pub async fn get_run(
    user: User,
    State(state): State<AppState>,
    Path(run_id): Path<RunId>,
) -> Result<Json<runs::RunDetail>, Error> {
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
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, run_id = %run_id))]
pub async fn delete_run(
    user: User,
    State(state): State<AppState>,
    Path(run_id): Path<RunId>,
) -> Result<impl IntoResponse, Error> {
    runs::delete_run(&state.db, &run_id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

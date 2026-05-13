use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState,
    domain::{SessionId, SessionRaceId},
    error::Error,
    middleware::auth::User,
    services::sessions,
};

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub ruleset: String,
}

/// POST /sessions — create a new session. Returns full session detail.
///
/// # Errors
///
/// Propagates the errors of [`sessions::create_session`]: `BadRequest` for an
/// unknown ruleset, `Conflict` if the user is already in another active session.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, ruleset = %body.ruleset))]
pub async fn create_session(
    user: User,
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, Error> {
    let detail = sessions::create_session(&state.db, &user.user_id, &body.ruleset).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Serialize)]
pub struct MySessionResponse {
    pub session_id: Option<SessionId>,
}

/// GET /sessions/mine — get the user's current active session ID.
///
/// # Errors
///
/// Propagates the errors of [`sessions::get_active_session_id`] — currently
/// only `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn my_session(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<MySessionResponse>, Error> {
    let session_id = sessions::get_active_session_id(&state.db, &user.user_id).await?;
    Ok(Json(MySessionResponse { session_id }))
}

/// GET /sessions — list active sessions.
///
/// # Errors
///
/// Propagates the errors of [`sessions::list_active_sessions`] — currently
/// only `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn list_sessions(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<Vec<sessions::SessionSummary>>, Error> {
    let summaries = sessions::list_active_sessions(&state.db).await?;
    Ok(Json(summaries))
}

/// GET /sessions/:id — full session state for polling.
///
/// # Errors
///
/// Propagates the errors of [`sessions::get_session_detail`]: `NotFound` if
/// the session does not exist.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn get_session(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<Json<sessions::SessionDetail>, Error> {
    let detail = sessions::get_session_detail(&state.db, &session_id, Some(&user.user_id)).await?;
    Ok(Json(detail))
}

/// POST /sessions/:id/join — join a session.
///
/// # Errors
///
/// Propagates the errors of [`sessions::join_session`]: `NotFound` if the
/// session doesn't exist, `Conflict` if the session is closed or the user is
/// already in another session.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn join_session(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<impl IntoResponse, Error> {
    sessions::join_session(&state.db, &session_id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /sessions/:id/leave — leave a session.
///
/// # Errors
///
/// Propagates the errors of [`sessions::leave_session`]: `NotFound` if the
/// session doesn't exist, `BadRequest` if the user is not currently in it.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn leave_session(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<impl IntoResponse, Error> {
    sessions::leave_session(&state.db, &session_id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /sessions/:id/next-track — pick the next random track.
///
/// # Errors
///
/// Propagates the errors of [`sessions::next_track`]: `NotFound` if the
/// session doesn't exist, `Conflict` if it's closed, `Forbidden` if the user
/// is not an active participant.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn next_track(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<(StatusCode, Json<sessions::SessionRaceInfo>), Error> {
    let race = sessions::next_track(&state.db, &session_id, &user.user_id).await?;
    Ok((StatusCode::CREATED, Json(race)))
}

/// POST /sessions/:id/skip-turn — re-roll the current track.
///
/// # Errors
///
/// Propagates the errors of [`sessions::skip_turn`]: `NotFound` if the
/// session doesn't exist, `Conflict` if it's closed or if runs have already
/// been submitted for the current race, `BadRequest` if there is no track to
/// skip.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn skip_turn(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<(StatusCode, Json<sessions::SessionRaceInfo>), Error> {
    let race = sessions::skip_turn(&state.db, &session_id, &user.user_id).await?;
    Ok((StatusCode::CREATED, Json(race)))
}

/// `POST /sessions/:id/races/:race_id/skip` — mark a pending race as skipped
/// for the requesting user. Idempotent.
///
/// # Errors
///
/// Propagates the errors of [`sessions::skip_pending_race`]: `NotFound` if
/// the session or race doesn't exist, `Conflict` if the session is closed or
/// the user already submitted a run for the race, `Forbidden` if the user is
/// not an active participant.
#[tracing::instrument(
    skip_all,
    fields(user_id = %user.user_id, session_id = %session_id, race_id = %race_id),
)]
pub async fn skip_pending_race(
    user: User,
    State(state): State<AppState>,
    Path((session_id, race_id)): Path<(SessionId, SessionRaceId)>,
) -> Result<impl IntoResponse, Error> {
    sessions::skip_pending_race(&state.db, &session_id, &race_id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// GET /sessions/:id/races — list all races in a session.
///
/// # Errors
///
/// Propagates the errors of [`sessions::list_races`] — currently only
/// `Internal` for unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, session_id = %session_id))]
pub async fn list_races(
    user: User,
    State(state): State<AppState>,
    Path(session_id): Path<SessionId>,
) -> Result<Json<Vec<sessions::RaceInfo>>, Error> {
    let races = sessions::list_races(&state.db, &session_id).await?;
    Ok(Json(races))
}

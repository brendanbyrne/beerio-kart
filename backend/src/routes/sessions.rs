use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::error::AppError;
use crate::middleware::auth::AuthUser;
use crate::services::sessions;

#[derive(Deserialize)]
pub struct CreateSessionRequest {
    pub ruleset: String,
}

/// POST /sessions — create a new session. Returns full session detail.
pub async fn create_session(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let detail = sessions::create_session(&state.db, &user.user_id, &body.ruleset).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Serialize)]
pub struct MySessionResponse {
    pub session_id: Option<String>,
}

/// GET /sessions/mine — get the user's current active session ID.
pub async fn my_session(
    user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<MySessionResponse>, AppError> {
    let session_id = sessions::get_active_session_id(&state.db, &user.user_id).await?;
    Ok(Json(MySessionResponse { session_id }))
}

/// GET /sessions — list active sessions.
pub async fn list_sessions(
    _user: AuthUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<sessions::SessionSummary>>, AppError> {
    let summaries = sessions::list_active_sessions(&state.db).await?;
    Ok(Json(summaries))
}

/// GET /sessions/:id — full session state for polling.
pub async fn get_session(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<sessions::SessionDetail>, AppError> {
    let detail = sessions::get_session_detail(&state.db, &id).await?;
    Ok(Json(detail))
}

/// POST /sessions/:id/join — join a session.
pub async fn join_session(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sessions::join_session(&state.db, &id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /sessions/:id/leave — leave a session.
pub async fn leave_session(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    sessions::leave_session(&state.db, &id, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// POST /sessions/:id/next-track — pick the next random track.
pub async fn next_track(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<sessions::SessionRaceInfo>), AppError> {
    let race = sessions::next_track(&state.db, &id, &user.user_id).await?;
    Ok((StatusCode::CREATED, Json(race)))
}

/// POST /sessions/:id/skip-turn — re-roll the current track.
pub async fn skip_turn(
    user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<sessions::SessionRaceInfo>), AppError> {
    let race = sessions::skip_turn(&state.db, &id, &user.user_id).await?;
    Ok((StatusCode::CREATED, Json(race)))
}

/// GET /sessions/:id/races — list all races in a session.
pub async fn list_races(
    _user: AuthUser,
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<sessions::RaceInfo>>, AppError> {
    let races = sessions::list_races(&state.db, &id).await?;
    Ok(Json(races))
}

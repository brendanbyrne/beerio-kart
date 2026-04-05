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

#[derive(Serialize)]
pub struct CreateSessionResponse {
    pub id: String,
    pub created_by: String,
    pub host_id: String,
    pub ruleset: String,
    pub status: String,
    pub created_at: String,
    pub last_activity_at: String,
}

/// POST /sessions — create a new session.
pub async fn create_session(
    user: AuthUser,
    State(state): State<AppState>,
    Json(body): Json<CreateSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let session = sessions::create_session(&state.db, &user.user_id, &body.ruleset).await?;
    let response = CreateSessionResponse {
        id: session.id,
        created_by: session.created_by,
        host_id: session.host_id,
        ruleset: session.ruleset,
        status: session.status,
        created_at: session.created_at,
        last_activity_at: session.last_activity_at,
    };
    Ok((StatusCode::CREATED, Json(response)))
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

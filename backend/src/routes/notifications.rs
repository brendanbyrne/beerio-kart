use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, error::Error, extract::Json, middleware::auth::User, services::notifications,
};

/// Query string for `GET /me/notifications`.
#[derive(Deserialize)]
pub struct ListNotificationsQuery {
    /// When `true`, include already-read notifications. Defaults to `false`
    /// (unread only) — the home-screen dropdown's day-one shape.
    #[serde(default)]
    pub include_read: bool,
}

/// `GET /me/notifications` — list the authenticated user's notifications,
/// newest first. Unread-only unless `?include_read=true`.
///
/// # Errors
///
/// Propagates the errors of [`notifications::list_notifications`] — `Internal`
/// for unexpected DB failures or a corrupt stored payload.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id, include_read = query.include_read))]
pub async fn list_notifications(
    user: User,
    State(state): State<AppState>,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<Json<Vec<notifications::NotificationView>>, Error> {
    let list =
        notifications::list_notifications(&state.db, &user.user_id, query.include_read).await?;
    Ok(Json(list))
}

/// Response body for `GET /me/notifications/unread-count`.
#[derive(Serialize)]
pub struct UnreadCountResponse {
    /// Number of unread notifications for the authenticated user.
    pub count: u64,
}

/// `GET /me/notifications/unread-count` — cheap unread tally for the
/// home-screen badge poll.
///
/// # Errors
///
/// Propagates the errors of [`notifications::unread_count`] — `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn unread_count(
    user: User,
    State(state): State<AppState>,
) -> Result<Json<UnreadCountResponse>, Error> {
    let count = notifications::unread_count(&state.db, &user.user_id).await?;
    Ok(Json(UnreadCountResponse { count }))
}

/// `POST /me/notifications/read-all` — mark all of the authenticated user's
/// unread notifications as read. Returns `204 No Content`.
///
/// # Errors
///
/// Propagates the errors of [`notifications::mark_all_read`] — `Internal` for
/// unexpected DB failures.
#[tracing::instrument(skip_all, fields(user_id = %user.user_id))]
pub async fn mark_all_read(
    user: User,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, Error> {
    notifications::mark_all_read(&state.db, &user.user_id).await?;
    Ok(StatusCode::NO_CONTENT)
}

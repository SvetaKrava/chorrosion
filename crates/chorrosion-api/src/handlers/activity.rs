// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, Json};
use chorrosion_application::AppState;
use serde::Serialize;
use tracing::debug;
use utoipa::ToSchema;

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityItemResponse {
    pub id: String,
    pub name: String,
    pub state: String,
    pub progress_percent: u8,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityListResponse {
    pub items: Vec<ActivityItemResponse>,
    pub total: i64,
}

pub(crate) async fn activity_queue_snapshot(_state: &AppState) -> ActivityListResponse {
    // Placeholder until queue integration is wired from download clients.
    ActivityListResponse {
        items: vec![],
        total: 0,
    }
}

pub(crate) async fn activity_import_snapshot(_state: &AppState) -> ActivityListResponse {
    // Placeholder until import pipeline progress reporting is wired.
    ActivityListResponse {
        items: vec![],
        total: 0,
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/queue",
    responses(
        (status = 200, description = "Current download queue", body = ActivityListResponse)
    ),
    tag = "activity"
)]
pub async fn get_activity_queue(State(state): State<AppState>) -> Json<ActivityListResponse> {
    debug!(target: "api", "fetching activity queue");

    Json(activity_queue_snapshot(&state).await)
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/history",
    responses(
        (status = 200, description = "Activity history", body = ActivityListResponse)
    ),
    tag = "activity"
)]
pub async fn get_activity_history(State(_state): State<AppState>) -> Json<ActivityListResponse> {
    debug!(target: "api", "fetching activity history");

    // Placeholder until history persistence/querying is implemented.
    Json(ActivityListResponse {
        items: vec![],
        total: 0,
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/processing",
    responses(
        (status = 200, description = "Currently processing items", body = ActivityListResponse)
    ),
    tag = "activity"
)]
pub async fn get_activity_processing(State(state): State<AppState>) -> Json<ActivityListResponse> {
    debug!(target: "api", "fetching currently processing items");

    Json(activity_import_snapshot(&state).await)
}

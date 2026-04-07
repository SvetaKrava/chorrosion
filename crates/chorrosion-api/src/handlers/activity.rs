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

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use serde_json::json;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn make_test_state() -> AppState {
        use sqlx::sqlite::SqlitePoolOptions;

        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");

        AppState::new(
            AppConfig::default(),
            Arc::new(SqliteArtistRepository::new(pool.clone())),
            Arc::new(SqliteAlbumRepository::new(pool.clone())),
            Arc::new(SqliteTrackRepository::new(pool.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    fn make_router(state: AppState) -> Router {
        Router::new()
            .route("/api/v1/activity/queue", get(get_activity_queue))
            .route("/api/v1/activity/history", get(get_activity_history))
            .route("/api/v1/activity/processing", get(get_activity_processing))
            .with_state(state)
    }

    async fn assert_empty_activity_response(app: Router, path: &str) {
        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should be readable");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("body should be valid JSON");

        assert_eq!(payload, json!({ "items": [], "total": 0 }));
    }

    #[tokio::test]
    async fn get_activity_queue_returns_empty_placeholder_payload() {
        let state = make_test_state().await;
        let app = make_router(state);

        assert_empty_activity_response(app, "/api/v1/activity/queue").await;
    }

    #[tokio::test]
    async fn get_activity_history_returns_empty_placeholder_payload() {
        let state = make_test_state().await;
        let app = make_router(state);

        assert_empty_activity_response(app, "/api/v1/activity/history").await;
    }

    #[tokio::test]
    async fn get_activity_processing_returns_empty_placeholder_payload() {
        let state = make_test_state().await;
        let app = make_router(state);

        assert_empty_activity_response(app, "/api/v1/activity/processing").await;
    }
}

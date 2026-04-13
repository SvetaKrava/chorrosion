// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, http::StatusCode, Json};
use chorrosion_application::{
    AppState, DelugeClient, DownloadClient, DownloadState, NzbgetClient, QBittorrentClient,
    SabnzbdClient, TransmissionClient,
};
use chorrosion_domain::DownloadClientDefinition;
use chorrosion_infrastructure::repositories::Repository;
use futures_util::future::join_all;
use serde::Serialize;
use tracing::{debug, warn};
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

fn build_download_client(definition: &DownloadClientDefinition) -> Option<Box<dyn DownloadClient>> {
    let client_type = definition.client_type.trim().to_lowercase();
    match client_type.as_str() {
        "qbittorrent" => Some(Box::new(QBittorrentClient::new(
            definition.base_url.clone(),
            definition.username.clone(),
            definition.password_encrypted.clone(),
        ))),
        "transmission" => Some(Box::new(TransmissionClient::new(
            definition.base_url.clone(),
            definition.username.clone(),
            definition.password_encrypted.clone(),
        ))),
        "deluge" => Some(Box::new(DelugeClient::new(
            definition.base_url.clone(),
            definition.password_encrypted.clone(),
        ))),
        "sabnzbd" => Some(Box::new(SabnzbdClient::new(
            definition.base_url.clone(),
            definition.password_encrypted.clone(),
        ))),
        "nzbget" => Some(Box::new(NzbgetClient::new(
            definition.base_url.clone(),
            definition.username.clone(),
            definition.password_encrypted.clone(),
        ))),
        _ => None,
    }
}

fn state_label(state: &DownloadState) -> &'static str {
    match state {
        DownloadState::Queued => "queued",
        DownloadState::Downloading => "downloading",
        DownloadState::Paused => "paused",
        DownloadState::Completed => "completed",
        DownloadState::Error => "error",
        DownloadState::Unknown => "unknown",
    }
}

pub(crate) async fn activity_queue_snapshot(
    state: &AppState,
) -> Result<ActivityListResponse, String> {
    let definitions = state
        .download_client_definition_repository
        .list(1000, 0)
        .await
        .map_err(|e| {
            warn!(target: "api", error = ?e, "failed to list download client definitions");
            format!("failed to list download client definitions: {e}")
        })?;

    let enabled: Vec<_> = definitions.into_iter().filter(|d| d.enabled).collect();

    // Build (definition, client) pairs, skipping unsupported types.
    let pairs: Vec<_> = enabled
        .into_iter()
        .filter_map(|definition| {
            let client = build_download_client(&definition);
            if client.is_none() {
                warn!(
                    target: "api",
                    client_name = %definition.name,
                    client_type = %definition.client_type,
                    "unsupported download client type while building activity snapshot"
                );
            }
            client.map(|c| (definition, c))
        })
        .collect();

    // Poll all clients concurrently.
    let results = join_all(pairs.into_iter().map(|(definition, client)| async move {
        match client.list_downloads().await {
            Ok(downloads) => downloads
                .into_iter()
                .map(|download| ActivityItemResponse {
                    id: format!("{}:{}", definition.id, download.hash),
                    name: format!("{}: {}", definition.name, download.name),
                    state: state_label(&download.state).to_string(),
                    progress_percent: download.progress_percent,
                })
                .collect::<Vec<_>>(),
            Err(error) => {
                warn!(
                    target: "api",
                    client_name = %definition.name,
                    client_type = %definition.client_type,
                    ?error,
                    "failed to retrieve downloads for activity queue snapshot"
                );
                Vec::new()
            }
        }
    }))
    .await;

    let items: Vec<_> = results.into_iter().flatten().collect();

    Ok(ActivityListResponse {
        total: items.len() as i64,
        items,
    })
}

pub(crate) async fn activity_import_snapshot(_state: &AppState) -> ActivityListResponse {
    // Placeholder until import pipeline progress reporting is wired.
    ActivityListResponse {
        items: vec![],
        total: 0,
    }
}

pub(crate) async fn activity_history_snapshot(
    state: &AppState,
) -> Result<ActivityListResponse, String> {
    let queue = activity_queue_snapshot(state).await?;
    let items: Vec<ActivityItemResponse> = queue
        .items
        .into_iter()
        .filter(|item| item.state == state_label(&DownloadState::Completed))
        .collect();

    Ok(ActivityListResponse {
        total: items.len() as i64,
        items,
    })
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityErrorResponse {
    pub error: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/queue",
    responses(
        (status = 200, description = "Current download queue", body = ActivityListResponse),
        (status = 500, description = "Internal server error", body = ActivityErrorResponse)
    ),
    tag = "activity"
)]
pub async fn get_activity_queue(
    State(state): State<AppState>,
) -> Result<Json<ActivityListResponse>, (StatusCode, Json<ActivityErrorResponse>)> {
    debug!(target: "api", "fetching activity queue");

    activity_queue_snapshot(&state)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ActivityErrorResponse { error: e }),
            )
        })
}

#[utoipa::path(
    get,
    path = "/api/v1/activity/history",
    responses(
        (status = 200, description = "Activity history", body = ActivityListResponse),
        (status = 500, description = "Internal server error", body = ActivityErrorResponse)
    ),
    tag = "activity"
)]
pub async fn get_activity_history(
    State(state): State<AppState>,
) -> Result<Json<ActivityListResponse>, (StatusCode, Json<ActivityErrorResponse>)> {
    debug!(target: "api", "fetching activity history");

    activity_history_snapshot(&state)
        .await
        .map(Json)
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ActivityErrorResponse { error: e }),
            )
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
    };
    use chorrosion_config::AppConfig;
    use chorrosion_domain::DownloadClientDefinition;
    use chorrosion_infrastructure::repositories::Repository;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use serde_json::json;
    use std::sync::Arc;
    use tower::util::ServiceExt;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    /// Build a test `AppState` with basic auth configured so that requests can
    /// be authenticated without touching the global API key store.
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

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());

        AppState::new(
            config,
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

    /// Issue a GET through the real router (including `auth_middleware`) with
    /// `Authorization: Basic user:pass` and assert the placeholder response.
    async fn assert_empty_activity_response(state: AppState, path: &str) {
        let app = crate::router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri(path)
                    // "user:pass" base64-encoded → dXNlcjpwYXNz
                    .header("Authorization", "Basic dXNlcjpwYXNz")
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
        assert_empty_activity_response(state, "/api/v1/activity/queue").await;
    }

    #[tokio::test]
    async fn get_activity_history_returns_empty_placeholder_payload() {
        let state = make_test_state().await;
        assert_empty_activity_response(state, "/api/v1/activity/history").await;
    }

    #[tokio::test]
    async fn get_activity_history_returns_only_completed_items() {
        let state = make_test_state().await;
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {
                        "hash": "completed1",
                        "name": "Album Done",
                        "progress": 1.0,
                        "state": "uploading",
                        "category": "music"
                    },
                    {
                        "hash": "active1",
                        "name": "Album Active",
                        "progress": 0.4,
                        "state": "downloading",
                        "category": "music"
                    }
                ]"#,
            ))
            .mount(&server)
            .await;

        state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "qbit-main",
                "qbittorrent",
                server.uri(),
            ))
            .await
            .expect("create download client definition");

        let app = crate::router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity/history")
                    .header("Authorization", "Basic dXNlcjpwYXNz")
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

        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["state"], "completed");
        assert_eq!(payload["items"][0]["name"], "qbit-main: Album Done");
    }

    #[tokio::test]
    async fn get_activity_processing_returns_empty_placeholder_payload() {
        let state = make_test_state().await;
        assert_empty_activity_response(state, "/api/v1/activity/processing").await;
    }

    #[tokio::test]
    async fn get_activity_queue_returns_download_status_items() {
        let state = make_test_state().await;
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[
                    {
                        "hash": "abc123",
                        "name": "Album FLAC",
                        "progress": 0.64,
                        "state": "downloading",
                        "category": "music"
                    }
                ]"#,
            ))
            .mount(&server)
            .await;

        let created = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "qbit-main",
                "qbittorrent",
                server.uri(),
            ))
            .await
            .expect("create download client definition");
        assert_eq!(created.name, "qbit-main");

        let app = crate::router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity/queue")
                    .header("Authorization", "Basic dXNlcjpwYXNz")
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

        assert_eq!(payload["total"], 1);
        let id = payload["items"][0]["id"]
            .as_str()
            .expect("id should be a string");
        assert!(
            id.ends_with(":abc123"),
            "id should end with download hash, got: {id}"
        );
        // The prefix is the definition UUID (stable, immutable).
        let prefix = id.strip_suffix(":abc123").unwrap();
        assert!(
            uuid::Uuid::parse_str(prefix).is_ok(),
            "id prefix should be a valid UUID, got: {prefix}"
        );
        assert_eq!(payload["items"][0]["name"], "qbit-main: Album FLAC");
        assert_eq!(payload["items"][0]["state"], "downloading");
        assert_eq!(payload["items"][0]["progress_percent"], 64);
    }

    #[tokio::test]
    async fn activity_endpoints_require_authentication() {
        let state = make_test_state().await;
        let app = crate::router(state);

        // Request without any credentials must be rejected.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity/queue")
                    .body(Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    /// One supported client returns a download item while a second definition
    /// uses an unsupported client type. The endpoint must still return the
    /// successful client's items and report the correct total.
    #[tokio::test]
    async fn get_activity_queue_skips_unsupported_client_and_returns_others() {
        let state = make_test_state().await;
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{
                    "hash": "def456",
                    "name": "Resilience Album",
                    "progress": 1.0,
                    "state": "completed",
                    "category": "music"
                }]"#,
            ))
            .mount(&server)
            .await;

        // Supported client.
        state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "qbit-ok",
                "qbittorrent",
                server.uri(),
            ))
            .await
            .expect("create qbittorrent definition");

        // Unsupported client type – should be silently skipped.
        state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "unknown-client",
                "not_a_real_client",
                "http://localhost:1",
            ))
            .await
            .expect("create unsupported definition");

        let app = crate::router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity/queue")
                    .header("Authorization", "Basic dXNlcjpwYXNz")
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

        // Only the qBittorrent item should appear; the unsupported client is skipped.
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["name"], "qbit-ok: Resilience Album");
        assert_eq!(payload["items"][0]["state"], "completed");
        assert_eq!(payload["items"][0]["progress_percent"], 100);
    }

    /// When a supported client returns an HTTP error (500), the endpoint should
    /// still succeed and return items from healthy clients.
    #[tokio::test]
    async fn get_activity_queue_skips_failing_client_and_returns_others() {
        let state = make_test_state().await;

        // Healthy client.
        let healthy_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"[{
                    "hash": "aaa111",
                    "name": "Good Album",
                    "progress": 0.50,
                    "state": "downloading",
                    "category": "music"
                }]"#,
            ))
            .mount(&healthy_server)
            .await;

        state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "qbit-healthy",
                "qbittorrent",
                healthy_server.uri(),
            ))
            .await
            .expect("create healthy definition");

        // Failing client – returns 500.
        let failing_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v2/torrents/info"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&failing_server)
            .await;

        state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "qbit-broken",
                "qbittorrent",
                failing_server.uri(),
            ))
            .await
            .expect("create failing definition");

        let app = crate::router(state);
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/activity/queue")
                    .header("Authorization", "Basic dXNlcjpwYXNz")
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

        // Only the healthy client's item should appear.
        assert_eq!(payload["total"], 1);
        assert_eq!(payload["items"][0]["name"], "qbit-healthy: Good Album");
        assert_eq!(payload["items"][0]["state"], "downloading");
        assert_eq!(payload["items"][0]["progress_percent"], 50);
    }
}

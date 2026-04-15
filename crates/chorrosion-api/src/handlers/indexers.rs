// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::{AppState, IndexerCapabilities, IndexerProtocol};
use chorrosion_domain::IndexerDefinition;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListIndexersQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexerResponse {
    pub id: String,
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub enabled: bool,
    pub has_api_key: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListIndexersResponse {
    pub items: Vec<IndexerResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<IndexerDefinition> for IndexerResponse {
    fn from(value: IndexerDefinition) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            base_url: value.base_url,
            protocol: value.protocol,
            enabled: value.enabled,
            has_api_key: value
                .api_key
                .as_ref()
                .is_some_and(|key| !key.trim().is_empty()),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateIndexerRequest {
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub api_key: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateIndexerRequest {
    pub name: Option<String>,
    pub base_url: Option<String>,
    pub protocol: Option<String>,
    pub api_key: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct TestIndexerRequest {
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub api_key: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TestIndexerResponse {
    pub success: bool,
    pub message: String,
    pub protocol: String,
    pub capabilities: IndexerCapabilitiesResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerCapabilitiesResponse {
    pub supports_search: bool,
    pub supports_rss: bool,
    pub supports_capabilities_detection: bool,
    pub supports_categories: bool,
    pub supported_categories: Vec<String>,
}

impl From<IndexerCapabilities> for IndexerCapabilitiesResponse {
    fn from(value: IndexerCapabilities) -> Self {
        Self {
            supports_search: value.supports_search,
            supports_rss: value.supports_rss,
            supports_capabilities_detection: value.supports_capabilities_detection,
            supports_categories: value.supports_categories,
            supported_categories: value.supported_categories,
        }
    }
}

fn default_true() -> bool {
    true
}

fn validate_name(name: &str) -> Result<(), (StatusCode, Json<IndexerErrorResponse>)> {
    if name.trim().is_empty() {
        Err((
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "name cannot be empty".to_string(),
            }),
        ))
    } else {
        Ok(())
    }
}

fn validate_base_url(base_url: &str) -> Result<(), (StatusCode, Json<IndexerErrorResponse>)> {
    if is_valid_base_url(base_url) {
        Ok(())
    } else {
        Err((
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "base_url must be a valid http or https URL with a host".to_string(),
            }),
        ))
    }
}

fn parse_protocol(
    protocol: &str,
) -> Result<IndexerProtocol, (StatusCode, Json<IndexerErrorResponse>)> {
    match protocol.parse() {
        Ok(value) => Ok(value),
        Err(error) => Err((
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse { error }),
        )),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/indexers",
    params(ListIndexersQuery),
    responses(
        (status = 200, description = "List indexers", body = ListIndexersResponse),
        (status = 400, description = "Invalid request", body = IndexerErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn list_indexers(
    State(state): State<AppState>,
    Query(query): Query<ListIndexersQuery>,
) -> Result<Json<ListIndexersResponse>, (StatusCode, Json<IndexerErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }

    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    let all = state
        .indexer_definition_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexerErrorResponse {
                    error: format!("failed to list indexers: {error}"),
                }),
            )
        })?;

    let total = all.len() as i64;
    let offset = usize::try_from(query.offset).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "offset out of valid range".to_string(),
            }),
        )
    })?;
    let limit = usize::try_from(query.limit).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "limit out of valid range".to_string(),
            }),
        )
    })?;

    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(IndexerResponse::from)
        .collect();

    Ok(Json(ListIndexersResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/indexers/{id}",
    params(("id" = String, Path, description = "Indexer ID")),
    responses(
        (status = 200, description = "Indexer found", body = IndexerResponse),
        (status = 404, description = "Indexer not found", body = IndexerErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn get_indexer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.indexer_definition_repository.get_by_id(&id).await {
        Ok(Some(indexer)) => (StatusCode::OK, Json(IndexerResponse::from(indexer))).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(IndexerErrorResponse {
                error: format!("Indexer {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IndexerErrorResponse {
                error: format!("failed to fetch indexer: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/indexers",
    request_body = CreateIndexerRequest,
    responses(
        (status = 201, description = "Indexer created", body = IndexerResponse),
        (status = 400, description = "Invalid request", body = IndexerErrorResponse),
        (status = 409, description = "Duplicate name", body = IndexerErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn create_indexer(
    State(state): State<AppState>,
    Json(request): Json<CreateIndexerRequest>,
) -> impl IntoResponse {
    if let Err(error) = validate_name(&request.name) {
        return error.into_response();
    }
    if let Err(error) = validate_base_url(&request.base_url) {
        return error.into_response();
    }

    let protocol = match parse_protocol(&request.protocol) {
        Ok(protocol) => protocol,
        Err(error) => return error.into_response(),
    };

    match state
        .indexer_definition_repository
        .get_by_name(request.name.trim())
        .await
    {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(IndexerErrorResponse {
                    error: format!("Indexer '{}' already exists", request.name.trim()),
                }),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexerErrorResponse {
                    error: format!("failed to validate indexer name uniqueness: {error}"),
                }),
            )
                .into_response();
        }
    }

    let mut indexer = IndexerDefinition::new(
        request.name.trim(),
        request.base_url.trim(),
        protocol.as_str(),
    );
    let normalized_api_key = request.api_key.as_ref().and_then(|key| {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });
    indexer.api_key = normalized_api_key;
    indexer.enabled = request.enabled;

    match state.indexer_definition_repository.create(indexer).await {
        Ok(created) => (StatusCode::CREATED, Json(IndexerResponse::from(created))).into_response(),
        Err(error) => {
            if let Some(sqlx::Error::Database(db_err)) = error.downcast_ref::<sqlx::Error>() {
                if db_err.is_unique_violation() {
                    return (
                        StatusCode::CONFLICT,
                        Json(IndexerErrorResponse {
                            error: format!("Indexer '{}' already exists", request.name.trim()),
                        }),
                    )
                        .into_response();
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexerErrorResponse {
                    error: format!("failed to create indexer: {error}"),
                }),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/indexers/{id}",
    params(("id" = String, Path, description = "Indexer ID")),
    request_body = UpdateIndexerRequest,
    responses(
        (status = 200, description = "Indexer updated", body = IndexerResponse),
        (status = 400, description = "Invalid request", body = IndexerErrorResponse),
        (status = 404, description = "Indexer not found", body = IndexerErrorResponse),
        (status = 409, description = "Duplicate name", body = IndexerErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_indexer(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateIndexerRequest>,
) -> impl IntoResponse {
    let mut indexer = match state.indexer_definition_repository.get_by_id(&id).await {
        Ok(Some(indexer)) => indexer,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(IndexerErrorResponse {
                    error: format!("Indexer {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexerErrorResponse {
                    error: format!("failed to fetch indexer: {error}"),
                }),
            )
                .into_response();
        }
    };

    if let Some(name) = request.name {
        if let Err(error) = validate_name(&name) {
            return error.into_response();
        }

        match state
            .indexer_definition_repository
            .get_by_name(name.trim())
            .await
        {
            Ok(Some(existing)) if existing.id != indexer.id => {
                return (
                    StatusCode::CONFLICT,
                    Json(IndexerErrorResponse {
                        error: format!("Indexer '{}' already exists", name.trim()),
                    }),
                )
                    .into_response();
            }
            Ok(_) => {
                indexer.name = name.trim().to_string();
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(IndexerErrorResponse {
                        error: format!("failed to validate indexer name uniqueness: {error}"),
                    }),
                )
                    .into_response();
            }
        }
    }

    if let Some(base_url) = request.base_url {
        if let Err(error) = validate_base_url(&base_url) {
            return error.into_response();
        }
        indexer.base_url = base_url.trim().to_string();
    }

    if let Some(protocol) = request.protocol {
        let parsed = match parse_protocol(&protocol) {
            Ok(protocol) => protocol,
            Err(error) => return error.into_response(),
        };
        indexer.protocol = parsed.as_str().to_string();
    }

    if let Some(api_key) = request.api_key {
        let trimmed = api_key.trim();
        indexer.api_key = if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        };
    }

    if let Some(enabled) = request.enabled {
        indexer.enabled = enabled;
    }

    indexer.updated_at = Utc::now();

    match state.indexer_definition_repository.update(indexer).await {
        Ok(updated) => (StatusCode::OK, Json(IndexerResponse::from(updated))).into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IndexerErrorResponse {
                error: format!("failed to update indexer: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/indexers/{id}",
    params(("id" = String, Path, description = "Indexer ID")),
    responses(
        (status = 204, description = "Indexer deleted"),
        (status = 404, description = "Indexer not found", body = IndexerErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn delete_indexer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state.indexer_definition_repository.get_by_id(&id).await {
        Ok(Some(_)) => {
            match state.indexer_definition_repository.delete(&id).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Recheck existence to distinguish concurrent deletion (404)
                    // from a transient delete failure (500).
                    match state.indexer_definition_repository.get_by_id(&id).await {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(IndexerErrorResponse {
                                error: format!("Indexer {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) | Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(IndexerErrorResponse {
                                error: format!("failed to delete indexer: {delete_error}"),
                            }),
                        )
                            .into_response(),
                    }
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(IndexerErrorResponse {
                error: format!("Indexer {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IndexerErrorResponse {
                error: format!("failed to fetch indexer before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerTestErrorResponse {
    pub error: String,
}

/// Test indexer configuration and return detected capabilities.
#[utoipa::path(
    post,
    path = "/api/v1/indexers/test",
    request_body = TestIndexerRequest,
    responses(
        (status = 200, description = "Indexer test completed", body = TestIndexerResponse),
        (status = 400, description = "Invalid request", body = IndexerTestErrorResponse)
    ),
    tag = "indexers"
)]
pub async fn test_indexer_endpoint(Json(request): Json<TestIndexerRequest>) -> impl IntoResponse {
    if request.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerTestErrorResponse {
                error: "Indexer name is required".to_string(),
            }),
        )
            .into_response();
    }

    if !is_valid_base_url(&request.base_url) {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerTestErrorResponse {
                error: "Indexer base_url must be a valid http or https URL with a host".to_string(),
            }),
        )
            .into_response();
    }

    let protocol: IndexerProtocol = match request.protocol.parse() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(IndexerTestErrorResponse { error: e }),
            )
                .into_response()
        }
    };
    let capabilities = capabilities_for_protocol(&protocol);

    (
        StatusCode::OK,
        Json(TestIndexerResponse {
            success: true,
            message: format!(
                "Indexer '{}' configuration validated for protocol {}",
                request.name,
                protocol.as_str()
            ),
            protocol: protocol.as_str().to_string(),
            capabilities: capabilities.into(),
        }),
    )
        .into_response()
}

fn is_valid_base_url(base_url: &str) -> bool {
    match url::Url::parse(base_url.trim()) {
        Ok(parsed) => matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some(),
        Err(_) => false,
    }
}

fn capabilities_for_protocol(protocol: &IndexerProtocol) -> IndexerCapabilities {
    match protocol {
        IndexerProtocol::Newznab | IndexerProtocol::Torznab => IndexerCapabilities {
            supports_search: true,
            supports_rss: true,
            supports_capabilities_detection: true,
            supports_categories: true,
            supported_categories: vec![
                "music".to_string(),
                "audio/flac".to_string(),
                "audio/mp3".to_string(),
            ],
        },
        IndexerProtocol::Gazelle => IndexerCapabilities {
            supports_search: true,
            supports_rss: false,
            supports_capabilities_detection: true,
            supports_categories: true,
            supported_categories: vec!["music".to_string(), "torrent".to_string()],
        },
        IndexerProtocol::Custom => IndexerCapabilities {
            supports_search: false,
            supports_rss: false,
            supports_capabilities_detection: false,
            supports_categories: false,
            supported_categories: vec![],
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::State;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use std::sync::Arc;

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

    #[tokio::test]
    async fn create_indexer_returns_created() {
        let state = make_test_state().await;
        let response = create_indexer(
            State(state.clone()),
            Json(CreateIndexerRequest {
                name: "Alpha".to_string(),
                base_url: "https://indexer.example".to_string(),
                protocol: "newznab".to_string(),
                api_key: Some("secret".to_string()),
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);

        let Json(list) = list_indexers(
            State(state),
            Query(ListIndexersQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list indexers");
        assert_eq!(list.total, 1);
        assert_eq!(list.items[0].name, "Alpha");
        assert!(list.items[0].has_api_key);
    }

    #[tokio::test]
    async fn create_indexer_rejects_invalid_protocol() {
        let state = make_test_state().await;
        let response = create_indexer(
            State(state),
            Json(CreateIndexerRequest {
                name: "Alpha".to_string(),
                base_url: "https://indexer.example".to_string(),
                protocol: "badproto".to_string(),
                api_key: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn update_indexer_changes_values() {
        let state = make_test_state().await;
        let created = state
            .indexer_definition_repository
            .create(IndexerDefinition::new(
                "Initial",
                "https://one.example",
                "newznab",
            ))
            .await
            .expect("create indexer");

        let response = update_indexer(
            State(state.clone()),
            Path(created.id.to_string()),
            Json(UpdateIndexerRequest {
                name: Some("Renamed".to_string()),
                base_url: Some("https://two.example".to_string()),
                protocol: Some("torznab".to_string()),
                api_key: Some("token".to_string()),
                enabled: Some(false),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let fetched = state
            .indexer_definition_repository
            .get_by_id(&created.id.to_string())
            .await
            .expect("get indexer")
            .expect("indexer exists");
        assert_eq!(fetched.name, "Renamed");
        assert_eq!(fetched.base_url, "https://two.example");
        assert_eq!(fetched.protocol, "torznab");
        assert!(!fetched.enabled);
        assert_eq!(fetched.api_key.as_deref(), Some("token"));
    }

    #[tokio::test]
    async fn delete_indexer_returns_not_found_after_delete() {
        let state = make_test_state().await;
        let created = state
            .indexer_definition_repository
            .create(IndexerDefinition::new(
                "Initial",
                "https://one.example",
                "newznab",
            ))
            .await
            .expect("create indexer");

        let delete_response = delete_indexer(State(state.clone()), Path(created.id.to_string()))
            .await
            .into_response();
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let get_response = get_indexer(State(state), Path(created.id.to_string()))
            .await
            .into_response();
        assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_indexer_endpoint_rejects_bad_url() {
        let response = test_indexer_endpoint(Json(TestIndexerRequest {
            name: "Index".to_string(),
            base_url: "not-a-url".to_string(),
            protocol: "newznab".to_string(),
            api_key: None,
        }))
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_indexer_normalizes_whitespace_api_key_to_none() {
        let state = make_test_state().await;
        let response = create_indexer(
            State(state.clone()),
            Json(CreateIndexerRequest {
                name: "Beta".to_string(),
                base_url: "https://beta.example".to_string(),
                protocol: "torznab".to_string(),
                api_key: Some("   ".to_string()),
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);

        let stored = state
            .indexer_definition_repository
            .get_by_name("Beta")
            .await
            .expect("get_by_name")
            .expect("indexer exists");
        assert!(stored.api_key.is_none());

        let Json(list) = list_indexers(
            State(state),
            Query(ListIndexersQuery {
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list indexers");
        assert!(!list.items[0].has_api_key);
    }

    #[tokio::test]
    async fn create_indexer_returns_conflict_for_duplicate_name() {
        let state = make_test_state().await;
        let first = create_indexer(
            State(state.clone()),
            Json(CreateIndexerRequest {
                name: "Duplicate".to_string(),
                base_url: "https://first.example".to_string(),
                protocol: "newznab".to_string(),
                api_key: None,
                enabled: true,
            }),
        )
        .await
        .into_response();
        assert_eq!(first.status(), StatusCode::CREATED);

        let second = create_indexer(
            State(state.clone()),
            Json(CreateIndexerRequest {
                name: "Duplicate".to_string(),
                base_url: "https://second.example".to_string(),
                protocol: "torznab".to_string(),
                api_key: None,
                enabled: true,
            }),
        )
        .await
        .into_response();
        assert_eq!(second.status(), StatusCode::CONFLICT);
    }
}

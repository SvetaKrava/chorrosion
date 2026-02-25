use axum::{http::StatusCode, response::IntoResponse, Json};
use chorrosion_application::{IndexerCapabilities, IndexerProtocol, IndexerTestResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

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
                error: "Indexer base_url must start with http:// or https://".to_string(),
            }),
        )
            .into_response();
    }

    let protocol = IndexerProtocol::from_str(&request.protocol);
    let capabilities = capabilities_for_protocol(&protocol);

    let test_result = IndexerTestResult {
        success: true,
        message: format!(
            "Indexer '{}' configuration validated for protocol {}",
            request.name,
            protocol.as_str()
        ),
        capabilities: Some(capabilities.clone()),
    };

    (
        StatusCode::OK,
        Json(TestIndexerResponse {
            success: test_result.success,
            message: test_result.message,
            protocol: protocol.as_str().to_string(),
            capabilities: capabilities.into(),
        }),
    )
        .into_response()
}

fn is_valid_base_url(base_url: &str) -> bool {
    let value = base_url.trim().to_lowercase();
    value.starts_with("http://") || value.starts_with("https://")
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

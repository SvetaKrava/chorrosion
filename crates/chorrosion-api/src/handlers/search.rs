// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use chorrosion_application::{
    manual_search, AppState, AudioQuality, IndexerConfig, IndexerError, IndexerProtocol,
    ManualSearchRequest, NewznabClient, ReleaseFilterOptions, TorznabClient,
};
use chorrosion_infrastructure::repositories::Repository;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Deserialize, ToSchema)]
pub struct ManualSearchApiRequest {
    pub indexer_id: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub query: Option<String>,
    #[serde(default)]
    pub preferred_qualities: Vec<String>,
    pub min_bitrate_kbps: Option<u32>,
    #[serde(default)]
    pub preferred_release_groups: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ManualSearchApiResponse {
    pub items: Vec<ManualSearchResultItem>,
    pub total: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ManualSearchResultItem {
    pub title: String,
    pub guid: Option<String>,
    pub download_url: Option<String>,
    pub published_at: Option<String>,
    pub size_bytes: Option<u64>,
    pub seeders: Option<u32>,
    pub leechers: Option<u32>,
    pub parsed_artist: Option<String>,
    pub parsed_album: Option<String>,
    pub parsed_quality: String,
    pub parsed_bitrate_kbps: Option<u32>,
    pub parsed_release_group: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SearchErrorResponse {
    pub error: String,
}

#[utoipa::path(
    post,
    path = "/api/v1/search/manual",
    request_body = ManualSearchApiRequest,
    responses(
        (status = 200, description = "Manual search results", body = ManualSearchApiResponse),
        (status = 400, description = "Invalid request", body = SearchErrorResponse),
        (status = 404, description = "Indexer not found", body = SearchErrorResponse),
        (status = 500, description = "Internal server error", body = SearchErrorResponse),
        (status = 502, description = "Indexer search failed", body = SearchErrorResponse)
    ),
    tag = "search"
)]
pub async fn manual_search_endpoint(
    State(state): State<AppState>,
    Json(request): Json<ManualSearchApiRequest>,
) -> impl IntoResponse {
    if request.indexer_id.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(SearchErrorResponse {
                error: "indexer_id is required".to_string(),
            }),
        )
            .into_response();
    }

    let artist = request
        .artist
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let album = request
        .album
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);
    let query = request
        .query
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned);

    if artist.is_none() && album.is_none() && query.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(SearchErrorResponse {
                error: "at least one of artist, album, or query must be provided".to_string(),
            }),
        )
            .into_response();
    }

    let preferred_qualities = match parse_preferred_qualities(&request.preferred_qualities) {
        Ok(values) => values,
        Err(error) => {
            return (StatusCode::BAD_REQUEST, Json(SearchErrorResponse { error })).into_response();
        }
    };

    let options = ReleaseFilterOptions {
        preferred_qualities,
        min_bitrate_kbps: request.min_bitrate_kbps,
        preferred_release_groups: request.preferred_release_groups,
    };

    let manual_request = ManualSearchRequest {
        artist,
        album,
        query,
    };

    let indexer = match state
        .indexer_definition_repository
        .get_by_id(request.indexer_id.clone())
        .await
    {
        Ok(Some(indexer)) => indexer,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(SearchErrorResponse {
                    error: format!("Indexer {} not found", request.indexer_id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SearchErrorResponse {
                    error: format!("failed to fetch indexer: {error}"),
                }),
            )
                .into_response();
        }
    };

    let protocol = match indexer.protocol.parse::<IndexerProtocol>() {
        Ok(protocol) => protocol,
        Err(error) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SearchErrorResponse {
                    error: format!("invalid indexer protocol: {error}"),
                }),
            )
                .into_response();
        }
    };

    let config = IndexerConfig {
        name: indexer.name,
        base_url: indexer.base_url,
        protocol: protocol.clone(),
        api_key: indexer.api_key,
        enabled: indexer.enabled,
    };

    let ranked_results = match protocol {
        IndexerProtocol::Newznab => {
            let client = NewznabClient::new(config);
            let result = manual_search(&client, &manual_request, &options).await;
            result
        }
        IndexerProtocol::Torznab => {
            let client = TorznabClient::new(config);
            let result = manual_search(&client, &manual_request, &options).await;
            result
        }
        IndexerProtocol::Gazelle | IndexerProtocol::Custom => {
            return (
                StatusCode::BAD_REQUEST,
                Json(SearchErrorResponse {
                    error: "interactive manual search currently supports newznab/torznab indexers"
                        .to_string(),
                }),
            )
                .into_response();
        }
    };

    match ranked_results {
        Ok(results) => {
            let items = results
                .into_iter()
                .map(|result| ManualSearchResultItem {
                    title: result.search_result.title,
                    guid: result.search_result.guid,
                    download_url: result.search_result.download_url,
                    published_at: result.search_result.published_at,
                    size_bytes: result.search_result.size_bytes,
                    seeders: result.search_result.seeders,
                    leechers: result.search_result.leechers,
                    parsed_artist: result.parsed.artist,
                    parsed_album: result.parsed.album,
                    parsed_quality: result.parsed.quality.as_str().to_string(),
                    parsed_bitrate_kbps: result.parsed.bitrate_kbps,
                    parsed_release_group: result.parsed.release_group,
                })
                .collect::<Vec<_>>();

            (
                StatusCode::OK,
                Json(ManualSearchApiResponse {
                    total: items.len(),
                    items,
                }),
            )
                .into_response()
        }
        Err(IndexerError::Request(msg)) => (
            StatusCode::BAD_REQUEST,
            Json(SearchErrorResponse {
                error: format!("invalid search request: {msg}"),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::BAD_GATEWAY,
            Json(SearchErrorResponse {
                error: format!("indexer search failed: {error}"),
            }),
        )
            .into_response(),
    }
}

fn parse_preferred_qualities(values: &[String]) -> Result<Vec<AudioQuality>, String> {
    values
        .iter()
        .map(|value| match value.trim().to_ascii_lowercase().as_str() {
            "flac" => Ok(AudioQuality::Flac),
            "mp3" => Ok(AudioQuality::Mp3),
            "aac" => Ok(AudioQuality::Aac),
            "alac" => Ok(AudioQuality::Alac),
            "unknown" => Ok(AudioQuality::Unknown),
            other => Err(format!(
                "unsupported quality '{}'; expected one of: flac, mp3, aac, alac, unknown",
                other
            )),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_config::AppConfig;
    use chorrosion_domain::IndexerDefinition;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use chorrosion_infrastructure::ResponseCache;
    use std::sync::Arc;

    async fn make_test_state() -> AppState {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite");

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
            ResponseCache::new(100, 60),
        )
    }

    #[tokio::test]
    async fn parse_preferred_qualities_rejects_invalid_values() {
        let err = parse_preferred_qualities(&["lossless".to_string()]).expect_err("invalid");
        assert!(err.contains("unsupported quality"));
    }

    #[tokio::test]
    async fn manual_search_endpoint_returns_404_for_unknown_indexer() {
        let state = make_test_state().await;

        let response = manual_search_endpoint(
            State(state),
            Json(ManualSearchApiRequest {
                indexer_id: "00000000-0000-0000-0000-000000000000".to_string(),
                artist: Some("Boards of Canada".to_string()),
                album: Some("Music Has the Right to Children".to_string()),
                query: None,
                preferred_qualities: vec![],
                min_bitrate_kbps: None,
                preferred_release_groups: vec![],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn manual_search_endpoint_rejects_unsupported_protocol() {
        let state = make_test_state().await;

        let indexer = IndexerDefinition::new("custom-indexer", "https://example.test", "custom");
        let id = indexer.id.to_string();
        state
            .indexer_definition_repository
            .create(indexer)
            .await
            .expect("create indexer");

        let response = manual_search_endpoint(
            State(state),
            Json(ManualSearchApiRequest {
                indexer_id: id,
                artist: None,
                album: None,
                query: Some("test".to_string()),
                preferred_qualities: vec![],
                min_bitrate_kbps: None,
                preferred_release_groups: vec![],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn manual_search_endpoint_returns_400_when_no_search_criteria() {
        let state = make_test_state().await;

        // All of artist, album, query are None – empty criteria should yield 400
        let response = manual_search_endpoint(
            State(state),
            Json(ManualSearchApiRequest {
                indexer_id: "00000000-0000-0000-0000-000000000000".to_string(),
                artist: None,
                album: None,
                query: None,
                preferred_qualities: vec![],
                min_bitrate_kbps: None,
                preferred_release_groups: vec![],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn manual_search_endpoint_returns_400_when_criteria_are_only_whitespace() {
        let state = make_test_state().await;

        let response = manual_search_endpoint(
            State(state),
            Json(ManualSearchApiRequest {
                indexer_id: "00000000-0000-0000-0000-000000000000".to_string(),
                artist: Some("   ".to_string()),
                album: Some("  ".to_string()),
                query: Some("".to_string()),
                preferred_qualities: vec![],
                min_bitrate_kbps: None,
                preferred_release_groups: vec![],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

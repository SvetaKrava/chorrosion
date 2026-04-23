use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use chorrosion_application::AppState;
use serde::{Deserialize, Serialize};
use tracing::{debug, error};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct ListDuplicatesQuery {
    /// Detection method: "fingerprint" or "hash"
    #[serde(default = "default_method")]
    pub method: String,
    /// Maximum number of groups to return (1–500)
    #[serde(default = "default_limit")]
    pub limit: i64,
    /// Number of groups to skip
    #[serde(default)]
    pub offset: i64,
}

fn default_method() -> String {
    "fingerprint".to_string()
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Deserialize, ToSchema, IntoParams)]
pub struct DuplicateGroupQuery {
    /// Detection method: "fingerprint" or "hash"
    #[serde(default = "default_method")]
    pub method: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DuplicateGroupResponse {
    pub key: String,
    pub method: String,
    pub file_count: i64,
    pub first_seen_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListDuplicatesResponse {
    pub items: Vec<DuplicateGroupResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
    pub method: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DuplicateFileResponse {
    pub track_file_id: String,
    pub track_id: String,
    pub path: String,
    pub size_bytes: u64,
    pub quality: Option<String>,
    pub bitrate_kbps: Option<u32>,
    pub codec: Option<String>,
    pub fingerprint_hash: Option<String>,
    pub file_hash: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DuplicateGroupDetailResponse {
    pub key: String,
    pub method: String,
    pub files: Vec<DuplicateFileResponse>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ResolveDuplicateRequest {
    /// Action: "delete_specific"
    pub action: String,
    /// ID of the track file to delete (required for "delete_specific")
    pub track_file_id: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ResolveDuplicateResponse {
    pub message: String,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = DuplicateErrorResponse)]
pub struct ErrorResponse {
    pub error: String,
}

fn error_response(
    status: StatusCode,
    message: impl Into<String>,
) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn group_to_response(group: chorrosion_application::DuplicateGroup) -> DuplicateGroupResponse {
    let method = match group.method {
        chorrosion_application::DuplicateDetectionMethod::FingerprintHash => "fingerprint",
        chorrosion_application::DuplicateDetectionMethod::FileHash => "hash",
    };

    DuplicateGroupResponse {
        key: group.key,
        method: method.to_string(),
        file_count: group.file_count,
        first_seen_at: group.first_seen_at.to_rfc3339(),
    }
}

fn file_to_response(file: chorrosion_application::DuplicateFileDetail) -> DuplicateFileResponse {
    DuplicateFileResponse {
        track_file_id: file.track_file_id.to_string(),
        track_id: file.track_id.to_string(),
        path: file.path,
        size_bytes: file.size_bytes,
        quality: file.quality,
        bitrate_kbps: file.bitrate_kbps,
        codec: file.codec,
        fingerprint_hash: file.fingerprint_hash,
        file_hash: file.file_hash,
        created_at: file.created_at.to_rfc3339(),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/duplicates",
    params(ListDuplicatesQuery),
    responses(
        (status = 200, description = "List duplicate groups", body = ListDuplicatesResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "duplicates"
)]
pub async fn list_duplicate_groups(
    State(state): State<AppState>,
    Query(query): Query<ListDuplicatesQuery>,
) -> Result<Json<ListDuplicatesResponse>, (StatusCode, Json<ErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "limit must be between 1 and 500",
        ));
    }
    if query.offset < 0 {
        return Err(error_response(
            StatusCode::BAD_REQUEST,
            "offset must be greater than or equal to 0",
        ));
    }

    debug!(target: "api", method = %query.method, limit = query.limit, offset = query.offset, "listing duplicate groups");

    match query.method.as_str() {
        "fingerprint" => {
            let total = state
                .duplicate_repository
                .count_fingerprint_duplicate_groups()
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to count fingerprint duplicate groups");
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "failed to list duplicates")
                })?;

            let items = state
                .duplicate_repository
                .find_fingerprint_duplicate_groups(query.limit, query.offset)
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to list fingerprint duplicate groups");
                    error_response(StatusCode::INTERNAL_SERVER_ERROR, "failed to list duplicates")
                })?
                .into_iter()
                .map(group_to_response)
                .collect();

            Ok(Json(ListDuplicatesResponse {
                items,
                total,
                limit: query.limit,
                offset: query.offset,
                method: "fingerprint".to_string(),
            }))
        }
        "hash" => {
            let total = state
                .duplicate_repository
                .count_hash_duplicate_groups()
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to count hash duplicate groups");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to list duplicates",
                    )
                })?;

            let items = state
                .duplicate_repository
                .find_hash_duplicate_groups(query.limit, query.offset)
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to list hash duplicate groups");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to list duplicates",
                    )
                })?
                .into_iter()
                .map(group_to_response)
                .collect();

            Ok(Json(ListDuplicatesResponse {
                items,
                total,
                limit: query.limit,
                offset: query.offset,
                method: "hash".to_string(),
            }))
        }
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "method must be 'fingerprint' or 'hash'",
        )),
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/duplicates/{key}",
    params(
        ("key" = String, Path, description = "Duplicate group key (fingerprint hash or file hash)"),
        DuplicateGroupQuery
    ),
    responses(
        (status = 200, description = "Duplicate group detail", body = DuplicateGroupDetailResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "duplicates"
)]
pub async fn get_duplicate_group(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<DuplicateGroupQuery>,
) -> Result<Json<DuplicateGroupDetailResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", key = %key, method = %query.method, "getting duplicate group detail");

    match query.method.as_str() {
        "fingerprint" => {
            let files = state
                .duplicate_repository
                .get_files_by_fingerprint(&key)
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to get files by fingerprint");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to get duplicate group",
                    )
                })?;

            if files.is_empty() {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "duplicate group not found",
                ));
            }

            Ok(Json(DuplicateGroupDetailResponse {
                key,
                method: "fingerprint".to_string(),
                files: files.into_iter().map(file_to_response).collect(),
            }))
        }
        "hash" => {
            let files = state
                .duplicate_repository
                .get_files_by_hash(&key)
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to get files by hash");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to get duplicate group",
                    )
                })?;

            if files.is_empty() {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "duplicate group not found",
                ));
            }

            Ok(Json(DuplicateGroupDetailResponse {
                key,
                method: "hash".to_string(),
                files: files.into_iter().map(file_to_response).collect(),
            }))
        }
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "method must be 'fingerprint' or 'hash'",
        )),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/duplicates/{key}/resolve",
    params(
        ("key" = String, Path, description = "Duplicate group key"),
        DuplicateGroupQuery
    ),
    request_body = ResolveDuplicateRequest,
    responses(
        (status = 200, description = "Duplicate resolved", body = ResolveDuplicateResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Track file not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "duplicates"
)]
pub async fn resolve_duplicate_group(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(query): Query<DuplicateGroupQuery>,
    Json(payload): Json<ResolveDuplicateRequest>,
) -> Result<Json<ResolveDuplicateResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", key = %key, method = %query.method, action = %payload.action, "resolving duplicate group");

    match payload.action.as_str() {
        "delete_specific" => {
            let track_file_id_raw = payload.track_file_id.ok_or_else(|| {
                error_response(
                    StatusCode::BAD_REQUEST,
                    "track_file_id is required for delete_specific action",
                )
            })?;
            let track_file_id = Uuid::parse_str(&track_file_id_raw).map_err(|err| {
                error!(
                    target: "api",
                    track_file_id = %track_file_id_raw,
                    error = %err,
                    "invalid track_file_id format for duplicate resolution"
                );
                error_response(
                    StatusCode::BAD_REQUEST,
                    "track_file_id must be a valid UUID",
                )
            })?;

            let group_files = match query.method.as_str() {
                "fingerprint" => state
                    .duplicate_repository
                    .get_files_by_fingerprint(&key)
                    .await
                    .map_err(|err| {
                        error!(
                            target: "api",
                            error = %err,
                            "failed to get duplicate group by fingerprint for resolution"
                        );
                        error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "failed to resolve duplicate group",
                        )
                    })?,
                "hash" => state
                    .duplicate_repository
                    .get_files_by_hash(&key)
                    .await
                    .map_err(|err| {
                        error!(
                            target: "api",
                            error = %err,
                            "failed to get duplicate group by hash for resolution"
                        );
                        error_response(
                            StatusCode::INTERNAL_SERVER_ERROR,
                            "failed to resolve duplicate group",
                        )
                    })?,
                _ => {
                    return Err(error_response(
                        StatusCode::BAD_REQUEST,
                        "method must be 'fingerprint' or 'hash'",
                    ));
                }
            };

            if group_files.is_empty() {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "duplicate group not found",
                ));
            }

            let track_file_id_str = track_file_id.to_string();
            if !group_files
                .iter()
                .any(|file| file.track_file_id.to_string() == track_file_id_str)
            {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "track file not found in duplicate group",
                ));
            }

            let deleted = state
                .duplicate_repository
                .delete_track_file(&track_file_id_str)
                .await
                .map_err(|err| {
                    error!(target: "api", error = %err, "failed to delete track file");
                    error_response(
                        StatusCode::INTERNAL_SERVER_ERROR,
                        "failed to delete track file",
                    )
                })?;

            if !deleted {
                return Err(error_response(
                    StatusCode::NOT_FOUND,
                    "track file not found",
                ));
            }

            Ok(Json(ResolveDuplicateResponse {
                message: format!("track file {} deleted", track_file_id_str),
            }))
        }
        _ => Err(error_response(
            StatusCode::BAD_REQUEST,
            "action must be 'delete_specific'",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::extract::{Query, State};
    use chorrosion_config::AppConfig;
    use std::sync::Arc;

    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteDuplicateRepository, SqliteIndexerDefinitionRepository,
        SqliteMetadataProfileRepository, SqliteQualityProfileRepository,
        SqliteSmartPlaylistRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
    };

    async fn make_test_state() -> AppState {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
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
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteTagRepository::new(pool.clone())),
            Arc::new(SqliteTaggedEntityRepository::new(pool.clone())),
            Arc::new(SqliteSmartPlaylistRepository::new(pool.clone())),
            Arc::new(SqliteDuplicateRepository::new(pool.clone())),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    async fn make_test_state_with_pool() -> (AppState, sqlx::SqlitePool) {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:")
            .await
            .expect("in-memory SQLite");
        sqlx::migrate!("../../migrations")
            .run(&pool)
            .await
            .expect("migrations");

        let state = AppState::new(
            AppConfig::default(),
            Arc::new(SqliteArtistRepository::new(pool.clone())),
            Arc::new(SqliteAlbumRepository::new(pool.clone())),
            Arc::new(SqliteTrackRepository::new(pool.clone())),
            Arc::new(SqliteQualityProfileRepository::new(pool.clone())),
            Arc::new(SqliteMetadataProfileRepository::new(pool.clone())),
            Arc::new(SqliteIndexerDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
            Arc::new(SqliteTagRepository::new(pool.clone())),
            Arc::new(SqliteTaggedEntityRepository::new(pool.clone())),
            Arc::new(SqliteSmartPlaylistRepository::new(pool.clone())),
            Arc::new(SqliteDuplicateRepository::new(pool.clone())),
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        );

        (state, pool)
    }

    #[tokio::test]
    async fn list_duplicates_returns_empty_on_fresh_db() {
        let state = make_test_state().await;

        let result = list_duplicate_groups(
            State(state),
            Query(ListDuplicatesQuery {
                method: "fingerprint".to_string(),
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list duplicate groups should succeed");

        assert_eq!(result.total, 0);
        assert!(result.items.is_empty());
        assert_eq!(result.method, "fingerprint");
    }

    #[tokio::test]
    async fn list_duplicates_hash_returns_empty_on_fresh_db() {
        let state = make_test_state().await;

        let result = list_duplicate_groups(
            State(state),
            Query(ListDuplicatesQuery {
                method: "hash".to_string(),
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list hash duplicate groups should succeed");

        assert_eq!(result.total, 0);
        assert!(result.items.is_empty());
        assert_eq!(result.method, "hash");
    }

    #[tokio::test]
    async fn list_duplicates_rejects_invalid_limit() {
        let state = make_test_state().await;

        let result = list_duplicate_groups(
            State(state),
            Query(ListDuplicatesQuery {
                method: "fingerprint".to_string(),
                limit: 0,
                offset: 0,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_duplicates_rejects_invalid_method() {
        let state = make_test_state().await;

        let result = list_duplicate_groups(
            State(state),
            Query(ListDuplicatesQuery {
                method: "invalid".to_string(),
                limit: 50,
                offset: 0,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn get_duplicate_group_returns_404_when_not_found() {
        let state = make_test_state().await;

        let result = get_duplicate_group(
            State(state),
            Path("nonexistent_hash".to_string()),
            Query(DuplicateGroupQuery {
                method: "fingerprint".to_string(),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resolve_duplicate_rejects_missing_track_file_id() {
        let state = make_test_state().await;

        let result = resolve_duplicate_group(
            State(state),
            Path("some_key".to_string()),
            Query(DuplicateGroupQuery {
                method: "fingerprint".to_string(),
            }),
            Json(ResolveDuplicateRequest {
                action: "delete_specific".to_string(),
                track_file_id: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn resolve_duplicate_returns_404_for_nonexistent_file() {
        let state = make_test_state().await;

        let result = resolve_duplicate_group(
            State(state),
            Path("some_key".to_string()),
            Query(DuplicateGroupQuery {
                method: "fingerprint".to_string(),
            }),
            Json(ResolveDuplicateRequest {
                action: "delete_specific".to_string(),
                track_file_id: Some("00000000-0000-0000-0000-000000000000".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resolve_duplicate_rejects_unknown_action() {
        let state = make_test_state().await;

        let result = resolve_duplicate_group(
            State(state),
            Path("some_key".to_string()),
            Query(DuplicateGroupQuery {
                method: "fingerprint".to_string(),
            }),
            Json(ResolveDuplicateRequest {
                action: "keep_best".to_string(),
                track_file_id: None,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn list_duplicates_returns_non_empty_with_expected_method_and_ordering() {
        let (state, pool) = make_test_state_with_pool().await;

        sqlx::query(
            r#"
            INSERT INTO artists (id, name, monitored, status, created_at, updated_at)
            VALUES (?, ?, 1, 'continuing', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("Test Artist")
        .execute(&pool)
        .await
        .expect("insert artist");

        sqlx::query(
            r#"
            INSERT INTO albums (id, artist_id, title, monitored, status, created_at, updated_at)
            VALUES (?, ?, ?, 1, 'wanted', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("22222222-2222-2222-2222-222222222222")
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("Test Album")
        .execute(&pool)
        .await
        .expect("insert album");

        sqlx::query(
            r#"
            INSERT INTO tracks (id, album_id, artist_id, title, track_number, has_file, monitored, created_at, updated_at)
            VALUES (?, ?, ?, ?, 1, 1, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("33333333-3333-3333-3333-333333333333")
        .bind("22222222-2222-2222-2222-222222222222")
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("Track 1")
        .execute(&pool)
        .await
        .expect("insert track 1");

        sqlx::query(
            r#"
            INSERT INTO tracks (id, album_id, artist_id, title, track_number, has_file, monitored, created_at, updated_at)
            VALUES (?, ?, ?, ?, 2, 1, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("44444444-4444-4444-4444-444444444444")
        .bind("22222222-2222-2222-2222-222222222222")
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("Track 2")
        .execute(&pool)
        .await
        .expect("insert track 2");

        sqlx::query(
            r#"
            INSERT INTO tracks (id, album_id, artist_id, title, track_number, has_file, monitored, created_at, updated_at)
            VALUES (?, ?, ?, ?, 3, 1, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            "#,
        )
        .bind("55555555-5555-5555-5555-555555555555")
        .bind("22222222-2222-2222-2222-222222222222")
        .bind("11111111-1111-1111-1111-111111111111")
        .bind("Track 3")
        .execute(&pool)
        .await
        .expect("insert track 3");

        sqlx::query(
            r#"
            INSERT INTO track_files (id, track_id, path, size_bytes, quality, bitrate_kbps, codec, hash, fingerprint_hash, created_at)
            VALUES (?, ?, ?, 100, 'flac', 1000, 'flac', 'hash_a', 'fp_a', '2026-01-01T00:00:00Z')
            "#,
        )
        .bind("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")
        .bind("33333333-3333-3333-3333-333333333333")
        .bind("/music/a.flac")
        .execute(&pool)
        .await
        .expect("insert file a");

        sqlx::query(
            r#"
            INSERT INTO track_files (id, track_id, path, size_bytes, quality, bitrate_kbps, codec, hash, fingerprint_hash, created_at)
            VALUES (?, ?, ?, 100, 'flac', 1000, 'flac', 'hash_a', 'fp_a', '2026-01-02T00:00:00Z')
            "#,
        )
        .bind("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb")
        .bind("44444444-4444-4444-4444-444444444444")
        .bind("/music/b.flac")
        .execute(&pool)
        .await
        .expect("insert file b");

        sqlx::query(
            r#"
            INSERT INTO track_files (id, track_id, path, size_bytes, quality, bitrate_kbps, codec, hash, fingerprint_hash, created_at)
            VALUES (?, ?, ?, 100, 'flac', 1000, 'flac', 'hash_b', 'fp_b', '2026-01-03T00:00:00Z')
            "#,
        )
        .bind("cccccccc-cccc-cccc-cccc-cccccccccccc")
        .bind("55555555-5555-5555-5555-555555555555")
        .bind("/music/c.flac")
        .execute(&pool)
        .await
        .expect("insert file c");

        sqlx::query(
            r#"
            INSERT INTO track_files (id, track_id, path, size_bytes, quality, bitrate_kbps, codec, hash, fingerprint_hash, created_at)
            VALUES (?, ?, ?, 100, 'flac', 1000, 'flac', 'hash_b', 'fp_b', '2026-01-04T00:00:00Z')
            "#,
        )
        .bind("dddddddd-dddd-dddd-dddd-dddddddddddd")
        .bind("33333333-3333-3333-3333-333333333333")
        .bind("/music/d.flac")
        .execute(&pool)
        .await
        .expect("insert file d");

        sqlx::query(
            r#"
            INSERT INTO track_files (id, track_id, path, size_bytes, quality, bitrate_kbps, codec, hash, fingerprint_hash, created_at)
            VALUES (?, ?, ?, 100, 'flac', 1000, 'flac', 'hash_b', 'fp_b', '2026-01-05T00:00:00Z')
            "#,
        )
        .bind("eeeeeeee-eeee-eeee-eeee-eeeeeeeeeeee")
        .bind("44444444-4444-4444-4444-444444444444")
        .bind("/music/e.flac")
        .execute(&pool)
        .await
        .expect("insert file e");

        let result = list_duplicate_groups(
            State(state),
            Query(ListDuplicatesQuery {
                method: "hash".to_string(),
                limit: 50,
                offset: 0,
            }),
        )
        .await
        .expect("list hash duplicate groups should succeed");

        assert_eq!(result.method, "hash");
        assert_eq!(result.total, 2);
        assert_eq!(result.items.len(), 2);
        assert_eq!(result.items[0].key, "hash_b");
        assert_eq!(result.items[0].method, "hash");
        assert_eq!(result.items[0].file_count, 3);
        assert_eq!(result.items[1].key, "hash_a");
        assert_eq!(result.items[1].method, "hash");
        assert_eq!(result.items[1].file_count, 2);
    }

    #[tokio::test]
    async fn resolve_duplicate_rejects_invalid_track_file_id_format() {
        let state = make_test_state().await;

        let result = resolve_duplicate_group(
            State(state),
            Path("some_key".to_string()),
            Query(DuplicateGroupQuery {
                method: "fingerprint".to_string(),
            }),
            Json(ResolveDuplicateRequest {
                action: "delete_specific".to_string(),
                track_file_id: Some("not-a-uuid".to_string()),
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

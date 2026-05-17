// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::MetadataProfile;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use tracing::debug;
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListMetadataProfilesQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetadataProfileResponse {
    pub id: String,
    pub name: String,
    pub primary_album_types: Vec<String>,
    pub secondary_album_types: Vec<String>,
    pub release_statuses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListMetadataProfilesResponse {
    pub items: Vec<MetadataProfileResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<MetadataProfile> for MetadataProfileResponse {
    fn from(profile: MetadataProfile) -> Self {
        Self {
            id: profile.id.to_string(),
            name: profile.name,
            primary_album_types: profile.primary_album_types,
            secondary_album_types: profile.secondary_album_types,
            release_statuses: profile.release_statuses,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMetadataProfileRequest {
    pub name: String,
    pub primary_album_types: Option<Vec<String>>,
    pub secondary_album_types: Option<Vec<String>>,
    pub release_statuses: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMetadataProfileRequest {
    pub name: Option<String>,
    pub primary_album_types: Option<Vec<String>>,
    pub secondary_album_types: Option<Vec<String>>,
    pub release_statuses: Option<Vec<String>>,
}

#[derive(Debug, Serialize, ToSchema)]
#[schema(as = MetadataProfileErrorResponse)]
pub struct ErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MetadataProfileBulkRequest {
    pub action: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetadataProfileBulkItemResult {
    pub id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetadataProfileBulkResponse {
    pub results: Vec<MetadataProfileBulkItemResult>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SettingsImportQuery {
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct MetadataProfileImportItem {
    pub name: String,
    pub primary_album_types: Vec<String>,
    pub secondary_album_types: Vec<String>,
    pub release_statuses: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct MetadataProfileExportEnvelope {
    pub version: String,
    pub exported_at: String,
    pub items: Vec<MetadataProfileImportItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ImportConflictPolicy {
    Merge,
    ReplaceAll,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MetadataProfileImportRequest {
    pub version: String,
    #[serde(default = "default_import_conflict_policy")]
    pub conflict_policy: ImportConflictPolicy,
    pub items: Vec<MetadataProfileImportItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportPreviewSummary {
    pub added: usize,
    pub updated: usize,
    pub deleted: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ImportPreviewItem {
    pub name: String,
    pub action: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetadataProfileImportResponse {
    pub dry_run: bool,
    pub summary: ImportPreviewSummary,
    pub preview: Vec<ImportPreviewItem>,
    pub results: Vec<MetadataProfileBulkItemResult>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MetadataProfileImportErrorResponse {
    pub error: String,
    pub details: Vec<String>,
}

fn default_import_conflict_policy() -> ImportConflictPolicy {
    ImportConflictPolicy::Merge
}

fn validate_name(name: &str) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if name.trim().is_empty() {
        Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "name cannot be empty".to_string(),
            }),
        ))
    } else {
        Ok(())
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/metadata-profiles",
    params(ListMetadataProfilesQuery),
    responses(
        (status = 200, description = "List metadata profiles", body = ListMetadataProfilesResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn list_metadata_profiles(
    State(state): State<AppState>,
    Query(query): Query<ListMetadataProfilesQuery>,
) -> Result<Json<ListMetadataProfilesResponse>, (StatusCode, Json<ErrorResponse>)> {
    debug!(target: "api", ?query, "listing metadata profiles");

    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }
    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    let all_profiles = state
        .metadata_profile_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to list metadata profiles: {error}"),
                }),
            )
        })?;

    let total = all_profiles.len() as i64;
    let offset = match usize::try_from(query.offset) {
        Ok(offset) => offset,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "offset out of valid range".to_string(),
                }),
            ));
        }
    };
    let limit = match usize::try_from(query.limit) {
        Ok(limit) => limit,
        Err(_) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "limit out of valid range".to_string(),
                }),
            ));
        }
    };

    let items = all_profiles
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(MetadataProfileResponse::from)
        .collect();

    Ok(Json(ListMetadataProfilesResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    responses(
        (status = 200, description = "Metadata profile found", body = MetadataProfileResponse),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn get_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "fetching metadata profile");

    match state.metadata_profile_repository.get_by_id(&id).await {
        Ok(Some(profile)) => {
            (StatusCode::OK, Json(MetadataProfileResponse::from(profile))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Metadata profile {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/metadata-profiles",
    request_body = CreateMetadataProfileRequest,
    responses(
        (status = 201, description = "Metadata profile created", body = MetadataProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn create_metadata_profile(
    State(state): State<AppState>,
    Json(request): Json<CreateMetadataProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", ?request, "creating metadata profile");

    if let Err(err_response) = validate_name(&request.name) {
        return err_response.into_response();
    }

    let mut profile = MetadataProfile::new(request.name);
    if let Some(primary) = request.primary_album_types {
        profile.primary_album_types = primary;
    }
    if let Some(secondary) = request.secondary_album_types {
        profile.secondary_album_types = secondary;
    }
    if let Some(statuses) = request.release_statuses {
        profile.release_statuses = statuses;
    }

    match state.metadata_profile_repository.create(profile).await {
        Ok(created) => (
            StatusCode::CREATED,
            Json(MetadataProfileResponse::from(created)),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to create metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    request_body = UpdateMetadataProfileRequest,
    responses(
        (status = 200, description = "Metadata profile updated", body = MetadataProfileResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateMetadataProfileRequest>,
) -> impl IntoResponse {
    debug!(target: "api", %id, ?request, "updating metadata profile");

    let mut profile = match state.metadata_profile_repository.get_by_id(&id).await {
        Ok(Some(profile)) => profile,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Metadata profile {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to fetch metadata profile: {error}"),
                }),
            )
                .into_response();
        }
    };

    if let Some(name) = request.name {
        if let Err(err_response) = validate_name(&name) {
            return err_response.into_response();
        }
        profile.name = name;
    }
    if let Some(primary) = request.primary_album_types {
        profile.primary_album_types = primary;
    }
    if let Some(secondary) = request.secondary_album_types {
        profile.secondary_album_types = secondary;
    }
    if let Some(statuses) = request.release_statuses {
        profile.release_statuses = statuses;
    }
    profile.updated_at = Utc::now();

    match state.metadata_profile_repository.update(profile).await {
        Ok(updated) => {
            (StatusCode::OK, Json(MetadataProfileResponse::from(updated))).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to update metadata profile: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/metadata-profiles/{id}",
    params(("id" = String, Path, description = "Metadata profile ID")),
    responses(
        (status = 204, description = "Metadata profile deleted"),
        (status = 404, description = "Metadata profile not found", body = ErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn delete_metadata_profile(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    debug!(target: "api", %id, "deleting metadata profile");

    match state.metadata_profile_repository.get_by_id(&id).await {
        Ok(Some(_)) => {
            match state.metadata_profile_repository.delete(&id).await {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Recheck existence to distinguish concurrent deletion (404)
                    // from a transient delete failure (500).
                    match state.metadata_profile_repository.get_by_id(&id).await {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(ErrorResponse {
                                error: format!("Metadata profile {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) | Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(ErrorResponse {
                                error: format!("failed to delete metadata profile: {delete_error}"),
                            }),
                        )
                            .into_response(),
                    }
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Metadata profile {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to fetch metadata profile before delete: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/metadata-profiles/bulk",
    request_body = MetadataProfileBulkRequest,
    responses(
        (status = 200, description = "Bulk action completed", body = MetadataProfileBulkResponse),
        (status = 207, description = "Bulk action partially succeeded", body = MetadataProfileBulkResponse),
        (status = 400, description = "Invalid request", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn bulk_metadata_profiles(
    State(state): State<AppState>,
    Json(request): Json<MetadataProfileBulkRequest>,
) -> impl IntoResponse {
    if request.ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "ids must contain at least one item".to_string(),
            }),
        )
            .into_response();
    }

    if !matches!(request.action.as_str(), "delete") {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "action must be one of: delete".to_string(),
            }),
        )
            .into_response();
    }

    let mut results = Vec::with_capacity(request.ids.len());

    for id in request.ids {
        let result = match request.action.as_str() {
            "delete" => match state.metadata_profile_repository.delete(&id).await {
                Ok(_) => MetadataProfileBulkItemResult {
                    id,
                    success: true,
                    error: None,
                },
                Err(error) => MetadataProfileBulkItemResult {
                    id,
                    success: false,
                    error: Some(format!("failed to delete metadata profile: {error}")),
                },
            },
            _ => unreachable!(),
        };

        results.push(result);
    }

    let has_failures = results.iter().any(|r| !r.success);
    let status = if has_failures {
        StatusCode::MULTI_STATUS
    } else {
        StatusCode::OK
    };

    (status, Json(MetadataProfileBulkResponse { results })).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/metadata-profiles/export",
    responses(
        (status = 200, description = "Export metadata profiles", body = MetadataProfileExportEnvelope),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn export_metadata_profiles(State(state): State<AppState>) -> impl IntoResponse {
    match state.metadata_profile_repository.list(5000, 0).await {
        Ok(items) => (
            StatusCode::OK,
            Json(MetadataProfileExportEnvelope {
                version: "1".to_string(),
                exported_at: Utc::now().to_rfc3339(),
                items: items
                    .into_iter()
                    .map(|item| MetadataProfileImportItem {
                        name: item.name,
                        primary_album_types: item.primary_album_types,
                        secondary_album_types: item.secondary_album_types,
                        release_statuses: item.release_statuses,
                    })
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to export metadata profiles: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/metadata-profiles/import",
    params(SettingsImportQuery),
    request_body = MetadataProfileImportRequest,
    responses(
        (status = 200, description = "Import processed", body = MetadataProfileImportResponse),
        (status = 400, description = "Invalid request", body = MetadataProfileImportErrorResponse),
        (status = 500, description = "Internal server error", body = ErrorResponse)
    ),
    tag = "settings"
)]
pub async fn import_metadata_profiles(
    State(state): State<AppState>,
    Query(query): Query<SettingsImportQuery>,
    Json(request): Json<MetadataProfileImportRequest>,
) -> impl IntoResponse {
    if request.version.trim() != "1" {
        return (
            StatusCode::BAD_REQUEST,
            Json(MetadataProfileImportErrorResponse {
                error: "unsupported import version".to_string(),
                details: vec!["version must be '1'".to_string()],
            }),
        )
            .into_response();
    }

    let mut validation_errors = Vec::new();
    for (idx, item) in request.items.iter().enumerate() {
        if item.name.trim().is_empty() {
            validation_errors.push(format!("items[{idx}].name cannot be empty"));
        }
    }
    if !validation_errors.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(MetadataProfileImportErrorResponse {
                error: "invalid import payload".to_string(),
                details: validation_errors,
            }),
        )
            .into_response();
    }

    let existing = match state.metadata_profile_repository.list(5000, 0).await {
        Ok(existing) => existing,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to read existing metadata profiles: {error}"),
                }),
            )
                .into_response()
        }
    };
    let mut existing_by_name: HashMap<String, MetadataProfile> = HashMap::new();
    for item in existing {
        existing_by_name.insert(item.name.to_lowercase(), item);
    }

    let mut import_names = HashSet::new();
    let mut preview = Vec::new();
    let mut summary = ImportPreviewSummary {
        added: 0,
        updated: 0,
        deleted: 0,
    };
    for item in &request.items {
        let key = item.name.to_lowercase();
        import_names.insert(key.clone());
        if existing_by_name.contains_key(&key) {
            summary.updated += 1;
            preview.push(ImportPreviewItem {
                name: item.name.clone(),
                action: "update".to_string(),
            });
        } else {
            summary.added += 1;
            preview.push(ImportPreviewItem {
                name: item.name.clone(),
                action: "add".to_string(),
            });
        }
    }
    if matches!(request.conflict_policy, ImportConflictPolicy::ReplaceAll) {
        for existing_item in existing_by_name.values() {
            if !import_names.contains(&existing_item.name.to_lowercase()) {
                summary.deleted += 1;
                preview.push(ImportPreviewItem {
                    name: existing_item.name.clone(),
                    action: "delete".to_string(),
                });
            }
        }
    }

    if query.dry_run {
        return (
            StatusCode::OK,
            Json(MetadataProfileImportResponse {
                dry_run: true,
                summary,
                preview,
                results: vec![],
            }),
        )
            .into_response();
    }

    let mut results = Vec::new();
    for item in &request.items {
        let key = item.name.to_lowercase();
        if let Some(mut existing_item) = existing_by_name.get(&key).cloned() {
            existing_item.name = item.name.clone();
            existing_item.primary_album_types = item.primary_album_types.clone();
            existing_item.secondary_album_types = item.secondary_album_types.clone();
            existing_item.release_statuses = item.release_statuses.clone();
            existing_item.updated_at = Utc::now();
            let update_result = state
                .metadata_profile_repository
                .update(existing_item)
                .await;
            match update_result {
                Ok(updated) => results.push(MetadataProfileBulkItemResult {
                    id: updated.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(MetadataProfileBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to update metadata profile: {error}")),
                }),
            }
        } else {
            let mut new_item = MetadataProfile::new(item.name.clone());
            new_item.primary_album_types = item.primary_album_types.clone();
            new_item.secondary_album_types = item.secondary_album_types.clone();
            new_item.release_statuses = item.release_statuses.clone();
            let create_result = state.metadata_profile_repository.create(new_item).await;
            match create_result {
                Ok(created) => results.push(MetadataProfileBulkItemResult {
                    id: created.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(MetadataProfileBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to create metadata profile: {error}")),
                }),
            }
        }
    }

    if matches!(request.conflict_policy, ImportConflictPolicy::ReplaceAll) {
        for existing_item in existing_by_name.values() {
            if !import_names.contains(&existing_item.name.to_lowercase()) {
                let delete_result = state
                    .metadata_profile_repository
                    .delete(&existing_item.id.to_string())
                    .await;
                if let Err(error) = delete_result {
                    results.push(MetadataProfileBulkItemResult {
                        id: existing_item.id.to_string(),
                        success: false,
                        error: Some(format!("failed to delete stale metadata profile: {error}")),
                    });
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(MetadataProfileImportResponse {
            dry_run: false,
            summary,
            preview,
            results,
        }),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    mod write_handlers {
        use super::*;
        use axum::extract::{Path, Query, State};
        use axum::response::IntoResponse;
        use chorrosion_config::AppConfig;
        use chorrosion_infrastructure::sqlite_adapters::{
            SqliteAlbumRepository, SqliteArtistRepository,
            SqliteDownloadClientDefinitionRepository, SqliteIndexerDefinitionRepository,
            SqliteMetadataProfileRepository, SqliteQualityProfileRepository, SqliteTagRepository,
            SqliteTaggedEntityRepository, SqliteTrackRepository,
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
                Arc::new(SqliteDownloadClientDefinitionRepository::new(pool.clone())),
                Arc::new(SqliteTagRepository::new(pool.clone())),
                Arc::new(SqliteTaggedEntityRepository::new(pool.clone())),
                Arc::new(
                    chorrosion_infrastructure::sqlite_adapters::SqliteSmartPlaylistRepository::new(
                        pool.clone(),
                    ),
                ),
                Arc::new(
                    chorrosion_infrastructure::sqlite_adapters::SqliteDuplicateRepository::new(
                        pool.clone(),
                    ),
                ),
                chorrosion_infrastructure::ResponseCache::new(100, 60),
            )
        }

        async fn create_test_profile(state: &AppState) -> chorrosion_domain::MetadataProfile {
            state
                .metadata_profile_repository
                .create(chorrosion_domain::MetadataProfile::new("Test Profile"))
                .await
                .expect("create test metadata profile")
        }

        // --- list_metadata_profiles ---

        #[tokio::test]
        async fn list_metadata_profiles_returns_empty_when_none() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 10,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query))
                .await
                .unwrap();
            assert_eq!(result.total, 0);
            assert!(result.items.is_empty());
        }

        #[tokio::test]
        async fn list_metadata_profiles_rejects_invalid_limit() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 0,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_metadata_profiles_rejects_negative_offset() {
            let state = make_test_state().await;
            let query = ListMetadataProfilesQuery {
                limit: 10,
                offset: -1,
            };
            let result = list_metadata_profiles(State(state), Query(query)).await;
            assert!(result.is_err());
            let (status, _) = result.unwrap_err();
            assert_eq!(status, StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn list_metadata_profiles_returns_accurate_total_with_pagination() {
            let state = make_test_state().await;
            for name in ["Profile A", "Profile B", "Profile C"] {
                state
                    .metadata_profile_repository
                    .create(chorrosion_domain::MetadataProfile::new(name))
                    .await
                    .unwrap();
            }
            let query = ListMetadataProfilesQuery {
                limit: 2,
                offset: 0,
            };
            let result = list_metadata_profiles(State(state), Query(query))
                .await
                .unwrap();
            assert_eq!(result.total, 3);
            assert_eq!(result.items.len(), 2);
        }

        // --- get_metadata_profile ---

        #[tokio::test]
        async fn get_metadata_profile_returns_200_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = get_metadata_profile(State(state), Path(profile.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn get_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = get_metadata_profile(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- create_metadata_profile ---

        #[tokio::test]
        async fn create_metadata_profile_returns_201_on_success() {
            let state = make_test_state().await;
            let request = CreateMetadataProfileRequest {
                name: "New Profile".to_string(),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = create_metadata_profile(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::CREATED);
        }

        #[tokio::test]
        async fn create_metadata_profile_returns_400_for_empty_name() {
            let state = make_test_state().await;
            let request = CreateMetadataProfileRequest {
                name: "   ".to_string(),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = create_metadata_profile(State(state), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- update_metadata_profile ---

        #[tokio::test]
        async fn update_metadata_profile_returns_200_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let request = UpdateMetadataProfileRequest {
                name: Some("Updated Name".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response =
                update_metadata_profile(State(state), Path(profile.id.to_string()), Json(request))
                    .await
                    .into_response();
            assert_eq!(response.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn update_metadata_profile_bumps_updated_at_and_preserves_created_at() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let original_created_at = profile.created_at;
            let original_updated_at = profile.updated_at;

            // Ensure the clock advances before the update.
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;

            let request = UpdateMetadataProfileRequest {
                name: Some("Bumped Name".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response = update_metadata_profile(
                State(state.clone()),
                Path(profile.id.to_string()),
                Json(request),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);

            let persisted = state
                .metadata_profile_repository
                .get_by_id(&profile.id.to_string())
                .await
                .expect("get_by_id succeeds")
                .expect("profile still exists");

            assert!(
                persisted.updated_at > original_updated_at,
                "updated_at should be bumped after update"
            );
            assert_eq!(
                persisted.created_at, original_created_at,
                "created_at must not change on update"
            );
        }

        #[tokio::test]
        async fn update_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let request = UpdateMetadataProfileRequest {
                name: Some("Name".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = update_metadata_profile(State(state), Path(unknown_id), Json(request))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn update_metadata_profile_returns_400_for_empty_name() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let request = UpdateMetadataProfileRequest {
                name: Some("  ".to_string()),
                primary_album_types: None,
                secondary_album_types: None,
                release_statuses: None,
            };
            let response =
                update_metadata_profile(State(state), Path(profile.id.to_string()), Json(request))
                    .await
                    .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        // --- delete_metadata_profile ---

        #[tokio::test]
        async fn delete_metadata_profile_returns_204_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = delete_metadata_profile(State(state), Path(profile.id.to_string()))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NO_CONTENT);
        }

        #[tokio::test]
        async fn delete_metadata_profile_returns_404_for_unknown_id() {
            let state = make_test_state().await;
            let unknown_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = delete_metadata_profile(State(state), Path(unknown_id))
                .await
                .into_response();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }

        // --- bulk_metadata_profiles ---

        #[tokio::test]
        async fn bulk_metadata_profiles_delete_returns_200_on_success() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = bulk_metadata_profiles(
                State(state.clone()),
                Json(MetadataProfileBulkRequest {
                    action: "delete".to_string(),
                    ids: vec![profile.id.to_string()],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::OK);

            let fetched = state
                .metadata_profile_repository
                .get_by_id(&profile.id.to_string())
                .await
                .expect("get_by_id");
            assert!(fetched.is_none(), "profile should be deleted");
        }

        #[tokio::test]
        async fn bulk_metadata_profiles_rejects_empty_ids() {
            let state = make_test_state().await;
            let response = bulk_metadata_profiles(
                State(state),
                Json(MetadataProfileBulkRequest {
                    action: "delete".to_string(),
                    ids: vec![],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn bulk_metadata_profiles_rejects_invalid_action() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = bulk_metadata_profiles(
                State(state),
                Json(MetadataProfileBulkRequest {
                    action: "frobulate".to_string(),
                    ids: vec![profile.id.to_string()],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn bulk_metadata_profiles_rejects_enable_action() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = bulk_metadata_profiles(
                State(state),
                Json(MetadataProfileBulkRequest {
                    action: "enable".to_string(),
                    ids: vec![profile.id.to_string()],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn bulk_metadata_profiles_rejects_disable_action() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let response = bulk_metadata_profiles(
                State(state),
                Json(MetadataProfileBulkRequest {
                    action: "disable".to_string(),
                    ids: vec![profile.id.to_string()],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn bulk_metadata_profiles_returns_207_for_partial_failure() {
            let state = make_test_state().await;
            let profile = create_test_profile(&state).await;
            let missing_id = "00000000-0000-0000-0000-000000000000".to_string();
            let response = bulk_metadata_profiles(
                State(state),
                Json(MetadataProfileBulkRequest {
                    action: "delete".to_string(),
                    ids: vec![profile.id.to_string(), missing_id],
                }),
            )
            .await
            .into_response();
            assert_eq!(response.status(), StatusCode::MULTI_STATUS);
        }
    }
}

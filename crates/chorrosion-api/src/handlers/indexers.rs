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
use std::collections::{HashMap, HashSet};
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
pub struct IndexerBulkRequest {
    pub action: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerBulkItemResult {
    pub id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerBulkResponse {
    pub results: Vec<IndexerBulkItemResult>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SettingsImportQuery {
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct IndexerImportItem {
    pub name: String,
    pub base_url: String,
    pub protocol: String,
    pub api_key: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IndexerExportEnvelope {
    pub version: String,
    pub exported_at: String,
    pub items: Vec<IndexerImportItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ImportConflictPolicy {
    Merge,
    ReplaceAll,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct IndexerImportRequest {
    pub version: String,
    #[serde(default = "default_import_conflict_policy")]
    pub conflict_policy: ImportConflictPolicy,
    pub items: Vec<IndexerImportItem>,
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
pub struct IndexerImportResponse {
    pub dry_run: bool,
    pub summary: ImportPreviewSummary,
    pub preview: Vec<ImportPreviewItem>,
    pub results: Vec<IndexerBulkItemResult>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct IndexerImportErrorResponse {
    pub error: String,
    pub details: Vec<String>,
}

fn default_import_conflict_policy() -> ImportConflictPolicy {
    ImportConflictPolicy::Merge
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

#[utoipa::path(
    post,
    path = "/api/v1/settings/indexers/bulk",
    request_body = IndexerBulkRequest,
    responses(
        (status = 200, description = "Bulk action completed", body = IndexerBulkResponse),
        (status = 207, description = "Bulk action partially succeeded", body = IndexerBulkResponse),
        (status = 400, description = "Invalid request", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn bulk_indexers(
    State(state): State<AppState>,
    Json(request): Json<IndexerBulkRequest>,
) -> impl IntoResponse {
    if request.ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "ids must contain at least one item".to_string(),
            }),
        )
            .into_response();
    }

    if !matches!(request.action.as_str(), "enable" | "disable" | "delete") {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerErrorResponse {
                error: "action must be one of: enable, disable, delete".to_string(),
            }),
        )
            .into_response();
    }

    let mut results = Vec::with_capacity(request.ids.len());

    for id in request.ids {
        let result = match request.action.as_str() {
            "delete" => match state.indexer_definition_repository.delete(&id).await {
                Ok(_) => IndexerBulkItemResult {
                    id,
                    success: true,
                    error: None,
                },
                Err(error) => IndexerBulkItemResult {
                    id,
                    success: false,
                    error: Some(format!("failed to delete indexer: {error}")),
                },
            },
            "enable" | "disable" => {
                let enabled = request.action == "enable";
                let fetch_result = state.indexer_definition_repository.get_by_id(&id).await;
                match fetch_result {
                    Ok(Some(mut indexer)) => {
                        indexer.enabled = enabled;
                        indexer.updated_at = Utc::now();
                        let update_result =
                            state.indexer_definition_repository.update(indexer).await;
                        match update_result {
                            Ok(_) => IndexerBulkItemResult {
                                id,
                                success: true,
                                error: None,
                            },
                            Err(error) => IndexerBulkItemResult {
                                id,
                                success: false,
                                error: Some(format!("failed to update indexer state: {error}")),
                            },
                        }
                    }
                    Ok(None) => IndexerBulkItemResult {
                        id,
                        success: false,
                        error: Some("indexer not found".to_string()),
                    },
                    Err(error) => IndexerBulkItemResult {
                        id,
                        success: false,
                        error: Some(format!("failed to fetch indexer: {error}")),
                    },
                }
            }
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

    (status, Json(IndexerBulkResponse { results })).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/indexers/export",
    responses(
        (status = 200, description = "Export indexers", body = IndexerExportEnvelope),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn export_indexers(State(state): State<AppState>) -> impl IntoResponse {
    match state.indexer_definition_repository.list(5000, 0).await {
        Ok(items) => (
            StatusCode::OK,
            Json(IndexerExportEnvelope {
                version: "1".to_string(),
                exported_at: Utc::now().to_rfc3339(),
                items: items
                    .into_iter()
                    .map(|item| IndexerImportItem {
                        name: item.name,
                        base_url: item.base_url,
                        protocol: item.protocol,
                        api_key: None,
                        enabled: item.enabled,
                    })
                    .collect(),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(IndexerErrorResponse {
                error: format!("failed to export indexers: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/indexers/import",
    params(SettingsImportQuery),
    request_body = IndexerImportRequest,
    responses(
        (status = 200, description = "Import processed", body = IndexerImportResponse),
        (status = 400, description = "Invalid request", body = IndexerImportErrorResponse),
        (status = 500, description = "Internal server error", body = IndexerErrorResponse)
    ),
    tag = "settings"
)]
pub async fn import_indexers(
    State(state): State<AppState>,
    Query(query): Query<SettingsImportQuery>,
    Json(request): Json<IndexerImportRequest>,
) -> impl IntoResponse {
    if request.version.trim() != "1" {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerImportErrorResponse {
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
        if validate_base_url(&item.base_url).is_err() {
            validation_errors.push(format!("items[{idx}].base_url is invalid"));
        }
        if parse_protocol(&item.protocol).is_err() {
            validation_errors.push(format!("items[{idx}].protocol is invalid"));
        }
    }

    if !validation_errors.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(IndexerImportErrorResponse {
                error: "invalid import payload".to_string(),
                details: validation_errors,
            }),
        )
            .into_response();
    }

    let existing = match state.indexer_definition_repository.list(5000, 0).await {
        Ok(existing) => existing,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(IndexerErrorResponse {
                    error: format!("failed to read existing indexers: {error}"),
                }),
            )
                .into_response()
        }
    };

    let mut existing_by_name: HashMap<String, IndexerDefinition> = HashMap::new();
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
            Json(IndexerImportResponse {
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
        let protocol = match parse_protocol(&item.protocol) {
            Ok(protocol) => protocol,
            Err(_) => {
                results.push(IndexerBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some("unsupported protocol".to_string()),
                });
                continue;
            }
        };

        if let Some(mut existing_item) = existing_by_name.get(&key).cloned() {
            existing_item.name = item.name.trim().to_string();
            existing_item.base_url = item.base_url.trim().to_string();
            existing_item.protocol = protocol.as_str().to_string();
            existing_item.api_key = item.api_key.as_ref().and_then(|key| {
                let trimmed = key.trim();
                if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
            });
            existing_item.enabled = item.enabled;
            existing_item.updated_at = Utc::now();

            let update_result = state
                .indexer_definition_repository
                .update(existing_item)
                .await;
            match update_result {
                Ok(updated) => results.push(IndexerBulkItemResult {
                    id: updated.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(IndexerBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to update indexer: {error}")),
                }),
            }
        } else {
            let mut new_item =
                IndexerDefinition::new(item.name.trim(), item.base_url.trim(), protocol.as_str());
            new_item.api_key = item.api_key.as_ref().and_then(|key| {
                let trimmed = key.trim();
                if trimmed.is_empty() { None } else { Some(trimmed.to_string()) }
            });
            new_item.enabled = item.enabled;

            let create_result = state.indexer_definition_repository.create(new_item).await;
            match create_result {
                Ok(created) => results.push(IndexerBulkItemResult {
                    id: created.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(IndexerBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to create indexer: {error}")),
                }),
            }
        }
    }

    if matches!(request.conflict_policy, ImportConflictPolicy::ReplaceAll) {
        for existing_item in existing_by_name.values() {
            if !import_names.contains(&existing_item.name.to_lowercase()) {
                let delete_result = state
                    .indexer_definition_repository
                    .delete(&existing_item.id.to_string())
                    .await;
                if let Err(error) = delete_result {
                    results.push(IndexerBulkItemResult {
                        id: existing_item.id.to_string(),
                        success: false,
                        error: Some(format!("failed to delete stale indexer: {error}")),
                    });
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(IndexerImportResponse {
            dry_run: false,
            summary,
            preview,
            results,
        }),
    )
        .into_response()
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
        SqliteQualityProfileRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
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

    // --- bulk_indexers ---

    async fn create_test_indexer(state: &AppState) -> IndexerDefinition {
        state
            .indexer_definition_repository
            .create(IndexerDefinition::new(
                "Test Indexer",
                "https://indexer.example",
                "newznab",
            ))
            .await
            .expect("create test indexer")
    }

    #[tokio::test]
    async fn bulk_indexers_enables_selected_items() {
        let state = make_test_state().await;
        let mut indexer = create_test_indexer(&state).await;
        indexer.enabled = false;
        state
            .indexer_definition_repository
            .update(indexer.clone())
            .await
            .expect("disable indexer");

        let response = bulk_indexers(
            State(state.clone()),
            Json(IndexerBulkRequest {
                action: "enable".to_string(),
                ids: vec![indexer.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let updated = state
            .indexer_definition_repository
            .get_by_id(&indexer.id.to_string())
            .await
            .expect("get_by_id")
            .expect("exists");
        assert!(updated.enabled);
    }

    #[tokio::test]
    async fn bulk_indexers_disables_selected_items() {
        let state = make_test_state().await;
        let indexer = create_test_indexer(&state).await;
        assert!(indexer.enabled);

        let response = bulk_indexers(
            State(state.clone()),
            Json(IndexerBulkRequest {
                action: "disable".to_string(),
                ids: vec![indexer.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let updated = state
            .indexer_definition_repository
            .get_by_id(&indexer.id.to_string())
            .await
            .expect("get_by_id")
            .expect("exists");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn bulk_indexers_deletes_selected_items() {
        let state = make_test_state().await;
        let indexer = create_test_indexer(&state).await;

        let response = bulk_indexers(
            State(state.clone()),
            Json(IndexerBulkRequest {
                action: "delete".to_string(),
                ids: vec![indexer.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let fetched = state
            .indexer_definition_repository
            .get_by_id(&indexer.id.to_string())
            .await
            .expect("get_by_id");
        assert!(fetched.is_none(), "indexer should be deleted");
    }

    #[tokio::test]
    async fn bulk_indexers_rejects_empty_ids() {
        let state = make_test_state().await;
        let response = bulk_indexers(
            State(state),
            Json(IndexerBulkRequest {
                action: "delete".to_string(),
                ids: vec![],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn bulk_indexers_rejects_invalid_action() {
        let state = make_test_state().await;
        let indexer = create_test_indexer(&state).await;
        let response = bulk_indexers(
            State(state),
            Json(IndexerBulkRequest {
                action: "frobulate".to_string(),
                ids: vec![indexer.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn bulk_indexers_returns_207_for_partial_failure() {
        let state = make_test_state().await;
        let indexer = create_test_indexer(&state).await;
        let missing_id = "00000000-0000-0000-0000-000000000000".to_string();

        let response = bulk_indexers(
            State(state),
            Json(IndexerBulkRequest {
                action: "delete".to_string(),
                ids: vec![indexer.id.to_string(), missing_id],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    }
}

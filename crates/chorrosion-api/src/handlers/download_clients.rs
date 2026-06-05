// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chorrosion_application::AppState;
use chorrosion_domain::DownloadClientDefinition;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Deserialize, IntoParams)]
pub struct ListDownloadClientsQuery {
    #[serde(default = "default_limit")]
    pub limit: i64,
    #[serde(default)]
    pub offset: i64,
}

fn default_limit() -> i64 {
    50
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DownloadClientResponse {
    pub id: String,
    pub name: String,
    pub client_type: String,
    pub base_url: String,
    pub username: Option<String>,
    pub category: Option<String>,
    pub enabled: bool,
    pub has_password: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ListDownloadClientsResponse {
    pub items: Vec<DownloadClientResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

impl From<DownloadClientDefinition> for DownloadClientResponse {
    fn from(value: DownloadClientDefinition) -> Self {
        Self {
            id: value.id.to_string(),
            name: value.name,
            client_type: value.client_type,
            base_url: value.base_url,
            username: value.username,
            category: value.category,
            enabled: value.enabled,
            has_password: value
                .password_encrypted
                .as_ref()
                .is_some_and(|password| !password.trim().is_empty()),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateDownloadClientRequest {
    pub name: String,
    pub client_type: String,
    pub base_url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub category: Option<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateDownloadClientRequest {
    pub name: Option<String>,
    pub client_type: Option<String>,
    pub base_url: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub category: Option<String>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadClientErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadClientBulkRequest {
    pub action: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadClientBulkItemResult {
    pub id: String,
    pub success: bool,
    pub error: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadClientBulkResponse {
    pub results: Vec<DownloadClientBulkItemResult>,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct SettingsImportQuery {
    #[serde(default)]
    pub dry_run: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone)]
pub struct DownloadClientImportItem {
    pub name: String,
    pub client_type: String,
    pub base_url: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub category: Option<String>,
    pub enabled: bool,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct DownloadClientExportEnvelope {
    pub version: String,
    pub exported_at: String,
    pub items: Vec<DownloadClientImportItem>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum ImportConflictPolicy {
    Merge,
    ReplaceAll,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct DownloadClientImportRequest {
    pub version: String,
    #[serde(default = "default_import_conflict_policy")]
    pub conflict_policy: ImportConflictPolicy,
    pub items: Vec<DownloadClientImportItem>,
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
pub struct DownloadClientImportResponse {
    pub dry_run: bool,
    pub summary: ImportPreviewSummary,
    pub preview: Vec<ImportPreviewItem>,
    pub results: Vec<DownloadClientBulkItemResult>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DownloadClientImportErrorResponse {
    pub error: String,
    pub details: Vec<String>,
}

fn default_import_conflict_policy() -> ImportConflictPolicy {
    ImportConflictPolicy::Merge
}

fn default_true() -> bool {
    true
}

fn validate_name(name: &str) -> Result<(), (StatusCode, Json<DownloadClientErrorResponse>)> {
    if name.trim().is_empty() {
        Err((
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "name cannot be empty".to_string(),
            }),
        ))
    } else {
        Ok(())
    }
}

fn validate_base_url(
    base_url: &str,
) -> Result<(), (StatusCode, Json<DownloadClientErrorResponse>)> {
    match url::Url::parse(base_url.trim()) {
        Ok(parsed) if matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some() => {
            Ok(())
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "base_url must be a valid http or https URL with a host".to_string(),
            }),
        )),
    }
}

fn normalize_client_type(
    client_type: &str,
) -> Result<String, (StatusCode, Json<DownloadClientErrorResponse>)> {
    let normalized = client_type.trim().to_lowercase();
    match normalized.as_str() {
        "qbittorrent" | "transmission" | "deluge" | "sabnzbd" | "nzbget" => {
            Ok(normalized)
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error:
                    "unsupported client_type; supported values: qbittorrent, transmission, deluge, sabnzbd, nzbget"
                        .to_string(),
            }),
        )),
    }
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/download-clients",
    params(ListDownloadClientsQuery),
    responses(
        (status = 200, description = "List download clients", body = ListDownloadClientsResponse),
        (status = 400, description = "Invalid request", body = DownloadClientErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn list_download_clients(
    State(state): State<AppState>,
    Query(query): Query<ListDownloadClientsQuery>,
) -> Result<Json<ListDownloadClientsResponse>, (StatusCode, Json<DownloadClientErrorResponse>)> {
    if !(1..=500).contains(&query.limit) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "limit must be between 1 and 500".to_string(),
            }),
        ));
    }

    if query.offset < 0 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "offset must be greater than or equal to 0".to_string(),
            }),
        ));
    }

    let all = state
        .download_client_definition_repository
        .list(5000, 0)
        .await
        .map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadClientErrorResponse {
                    error: format!("failed to list download clients: {error}"),
                }),
            )
        })?;

    let total = all.len() as i64;
    let offset = usize::try_from(query.offset).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "offset out of valid range".to_string(),
            }),
        )
    })?;
    let limit = usize::try_from(query.limit).map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "limit out of valid range".to_string(),
            }),
        )
    })?;

    let items = all
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(DownloadClientResponse::from)
        .collect();

    Ok(Json(ListDownloadClientsResponse {
        items,
        total,
        limit: query.limit,
        offset: query.offset,
    }))
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/download-clients/{id}",
    params(("id" = String, Path, description = "Download client ID")),
    responses(
        (status = 200, description = "Download client found", body = DownloadClientResponse),
        (status = 404, description = "Download client not found", body = DownloadClientErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn get_download_client(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state
        .download_client_definition_repository
        .get_by_id(&id)
        .await
    {
        Ok(Some(client)) => {
            (StatusCode::OK, Json(DownloadClientResponse::from(client))).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(DownloadClientErrorResponse {
                error: format!("Download client {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DownloadClientErrorResponse {
                error: format!("failed to fetch download client: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/download-clients",
    request_body = CreateDownloadClientRequest,
    responses(
        (status = 201, description = "Download client created", body = DownloadClientResponse),
        (status = 400, description = "Invalid request", body = DownloadClientErrorResponse),
        (status = 409, description = "Duplicate name", body = DownloadClientErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn create_download_client(
    State(state): State<AppState>,
    Json(request): Json<CreateDownloadClientRequest>,
) -> impl IntoResponse {
    if let Err(error) = validate_name(&request.name) {
        return error.into_response();
    }
    if let Err(error) = validate_base_url(&request.base_url) {
        return error.into_response();
    }
    let client_type = match normalize_client_type(&request.client_type) {
        Ok(client_type) => client_type,
        Err(error) => return error.into_response(),
    };

    match state
        .download_client_definition_repository
        .get_by_name(request.name.trim())
        .await
    {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(DownloadClientErrorResponse {
                    error: format!("Download client '{}' already exists", request.name.trim()),
                }),
            )
                .into_response();
        }
        Ok(None) => {}
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadClientErrorResponse {
                    error: format!("failed to validate download client name uniqueness: {error}"),
                }),
            )
                .into_response();
        }
    }

    let mut client =
        DownloadClientDefinition::new(request.name.trim(), client_type, request.base_url.trim());
    client.username = normalize_optional(request.username);
    client.password_encrypted = normalize_optional(request.password);
    client.category = normalize_optional(request.category);
    client.enabled = request.enabled;

    match state
        .download_client_definition_repository
        .create(client)
        .await
    {
        Ok(created) => (
            StatusCode::CREATED,
            Json(DownloadClientResponse::from(created)),
        )
            .into_response(),
        Err(error) => {
            if let Some(sqlx::Error::Database(db_err)) = error.downcast_ref::<sqlx::Error>() {
                if db_err.is_unique_violation() {
                    return (
                        StatusCode::CONFLICT,
                        Json(DownloadClientErrorResponse {
                            error: format!(
                                "Download client '{}' already exists",
                                request.name.trim()
                            ),
                        }),
                    )
                        .into_response();
                }
            }
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadClientErrorResponse {
                    error: format!("failed to create download client: {error}"),
                }),
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    put,
    path = "/api/v1/settings/download-clients/{id}",
    params(("id" = String, Path, description = "Download client ID")),
    request_body = UpdateDownloadClientRequest,
    responses(
        (status = 200, description = "Download client updated", body = DownloadClientResponse),
        (status = 400, description = "Invalid request", body = DownloadClientErrorResponse),
        (status = 404, description = "Download client not found", body = DownloadClientErrorResponse),
        (status = 409, description = "Duplicate name", body = DownloadClientErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn update_download_client(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateDownloadClientRequest>,
) -> impl IntoResponse {
    let mut client = match state
        .download_client_definition_repository
        .get_by_id(&id)
        .await
    {
        Ok(Some(client)) => client,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(DownloadClientErrorResponse {
                    error: format!("Download client {} not found", id),
                }),
            )
                .into_response();
        }
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadClientErrorResponse {
                    error: format!("failed to fetch download client: {error}"),
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
            .download_client_definition_repository
            .get_by_name(name.trim())
            .await
        {
            Ok(Some(existing)) if existing.id != client.id => {
                return (
                    StatusCode::CONFLICT,
                    Json(DownloadClientErrorResponse {
                        error: format!("Download client '{}' already exists", name.trim()),
                    }),
                )
                    .into_response();
            }
            Ok(_) => {
                client.name = name.trim().to_string();
            }
            Err(error) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(DownloadClientErrorResponse {
                        error: format!(
                            "failed to validate download client name uniqueness: {error}"
                        ),
                    }),
                )
                    .into_response();
            }
        }
    }

    if let Some(client_type) = request.client_type {
        let client_type = match normalize_client_type(&client_type) {
            Ok(client_type) => client_type,
            Err(error) => return error.into_response(),
        };
        client.client_type = client_type;
    }

    if let Some(base_url) = request.base_url {
        if let Err(error) = validate_base_url(&base_url) {
            return error.into_response();
        }
        client.base_url = base_url.trim().to_string();
    }

    if let Some(username) = request.username {
        client.username = normalize_optional(Some(username));
    }

    if let Some(password) = request.password {
        client.password_encrypted = normalize_optional(Some(password));
    }

    if let Some(category) = request.category {
        client.category = normalize_optional(Some(category));
    }

    if let Some(enabled) = request.enabled {
        client.enabled = enabled;
    }

    client.updated_at = Utc::now();

    match state
        .download_client_definition_repository
        .update(client)
        .await
    {
        Ok(updated) => {
            (StatusCode::OK, Json(DownloadClientResponse::from(updated))).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DownloadClientErrorResponse {
                error: format!("failed to update download client: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    delete,
    path = "/api/v1/settings/download-clients/{id}",
    params(("id" = String, Path, description = "Download client ID")),
    responses(
        (status = 204, description = "Download client deleted"),
        (status = 404, description = "Download client not found", body = DownloadClientErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn delete_download_client(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match state
        .download_client_definition_repository
        .get_by_id(&id)
        .await
    {
        Ok(Some(_)) => {
            match state
                .download_client_definition_repository
                .delete(&id)
                .await
            {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(delete_error) => {
                    // Recheck existence to distinguish concurrent deletion (404)
                    // from a transient delete failure (500).
                    match state
                        .download_client_definition_repository
                        .get_by_id(&id)
                        .await
                    {
                        Ok(None) => (
                            StatusCode::NOT_FOUND,
                            Json(DownloadClientErrorResponse {
                                error: format!("Download client {} not found", id),
                            }),
                        )
                            .into_response(),
                        Ok(Some(_)) | Err(_) => (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(DownloadClientErrorResponse {
                                error: format!(
                                    "failed to delete download client {}: {}",
                                    id, delete_error
                                ),
                            }),
                        )
                            .into_response(),
                    }
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(DownloadClientErrorResponse {
                error: format!("Download client {} not found", id),
            }),
        )
            .into_response(),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DownloadClientErrorResponse {
                error: format!(
                    "failed to fetch download client {} before delete: {}",
                    id, error
                ),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/download-clients/bulk",
    request_body = DownloadClientBulkRequest,
    responses(
        (status = 200, description = "Bulk action completed", body = DownloadClientBulkResponse),
        (status = 207, description = "Bulk action partially succeeded", body = DownloadClientBulkResponse),
        (status = 400, description = "Invalid request", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn bulk_download_clients(
    State(state): State<AppState>,
    Json(request): Json<DownloadClientBulkRequest>,
) -> impl IntoResponse {
    if request.ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "ids must contain at least one item".to_string(),
            }),
        )
            .into_response();
    }

    if !matches!(request.action.as_str(), "enable" | "disable" | "delete") {
        return (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientErrorResponse {
                error: "action must be one of: enable, disable, delete".to_string(),
            }),
        )
            .into_response();
    }

    let mut results = Vec::with_capacity(request.ids.len());

    for id in request.ids {
        let result = match request.action.as_str() {
            "delete" => match state
                .download_client_definition_repository
                .delete(&id)
                .await
            {
                Ok(_) => DownloadClientBulkItemResult {
                    id,
                    success: true,
                    error: None,
                },
                Err(error) => DownloadClientBulkItemResult {
                    id,
                    success: false,
                    error: Some(format!("failed to delete download client: {error}")),
                },
            },
            "enable" | "disable" => {
                let enabled = request.action == "enable";
                let fetch_result = state
                    .download_client_definition_repository
                    .get_by_id(&id)
                    .await;
                match fetch_result {
                    Ok(Some(mut client)) => {
                        client.enabled = enabled;
                        client.updated_at = Utc::now();
                        let update_result = state
                            .download_client_definition_repository
                            .update(client)
                            .await;
                        match update_result {
                            Ok(_) => DownloadClientBulkItemResult {
                                id,
                                success: true,
                                error: None,
                            },
                            Err(error) => DownloadClientBulkItemResult {
                                id,
                                success: false,
                                error: Some(format!(
                                    "failed to update download client state: {error}"
                                )),
                            },
                        }
                    }
                    Ok(None) => DownloadClientBulkItemResult {
                        id,
                        success: false,
                        error: Some("download client not found".to_string()),
                    },
                    Err(error) => DownloadClientBulkItemResult {
                        id,
                        success: false,
                        error: Some(format!("failed to fetch download client: {error}")),
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

    (status, Json(DownloadClientBulkResponse { results })).into_response()
}

#[utoipa::path(
    get,
    path = "/api/v1/settings/download-clients/export",
    responses(
        (status = 200, description = "Export download clients", body = DownloadClientExportEnvelope),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn export_download_clients(State(state): State<AppState>) -> impl IntoResponse {
    match state
        .download_client_definition_repository
        .list(5000, 0)
        .await
    {
        Ok(items) => {
            let exported = DownloadClientExportEnvelope {
                version: "1".to_string(),
                exported_at: Utc::now().to_rfc3339(),
                items: items
                    .into_iter()
                    .map(|item| DownloadClientImportItem {
                        name: item.name,
                        client_type: item.client_type,
                        base_url: item.base_url,
                        username: item.username,
                        password: None,
                        category: item.category,
                        enabled: item.enabled,
                    })
                    .collect(),
            };
            (StatusCode::OK, Json(exported)).into_response()
        }
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DownloadClientErrorResponse {
                error: format!("failed to export download clients: {error}"),
            }),
        )
            .into_response(),
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/settings/download-clients/import",
    params(SettingsImportQuery),
    request_body = DownloadClientImportRequest,
    responses(
        (status = 200, description = "Import processed", body = DownloadClientImportResponse),
        (status = 400, description = "Invalid request", body = DownloadClientImportErrorResponse),
        (status = 500, description = "Internal server error", body = DownloadClientErrorResponse)
    ),
    tag = "settings"
)]
pub async fn import_download_clients(
    State(state): State<AppState>,
    Query(query): Query<SettingsImportQuery>,
    Json(request): Json<DownloadClientImportRequest>,
) -> impl IntoResponse {
    if request.version.trim() != "1" {
        return (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientImportErrorResponse {
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
        if normalize_client_type(&item.client_type).is_err() {
            validation_errors.push(format!("items[{idx}].client_type is not supported"));
        }
    }

    if !validation_errors.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(DownloadClientImportErrorResponse {
                error: "invalid import payload".to_string(),
                details: validation_errors,
            }),
        )
            .into_response();
    }

    let existing = match state
        .download_client_definition_repository
        .list(5000, 0)
        .await
    {
        Ok(existing) => existing,
        Err(error) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DownloadClientErrorResponse {
                    error: format!("failed to read existing download clients: {error}"),
                }),
            )
                .into_response()
        }
    };

    let mut existing_by_name: HashMap<String, DownloadClientDefinition> = HashMap::new();
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
            Json(DownloadClientImportResponse {
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
        let client_type = match normalize_client_type(&item.client_type) {
            Ok(client_type) => client_type,
            Err(_) => {
                results.push(DownloadClientBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some("unsupported client_type".to_string()),
                });
                continue;
            }
        };

        if let Some(mut existing_item) = existing_by_name.get(&key).cloned() {
            existing_item.name = item.name.trim().to_string();
            existing_item.client_type = client_type;
            existing_item.base_url = item.base_url.trim().to_string();
            existing_item.username = normalize_optional(item.username.clone());
            existing_item.password_encrypted = normalize_optional(item.password.clone());
            existing_item.category = normalize_optional(item.category.clone());
            existing_item.enabled = item.enabled;
            existing_item.updated_at = Utc::now();

            let update_result = state
                .download_client_definition_repository
                .update(existing_item)
                .await;
            match update_result {
                Ok(updated) => results.push(DownloadClientBulkItemResult {
                    id: updated.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(DownloadClientBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to update download client: {error}")),
                }),
            }
        } else {
            let mut new_item =
                DownloadClientDefinition::new(item.name.trim(), client_type, item.base_url.trim());
            new_item.username = normalize_optional(item.username.clone());
            new_item.password_encrypted = normalize_optional(item.password.clone());
            new_item.category = normalize_optional(item.category.clone());
            new_item.enabled = item.enabled;

            let create_result = state
                .download_client_definition_repository
                .create(new_item)
                .await;
            match create_result {
                Ok(created) => results.push(DownloadClientBulkItemResult {
                    id: created.id.to_string(),
                    success: true,
                    error: None,
                }),
                Err(error) => results.push(DownloadClientBulkItemResult {
                    id: item.name.clone(),
                    success: false,
                    error: Some(format!("failed to create download client: {error}")),
                }),
            }
        }
    }

    if matches!(request.conflict_policy, ImportConflictPolicy::ReplaceAll) {
        for existing_item in existing_by_name.values() {
            if !import_names.contains(&existing_item.name.to_lowercase()) {
                let delete_result = state
                    .download_client_definition_repository
                    .delete(&existing_item.id.to_string())
                    .await;
                if let Err(error) = delete_result {
                    results.push(DownloadClientBulkItemResult {
                        id: existing_item.id.to_string(),
                        success: false,
                        error: Some(format!("failed to delete stale download client: {error}")),
                    });
                }
            }
        }
    }

    (
        StatusCode::OK,
        Json(DownloadClientImportResponse {
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

    #[test]
    fn validate_name_rejects_empty_input() {
        assert!(validate_name("   ").is_err());
    }

    #[test]
    fn validate_base_url_accepts_trimmed_https_url() {
        assert!(validate_base_url("  https://downloads.example  ").is_ok());
    }

    #[test]
    fn validate_base_url_rejects_unsupported_scheme() {
        assert!(validate_base_url("ftp://downloads.example").is_err());
    }

    #[test]
    fn normalize_client_type_rejects_unknown_value() {
        assert!(normalize_client_type("invalid-client").is_err());
    }

    #[tokio::test]
    async fn list_download_clients_rejects_invalid_limit() {
        let state = make_test_state().await;

        let result = list_download_clients(
            State(state),
            Query(ListDownloadClientsQuery {
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
    async fn list_download_clients_rejects_negative_offset() {
        let state = make_test_state().await;

        let result = list_download_clients(
            State(state),
            Query(ListDownloadClientsQuery {
                limit: 50,
                offset: -1,
            }),
        )
        .await;

        assert!(result.is_err());
        let (status, _) = result.unwrap_err();
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_download_client_returns_created() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "qbit-main".to_string(),
                client_type: "qbittorrent".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: Some("admin".to_string()),
                password: Some("secret".to_string()),
                category: Some("music".to_string()),
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_download_client_rejects_invalid_type() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "main".to_string(),
                client_type: "not-supported".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_download_client_accepts_transmission_type() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "tx-main".to_string(),
                client_type: "transmission".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_download_client_accepts_deluge_type() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "deluge-main".to_string(),
                client_type: "deluge".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_download_client_accepts_sabnzbd_type() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "sab-main".to_string(),
                client_type: "sabnzbd".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn create_download_client_accepts_nzbget_type() {
        let state = make_test_state().await;
        let response = create_download_client(
            State(state),
            Json(CreateDownloadClientRequest {
                name: "nzbget-main".to_string(),
                client_type: "nzbget".to_string(),
                base_url: "https://downloads.example".to_string(),
                username: Some("nzbget".to_string()),
                password: Some("secret".to_string()),
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn update_download_client_changes_values() {
        let state = make_test_state().await;
        let created = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "main",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create download client");

        let response = update_download_client(
            State(state.clone()),
            Path(created.id.to_string()),
            Json(UpdateDownloadClientRequest {
                name: Some("renamed".to_string()),
                client_type: Some("qbittorrent".to_string()),
                base_url: Some("https://new-downloads.example".to_string()),
                username: Some("operator".to_string()),
                password: Some("new-secret".to_string()),
                category: Some("lossless".to_string()),
                enabled: Some(false),
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let updated = state
            .download_client_definition_repository
            .get_by_id(&created.id.to_string())
            .await
            .expect("fetch")
            .expect("exists");
        assert_eq!(updated.name, "renamed");
        assert_eq!(updated.base_url, "https://new-downloads.example");
        assert_eq!(updated.username.as_deref(), Some("operator"));
        assert_eq!(updated.category.as_deref(), Some("lossless"));
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn delete_download_client_returns_not_found_after_delete() {
        let state = make_test_state().await;
        let created = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "main",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create download client");

        let delete_response =
            delete_download_client(State(state.clone()), Path(created.id.to_string()))
                .await
                .into_response();
        assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

        let get_response = get_download_client(State(state), Path(created.id.to_string()))
            .await
            .into_response();
        assert_eq!(get_response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn import_download_clients_rejects_unsupported_version() {
        let state = make_test_state().await;

        let response = import_download_clients(
            State(state),
            Query(SettingsImportQuery { dry_run: false }),
            Json(DownloadClientImportRequest {
                version: "2".to_string(),
                conflict_policy: ImportConflictPolicy::Merge,
                items: vec![],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn import_download_clients_rejects_unsupported_client_type_item() {
        let state = make_test_state().await;

        let response = import_download_clients(
            State(state),
            Query(SettingsImportQuery { dry_run: false }),
            Json(DownloadClientImportRequest {
                version: "1".to_string(),
                conflict_policy: ImportConflictPolicy::Merge,
                items: vec![DownloadClientImportItem {
                    name: "Imported".to_string(),
                    client_type: "not-supported".to_string(),
                    base_url: "https://downloads.example".to_string(),
                    username: None,
                    password: None,
                    category: None,
                    enabled: true,
                }],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn bulk_download_clients_enables_selected_items() {
        let state = make_test_state().await;
        let first = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "first",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create first");
        let mut second = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "second",
                "transmission",
                "https://downloads.example",
            ))
            .await
            .expect("create second");
        second.enabled = false;
        state
            .download_client_definition_repository
            .update(second)
            .await
            .expect("disable second");

        let response = bulk_download_clients(
            State(state.clone()),
            Json(DownloadClientBulkRequest {
                action: "enable".to_string(),
                ids: vec![first.id.to_string()],
            }),
        )
        .await
        .into_response();

        assert_eq!(response.status(), StatusCode::OK);

        let updated = state
            .download_client_definition_repository
            .get_by_id(&first.id.to_string())
            .await
            .expect("fetch updated")
            .expect("exists");
        assert!(updated.enabled);
    }

    #[tokio::test]
    async fn create_download_client_returns_conflict_for_duplicate_name() {
        let state = make_test_state().await;
        let first = create_download_client(
            State(state.clone()),
            Json(CreateDownloadClientRequest {
                name: "Duplicate".to_string(),
                client_type: "qbittorrent".to_string(),
                base_url: "https://first.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();
        assert_eq!(first.status(), StatusCode::CREATED);

        let second = create_download_client(
            State(state.clone()),
            Json(CreateDownloadClientRequest {
                name: "Duplicate".to_string(),
                client_type: "qbittorrent".to_string(),
                base_url: "https://second.example".to_string(),
                username: None,
                password: None,
                category: None,
                enabled: true,
            }),
        )
        .await
        .into_response();
        assert_eq!(second.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn bulk_download_clients_disables_selected_items() {
        let state = make_test_state().await;
        let client = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "active-client",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create client");
        assert!(client.enabled);

        let response = bulk_download_clients(
            State(state.clone()),
            Json(DownloadClientBulkRequest {
                action: "disable".to_string(),
                ids: vec![client.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let updated = state
            .download_client_definition_repository
            .get_by_id(&client.id.to_string())
            .await
            .expect("fetch updated")
            .expect("exists");
        assert!(!updated.enabled);
    }

    #[tokio::test]
    async fn bulk_download_clients_deletes_selected_items() {
        let state = make_test_state().await;
        let client = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "to-delete",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create client");

        let response = bulk_download_clients(
            State(state.clone()),
            Json(DownloadClientBulkRequest {
                action: "delete".to_string(),
                ids: vec![client.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let fetched = state
            .download_client_definition_repository
            .get_by_id(&client.id.to_string())
            .await
            .expect("get_by_id");
        assert!(fetched.is_none(), "client should be deleted");
    }

    #[tokio::test]
    async fn bulk_download_clients_rejects_empty_ids() {
        let state = make_test_state().await;
        let response = bulk_download_clients(
            State(state),
            Json(DownloadClientBulkRequest {
                action: "delete".to_string(),
                ids: vec![],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn bulk_download_clients_rejects_invalid_action() {
        let state = make_test_state().await;
        let client = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "test-client",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create client");
        let response = bulk_download_clients(
            State(state),
            Json(DownloadClientBulkRequest {
                action: "frobulate".to_string(),
                ids: vec![client.id.to_string()],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn bulk_download_clients_returns_207_for_partial_failure() {
        let state = make_test_state().await;
        let client = state
            .download_client_definition_repository
            .create(DownloadClientDefinition::new(
                "real-client",
                "qbittorrent",
                "https://downloads.example",
            ))
            .await
            .expect("create client");
        let missing_id = "00000000-0000-0000-0000-000000000000".to_string();

        let response = bulk_download_clients(
            State(state.clone()),
            Json(DownloadClientBulkRequest {
                action: "delete".to_string(),
                ids: vec![client.id.to_string(), missing_id],
            }),
        )
        .await
        .into_response();
        assert_eq!(response.status(), StatusCode::MULTI_STATUS);
    }
}

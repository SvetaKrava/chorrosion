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
}

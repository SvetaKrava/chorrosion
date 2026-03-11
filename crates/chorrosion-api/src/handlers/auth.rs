// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use chorrosion_application::AppState;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use tokio::sync::RwLock;
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ApiKeyRecord {
    id: String,
    key: String,
    name: Option<String>,
    created_at: chrono::DateTime<Utc>,
    last_used_at: Option<chrono::DateTime<Utc>>,
}

static API_KEYS: OnceLock<RwLock<Vec<ApiKeyRecord>>> = OnceLock::new();

fn api_key_store() -> &'static RwLock<Vec<ApiKeyRecord>> {
    API_KEYS.get_or_init(|| RwLock::new(Vec::new()))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: String,
    pub key: String,
    pub name: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyMetadataResponse {
    pub id: String,
    pub key_prefix: String,
    pub name: Option<String>,
    pub created_at: String,
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ListApiKeysResponse {
    pub items: Vec<ApiKeyMetadataResponse>,
    pub total: i64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteApiKeyResponse {
    pub deleted: bool,
    pub id: String,
}

fn key_prefix(key: &str) -> String {
    key.chars().take(8).collect()
}

fn to_metadata(record: &ApiKeyRecord) -> ApiKeyMetadataResponse {
    ApiKeyMetadataResponse {
        id: record.id.clone(),
        key_prefix: key_prefix(&record.key),
        name: record.name.clone(),
        created_at: record.created_at.to_rfc3339(),
        last_used_at: record.last_used_at.map(|ts| ts.to_rfc3339()),
    }
}

pub(crate) async fn api_key_count() -> usize {
    api_key_store().read().await.len()
}

pub(crate) async fn validate_api_key_and_touch(key: &str) -> bool {
    // First check with read-lock so concurrent requests are not serialized.
    {
        let store = api_key_store().read().await;
        if !store.iter().any(|r| r.key == key) {
            return false;
        }
    }
    // Key exists; take write-lock only to update last_used_at.
    let now = Utc::now();
    let mut store = api_key_store().write().await;
    if let Some(record) = store.iter_mut().find(|r| r.key == key) {
        record.last_used_at = Some(now);
        true
    } else {
        // Key was removed between the read-lock check and here.
        false
    }
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/api-keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created", body = ApiKeyResponse)
    ),
    security(()),
    tag = "auth"
)]
pub async fn create_api_key(
    State(_state): State<AppState>,
    Json(request): Json<CreateApiKeyRequest>,
) -> (StatusCode, Json<ApiKeyResponse>) {
    let id = Uuid::new_v4().to_string();
    let key = format!("ck_{}", Uuid::new_v4());
    let created_at = Utc::now();

    let record = ApiKeyRecord {
        id: id.clone(),
        key: key.clone(),
        name: request.name.clone(),
        created_at,
        last_used_at: None,
    };

    api_key_store().write().await.push(record);

    (
        StatusCode::CREATED,
        Json(ApiKeyResponse {
            id,
            key,
            name: request.name,
            created_at: created_at.to_rfc3339(),
        }),
    )
}

#[utoipa::path(
    get,
    path = "/api/v1/auth/api-keys",
    responses(
        (status = 200, description = "List API keys", body = ListApiKeysResponse)
    ),
    tag = "auth"
)]
pub async fn list_api_keys(State(_state): State<AppState>) -> Json<ListApiKeysResponse> {
    debug!(target: "api", "listing api keys");

    let store = api_key_store().read().await;
    let items = store.iter().map(to_metadata).collect::<Vec<_>>();

    Json(ListApiKeysResponse {
        total: items.len() as i64,
        items,
    })
}

#[utoipa::path(
    delete,
    path = "/api/v1/auth/api-keys/{id}",
    params(
        ("id" = String, Path, description = "API key ID")
    ),
    responses(
        (status = 200, description = "API key deleted", body = DeleteApiKeyResponse),
        (status = 404, description = "API key not found", body = AuthErrorResponse)
    ),
    tag = "auth"
)]
pub async fn delete_api_key(
    State(_state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<DeleteApiKeyResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    let mut store = api_key_store().write().await;
    let before = store.len();
    store.retain(|record| record.id != id);

    if store.len() == before {
        return Err((
            StatusCode::NOT_FOUND,
            Json(AuthErrorResponse {
                error: format!("API key {} not found", id),
            }),
        ));
    }

    Ok(Json(DeleteApiKeyResponse { deleted: true, id }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use std::sync::{Arc, OnceLock};

    static TEST_MUTEX: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

    fn test_mutex() -> &'static tokio::sync::Mutex<()> {
        TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    async fn reset_store() {
        api_key_store().write().await.clear();
    }

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
        )
    }

    #[tokio::test]
    async fn api_key_lifecycle_create_list_validate_delete() {
        let _lock = test_mutex().lock().await;
        reset_store().await;
        let state = make_test_state().await;

        let (status, Json(created)) = create_api_key(
            State(state.clone()),
            Json(CreateApiKeyRequest {
                name: Some("test".to_string()),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(created.key.starts_with("ck_"));

        let Json(listed) = list_api_keys(State(state.clone())).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, created.id);

        let valid = validate_api_key_and_touch(&created.key).await;
        assert!(valid);

        let deleted = delete_api_key(State(state), Path(created.id.clone()))
            .await
            .expect("delete should succeed");
        assert_eq!(deleted.0.deleted, true);
    }
}

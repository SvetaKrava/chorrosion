// SPDX-License-Identifier: GPL-3.0-or-later
use axum::{
    extract::{Path, State},
    http::StatusCode,
    http::{header, HeaderMap},
    response::AppendHeaders,
    Form, Json,
};
use chorrosion_application::AppState;
use chorrosion_config::PermissionLevel;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use subtle::ConstantTimeEq;
use tokio::sync::RwLock;
use tracing::debug;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone)]
struct ApiKeyRecord {
    id: String,
    key: String,
    name: Option<String>,
    permission_level: PermissionLevel,
    created_at: chrono::DateTime<Utc>,
    last_used_at: Option<chrono::DateTime<Utc>>,
}

const SESSION_TTL_SECONDS: i64 = 86_400;

#[derive(Debug, Clone)]
struct FormSessionRecord {
    token: String,
    permission_level: PermissionLevel,
    created_at: chrono::DateTime<Utc>,
    last_used_at: Option<chrono::DateTime<Utc>>,
}

static API_KEYS: OnceLock<RwLock<Vec<ApiKeyRecord>>> = OnceLock::new();
static FORM_SESSIONS: OnceLock<RwLock<Vec<FormSessionRecord>>> = OnceLock::new();

fn api_key_store() -> &'static RwLock<Vec<ApiKeyRecord>> {
    API_KEYS.get_or_init(|| RwLock::new(Vec::new()))
}

fn form_session_store() -> &'static RwLock<Vec<FormSessionRecord>> {
    FORM_SESSIONS.get_or_init(|| RwLock::new(Vec::new()))
}

#[derive(Debug, Serialize, ToSchema)]
pub struct AuthErrorResponse {
    pub error: String,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateApiKeyRequest {
    pub name: Option<String>,
    pub permission_level: Option<PermissionLevel>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyResponse {
    pub id: String,
    pub key: String,
    pub name: Option<String>,
    pub permission_level: PermissionLevel,
    pub created_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ApiKeyMetadataResponse {
    pub id: String,
    pub key_prefix: String,
    pub name: Option<String>,
    pub permission_level: PermissionLevel,
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

#[derive(Debug, Deserialize, ToSchema)]
pub struct FormsLoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FormsLoginResponse {
    pub authenticated: bool,
    pub username: String,
    pub permission_level: PermissionLevel,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FormsLogoutResponse {
    pub logged_out: bool,
}

fn key_prefix(key: &str) -> String {
    key.chars().take(8).collect()
}

fn to_metadata(record: &ApiKeyRecord) -> ApiKeyMetadataResponse {
    ApiKeyMetadataResponse {
        id: record.id.clone(),
        key_prefix: key_prefix(&record.key),
        name: record.name.clone(),
        permission_level: record.permission_level,
        created_at: record.created_at.to_rfc3339(),
        last_used_at: record.last_used_at.map(|ts| ts.to_rfc3339()),
    }
}

pub(crate) async fn api_key_count() -> usize {
    api_key_store().read().await.len()
}

pub(crate) async fn validate_api_key_and_touch(key: &str) -> Option<PermissionLevel> {
    // First check with read-lock so concurrent requests are not serialized.
    {
        let store = api_key_store().read().await;
        if !store.iter().any(|r| r.key == key) {
            return None;
        }
    }
    // Yield between the two locks in tests so that a concurrently spawned delete
    // task can acquire the write-lock and remove the key before we do, making
    // the TOCTOU window deterministically observable in unit tests.
    #[cfg(test)]
    tokio::task::yield_now().await;

    // Key exists; take write-lock only to update last_used_at.
    let now = Utc::now();
    let mut store = api_key_store().write().await;
    if let Some(record) = store.iter_mut().find(|r| r.key == key) {
        record.last_used_at = Some(now);
        Some(record.permission_level)
    } else {
        // Key was removed between the read-lock check and here (TOCTOU).
        None
    }
}

fn build_form_session_cookie(token: &str) -> String {
    format!(
        "chorrosion_session={token}; HttpOnly; Path=/; SameSite=Lax; Secure; Max-Age={SESSION_TTL_SECONDS}"
    )
}

fn clear_form_session_cookie() -> &'static str {
    "chorrosion_session=; HttpOnly; Path=/; SameSite=Lax; Secure; Max-Age=0"
}

/// Matches the middleware's credential comparison including the same `MAX_CREDENTIAL_BYTES`
/// truncation so both auth paths behave consistently for long credentials.
const MAX_CREDENTIAL_BYTES: usize = 256;

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    // Truncate to cap allocation/CPU work on attacker-controlled input.
    let a = &a[..a.len().min(MAX_CREDENTIAL_BYTES)];
    let b = &b[..b.len().min(MAX_CREDENTIAL_BYTES)];

    let target_len = b.len().max(1);
    let mut pa = vec![0u8; target_len];
    let mut pb = vec![0u8; target_len];
    let a_copy_len = a.len().min(target_len);
    pa[..a_copy_len].copy_from_slice(&a[..a_copy_len]);
    pb[..b.len()].copy_from_slice(b);

    let lengths_equal = subtle::Choice::from((a.len() == b.len()) as u8);
    let contents_equal = pa.ct_eq(&pb);
    bool::from(lengths_equal & contents_equal)
}

pub(crate) fn extract_form_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_header = headers.get(header::COOKIE)?;
    let raw = cookie_header.to_str().ok()?;
    for part in raw.split(';') {
        let trimmed = part.trim();
        if let Some(token) = trimmed.strip_prefix("chorrosion_session=") {
            let token = token.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }
    None
}

pub(crate) async fn validate_form_session_and_touch(token: &str) -> Option<PermissionLevel> {
    let now = Utc::now();
    {
        let store = form_session_store().read().await;
        match store.iter().find(|r| r.token == token) {
            None => return None,
            Some(record) => {
                // Reject sessions that have exceeded SESSION_TTL_SECONDS.
                let age_secs = now.signed_duration_since(record.created_at).num_seconds();
                if age_secs >= SESSION_TTL_SECONDS {
                    // Will be evicted on the write-lock pass below.
                    drop(store);
                    form_session_store()
                        .write()
                        .await
                        .retain(|r| r.token != token);
                    return None;
                }
            }
        }
    }

    let mut store = form_session_store().write().await;
    // Prune any other sessions that have expired while we hold the write lock.
    store.retain(|r| now.signed_duration_since(r.created_at).num_seconds() < SESSION_TTL_SECONDS);
    if let Some(record) = store.iter_mut().find(|r| r.token == token) {
        record.last_used_at = Some(now);
        Some(record.permission_level)
    } else {
        // Key was removed between the read-lock check and here (TOCTOU).
        None
    }
}

pub(crate) async fn revoke_form_session(token: &str) -> bool {
    let mut store = form_session_store().write().await;
    let before = store.len();
    store.retain(|record| record.token != token);
    store.len() != before
}

fn forms_auth_configured(state: &AppState) -> bool {
    state
        .config
        .auth
        .basic_username
        .as_ref()
        .is_some_and(|v| !v.trim().is_empty())
        && state
            .config
            .auth
            .basic_password
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty())
}

pub(crate) fn unauthorized_response() -> (StatusCode, Json<AuthErrorResponse>) {
    (
        StatusCode::UNAUTHORIZED,
        Json(AuthErrorResponse {
            error: "Unauthorized".to_string(),
        }),
    )
}

pub(crate) fn permission_denied_response() -> (StatusCode, Json<AuthErrorResponse>) {
    (
        StatusCode::FORBIDDEN,
        Json(AuthErrorResponse {
            error: "insufficient permissions".to_string(),
        }),
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forms/login",
    request_body(content = FormsLoginRequest, content_type = "application/x-www-form-urlencoded"),
    responses(
        (status = 200, description = "Forms login successful", body = FormsLoginResponse),
        (status = 401, description = "Invalid credentials", body = AuthErrorResponse),
        (status = 503, description = "Forms auth not configured", body = AuthErrorResponse)
    ),
    security(()),
    tag = "auth"
)]
pub async fn forms_login(
    State(state): State<AppState>,
    Form(request): Form<FormsLoginRequest>,
) -> Result<
    (
        AppendHeaders<[(header::HeaderName, String); 1]>,
        Json<FormsLoginResponse>,
    ),
    (StatusCode, Json<AuthErrorResponse>),
> {
    if !forms_auth_configured(&state) {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(AuthErrorResponse {
                error: "forms authentication is not configured".to_string(),
            }),
        ));
    }

    let expected_username = state
        .config
        .auth
        .basic_username
        .as_deref()
        .unwrap_or_default();
    let expected_password = state
        .config
        .auth
        .basic_password
        .as_deref()
        .unwrap_or_default();

    if !constant_time_eq(request.username.as_bytes(), expected_username.as_bytes())
        || !constant_time_eq(request.password.as_bytes(), expected_password.as_bytes())
    {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "invalid credentials".to_string(),
            }),
        ));
    }

    let token = format!("cs_{}", Uuid::new_v4());
    let permission_level = state.config.auth.basic_permission_level;
    let record = FormSessionRecord {
        token: token.clone(),
        permission_level,
        created_at: Utc::now(),
        last_used_at: None,
    };
    form_session_store().write().await.push(record);

    Ok((
        AppendHeaders([(header::SET_COOKIE, build_form_session_cookie(&token))]),
        Json(FormsLoginResponse {
            authenticated: true,
            username: request.username,
            permission_level,
        }),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/forms/logout",
    responses(
        (status = 200, description = "Forms logout completed", body = FormsLogoutResponse)
    ),
    tag = "auth"
)]
pub async fn forms_logout(
    headers: HeaderMap,
) -> (
    AppendHeaders<[(header::HeaderName, &'static str); 1]>,
    Json<FormsLogoutResponse>,
) {
    let revoked = if let Some(token) = extract_form_session_token(&headers) {
        revoke_form_session(&token).await
    } else {
        false
    };

    (
        AppendHeaders([(header::SET_COOKIE, clear_form_session_cookie())]),
        Json(FormsLogoutResponse {
            logged_out: revoked,
        }),
    )
}

#[utoipa::path(
    post,
    path = "/api/v1/auth/api-keys",
    request_body = CreateApiKeyRequest,
    responses(
        (status = 201, description = "API key created", body = ApiKeyResponse)
    ),
    tag = "auth",
    description = "Create a new API key.\n\nThis endpoint requires authentication except during initial bootstrap: \
        when no API keys exist the request is allowed without credentials so that the very \
        first key can be created. Once at least one key exists, a valid API key must be \
        supplied via `X-Api-Key` or `Authorization: Bearer <key>`."
)]
pub async fn create_api_key(
    State(_state): State<AppState>,
    Json(request): Json<CreateApiKeyRequest>,
) -> (StatusCode, Json<ApiKeyResponse>) {
    let id = Uuid::new_v4().to_string();
    let key = format!("ck_{}", Uuid::new_v4());
    let created_at = Utc::now();
    let permission_level = request.permission_level.unwrap_or_default();

    let record = ApiKeyRecord {
        id: id.clone(),
        key: key.clone(),
        name: request.name.clone(),
        permission_level,
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
            permission_level,
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
static AUTH_TEST_MUTEX: std::sync::OnceLock<tokio::sync::Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn auth_test_mutex() -> &'static tokio::sync::Mutex<()> {
    AUTH_TEST_MUTEX.get_or_init(|| tokio::sync::Mutex::new(()))
}

#[cfg(test)]
pub(crate) async fn clear_stores_for_tests() {
    api_key_store().write().await.clear();
    if let Some(store) = FORM_SESSIONS.get() {
        store.write().await.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }

    #[tokio::test]
    async fn api_key_lifecycle_create_list_validate_delete() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;
        let state = make_test_state().await;

        let (status, Json(created)) = create_api_key(
            State(state.clone()),
            Json(CreateApiKeyRequest {
                name: Some("test".to_string()),
                permission_level: None,
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);
        assert!(created.key.starts_with("ck_"));
        assert_eq!(created.permission_level, PermissionLevel::Admin);

        let Json(listed) = list_api_keys(State(state.clone())).await;
        assert_eq!(listed.total, 1);
        assert_eq!(listed.items[0].id, created.id);

        let valid = validate_api_key_and_touch(&created.key).await;
        assert_eq!(valid, Some(PermissionLevel::Admin));

        let deleted = delete_api_key(State(state), Path(created.id.clone()))
            .await
            .expect("delete should succeed");
        assert!(deleted.0.deleted);
    }

    /// Tests the TOCTOU branch in `validate_api_key_and_touch` where the key is
    /// deleted between the read-lock check and the write-lock update.
    ///
    /// The `#[cfg(test)]` `yield_now()` hook inside the function ensures that a
    /// concurrently spawned delete task runs in the gap between the two lock
    /// acquisitions, making the race window deterministically observable.
    #[tokio::test]
    async fn validate_returns_false_when_key_deleted_between_locks() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;
        let state = make_test_state().await;

        let (_, Json(created)) = create_api_key(
            State(state.clone()),
            Json(CreateApiKeyRequest {
                name: Some("toctou".to_string()),
                permission_level: None,
            }),
        )
        .await;
        let key = created.key.clone();

        // Spawn a task that deletes the key. It will run when `validate_api_key_and_touch`
        // yields (via the `#[cfg(test)] yield_now()` hook) between releasing the
        // read-lock and acquiring the write-lock.
        let key_to_delete = key.clone();
        let delete_handle = tokio::spawn(async move {
            let mut store = api_key_store().write().await;
            store.retain(|r| r.key != key_to_delete);
        });

        // validate passes the read-lock check (key exists), yields, the delete task
        // runs, and then validate finds the key gone at the write-lock step.
        let result = validate_api_key_and_touch(&key).await;
        delete_handle.await.expect("delete task should not panic");

        assert!(
            result.is_none(),
            "validate_api_key_and_touch must return false when key is deleted between locks"
        );
    }

    #[tokio::test]
    async fn create_api_key_allows_read_only_permission_level() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;
        let state = make_test_state().await;

        let (status, Json(created)) = create_api_key(
            State(state),
            Json(CreateApiKeyRequest {
                name: Some("viewer".to_string()),
                permission_level: Some(PermissionLevel::ReadOnly),
            }),
        )
        .await;

        assert_eq!(status, StatusCode::CREATED);
        assert_eq!(created.permission_level, PermissionLevel::ReadOnly);
    }

    #[tokio::test]
    async fn forms_login_and_logout_lifecycle() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state_with_config(config).await;

        let login = forms_login(
            State(state),
            Form(FormsLoginRequest {
                username: "user".to_string(),
                password: "pass".to_string(),
            }),
        )
        .await
        .expect("forms login should succeed");

        let set_cookie = login
            .0
             .0
            .iter()
            .find(|(name, _)| *name == header::SET_COOKIE)
            .map(|(_, value)| value.clone())
            .expect("set-cookie header");
        let cookie_header = set_cookie
            .split(';')
            .next()
            .expect("cookie pair")
            .to_string();

        let mut headers = HeaderMap::new();
        headers.insert(
            header::COOKIE,
            cookie_header.parse().expect("cookie header value"),
        );

        let token = extract_form_session_token(&headers).expect("session token extracted");
        assert_eq!(
            validate_form_session_and_touch(&token).await,
            Some(PermissionLevel::Admin)
        );

        let (_, Json(logout_body)) = forms_logout(headers).await;
        assert!(logout_body.logged_out);
        assert_eq!(validate_form_session_and_touch(&token).await, None);
    }

    #[tokio::test]
    async fn forms_login_rejects_invalid_credentials() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state_with_config(config).await;

        let login = forms_login(
            State(state),
            Form(FormsLoginRequest {
                username: "user".to_string(),
                password: "wrong".to_string(),
            }),
        )
        .await;

        assert!(matches!(login, Err((StatusCode::UNAUTHORIZED, _))));
    }

    #[tokio::test]
    async fn forms_login_returns_configured_permission_level() {
        let _lock = auth_test_mutex().lock().await;
        clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        config.auth.basic_permission_level = PermissionLevel::ReadOnly;
        let state = make_test_state_with_config(config).await;

        let (_, Json(response)) = forms_login(
            State(state),
            Form(FormsLoginRequest {
                username: "user".to_string(),
                password: "pass".to_string(),
            }),
        )
        .await
        .expect("forms login should succeed");

        assert_eq!(response.permission_level, PermissionLevel::ReadOnly);
    }

    async fn make_test_state_with_config(config: AppConfig) -> AppState {
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
            config,
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
            chorrosion_infrastructure::ResponseCache::new(100, 60),
        )
    }
}

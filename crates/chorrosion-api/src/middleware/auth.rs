// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::auth::{api_key_count, validate_api_key_and_touch};
use crate::API_V1_BASE;
use axum::{
    extract::{Request, State},
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use tracing::debug;

fn extract_api_key(headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(api_key) = headers.get("X-Api-Key") {
        if let Ok(value) = api_key.to_str() {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            let auth_str = auth_str.trim();
            if let Some((scheme, rest)) = auth_str.split_once(' ') {
                if scheme.eq_ignore_ascii_case("bearer") {
                    let trimmed = rest.trim();
                    if !trimmed.is_empty() {
                        return Some(trimmed.to_string());
                    }
                }
            }
        }
    }

    None
}

fn extract_basic_credentials(headers: &axum::http::HeaderMap) -> Option<(String, String)> {
    let auth_header = headers.get("Authorization")?;
    let auth_str = auth_header.to_str().ok()?.trim();
    let (scheme, encoded) = auth_str.split_once(' ')?;

    if !scheme.eq_ignore_ascii_case("basic") {
        return None;
    }

    let decoded = BASE64_STANDARD.decode(encoded.trim()).ok()?;
    let decoded = String::from_utf8(decoded).ok()?;
    let (username, password) = decoded.split_once(':')?;

    Some((username.to_string(), password.to_string()))
}

fn basic_auth_is_configured(state: &chorrosion_application::AppState) -> bool {
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

/// API key authentication middleware.
pub async fn auth_middleware(
    State(state): State<chorrosion_application::AppState>,
    request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Bootstrap bypass: allow POST /api/v1/auth/api-keys only when no keys exist yet,
    // so the first key can be created without requiring prior authentication.
    if method == Method::POST
        && path.strip_prefix(API_V1_BASE) == Some("/auth/api-keys")
        && api_key_count().await == 0
    {
        debug!(target: "auth", %path, "auth bootstrap: no keys exist, allowing first key creation");
        return next.run(request).await;
    }

    if basic_auth_is_configured(&state) {
        if let Some((username, password)) = extract_basic_credentials(request.headers()) {
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

            if username == expected_username && password == expected_password {
                debug!(target: "auth", %path, "basic authentication successful");
                return next.run(request).await;
            }
            debug!(target: "auth", %path, "basic authentication failed");
            return unauthorized().await.into_response();
        }
    }

    if let Some(api_key) = extract_api_key(request.headers()) {
        if validate_api_key_and_touch(&api_key).await {
            debug!(target: "auth", %path, "API key authentication successful");
            return next.run(request).await;
        }
        debug!(target: "auth", %path, "API key authentication failed");
        return unauthorized().await.into_response();
    }

    debug!(target: "auth", %path, "missing API key or bearer token");
    unauthorized().await.into_response()
}

/// Response for unauthorized requests
pub async fn unauthorized() -> impl IntoResponse {
    (StatusCode::UNAUTHORIZED, "Unauthorized")
}

#[cfg(test)]
mod tests {
    use super::{extract_api_key, extract_basic_credentials};
    use axum::{
        body::Body,
        http::{HeaderMap, HeaderValue, Request, StatusCode},
    };
    use chorrosion_application::AppState;
    use chorrosion_config::AppConfig;
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    };
    use std::sync::Arc;
    use tower::util::ServiceExt;

    #[test]
    fn extract_api_key_prefers_x_api_key_header() {
        let mut headers = HeaderMap::new();
        headers.insert("X-Api-Key", HeaderValue::from_static("direct-key"));
        headers.insert("Authorization", HeaderValue::from_static("Bearer bearer-key"));

        let extracted = extract_api_key(&headers);
        assert_eq!(extracted.as_deref(), Some("direct-key"));
    }

    #[test]
    fn extract_api_key_accepts_bearer_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer some-token"));

        let extracted = extract_api_key(&headers);
        assert_eq!(extracted.as_deref(), Some("some-token"));
    }

    #[test]
    fn extract_basic_credentials_returns_username_and_password() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Basic dXNlcjpwYXNz"),
        );

        let extracted = extract_basic_credentials(&headers);
        assert_eq!(
            extracted,
            Some(("user".to_string(), "pass".to_string()))
        );
    }

    #[test]
    fn extract_basic_credentials_rejects_malformed_base64() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Basic !not-base64!"));

        let extracted = extract_basic_credentials(&headers);
        assert!(extracted.is_none());
    }

    #[test]
    fn extract_basic_credentials_rejects_payload_without_colon() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Basic dXNlcm9ubHk="),
        );

        let extracted = extract_basic_credentials(&headers);
        assert!(extracted.is_none());
    }

    #[test]
    fn extract_basic_credentials_ignores_non_basic_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert("Authorization", HeaderValue::from_static("Bearer some-token"));

        let extracted = extract_basic_credentials(&headers);
        assert!(extracted.is_none());
    }

    async fn make_test_state(config: AppConfig) -> AppState {
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
            Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
        )
    }

    #[tokio::test]
    async fn middleware_allows_valid_basic_auth_when_configured() {
        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .header("Authorization", "Basic dXNlcjpwYXNz")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_rejects_invalid_basic_auth_when_configured() {
        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .header("Authorization", "Basic dXNlcjp3cm9uZw==")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_missing_auth_when_basic_is_configured() {
        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }
}

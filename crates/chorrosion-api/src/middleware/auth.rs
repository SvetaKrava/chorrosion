// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::auth::{api_key_count, validate_api_key_and_touch};
use crate::API_V1_BASE;
use axum::{
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use subtle::ConstantTimeEq;
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

/// Constant-time byte-slice equality to prevent timing attacks during credential comparison.
///
/// Uses `subtle::ConstantTimeEq` for the byte content and a constant-time length check,
/// ensuring the comparison time does not reveal information about the expected credential.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    // Build padded slices so the content comparison always runs the same number of iterations,
    // regardless of the lengths supplied. The length equality flag is combined via bitwise AND
    // to avoid short-circuit branching.
    let max_len = a.len().max(b.len()).max(1);
    let mut pa = vec![0u8; max_len];
    let mut pb = vec![0u8; max_len];
    pa[..a.len()].copy_from_slice(a);
    pb[..b.len()].copy_from_slice(b);

    let lengths_equal = subtle::Choice::from((a.len() == b.len()) as u8);
    let contents_equal = pa.ct_eq(&pb);
    bool::from(lengths_equal & contents_equal)
}

/// Authentication middleware supporting API key and optional HTTP Basic auth.
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();

    // Borrow auth config fields from the shared state without cloning the full AppState.
    let (basic_username_opt, basic_password_opt) = {
        let state = request
            .extensions()
            .get::<chorrosion_application::AppState>()
            .expect("AppState missing from request extensions");
        (
            state.config.auth.basic_username.clone(),
            state.config.auth.basic_password.clone(),
        )
    };

    let basic_configured = basic_username_opt
        .as_ref()
        .is_some_and(|v| !v.trim().is_empty())
        && basic_password_opt
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty());

    // Bootstrap bypass: allow POST /api/v1/auth/api-keys only when no keys exist yet,
    // so the first key can be created without requiring prior authentication.
    if method == Method::POST
        && path.strip_prefix(API_V1_BASE) == Some("/auth/api-keys")
        && api_key_count().await == 0
    {
        debug!(target: "auth", %path, "auth bootstrap: no keys exist, allowing first key creation");
        return next.run(request).await;
    }

    if basic_configured {
        if let Some((username, password)) = extract_basic_credentials(request.headers()) {
            let expected_username = basic_username_opt.as_deref().unwrap_or_default();
            let expected_password = basic_password_opt.as_deref().unwrap_or_default();

            if constant_time_eq(username.as_bytes(), expected_username.as_bytes())
                && constant_time_eq(password.as_bytes(), expected_password.as_bytes())
            {
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
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer bearer-key"),
        );

        let extracted = extract_api_key(&headers);
        assert_eq!(extracted.as_deref(), Some("direct-key"));
    }

    #[test]
    fn extract_api_key_accepts_bearer_authorization() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer some-token"),
        );

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
        assert_eq!(extracted, Some(("user".to_string(), "pass".to_string())));
    }

    #[test]
    fn extract_basic_credentials_rejects_malformed_base64() {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Basic !not-base64!"),
        );

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
        headers.insert(
            "Authorization",
            HeaderValue::from_static("Bearer some-token"),
        );

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

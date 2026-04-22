// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::auth::{
    api_key_count, extract_form_session_token, permission_denied_response, unauthorized_response,
    validate_api_key_and_touch, validate_form_session_and_touch,
};
use crate::API_V1_BASE;
use axum::{
    extract::{Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine as _};
use chorrosion_config::PermissionLevel;
use subtle::ConstantTimeEq;
use tracing::debug;

fn path_matches(path: &str, route: &str) -> bool {
    path == route || path.strip_prefix(API_V1_BASE) == Some(route)
}

fn is_mutating_method(method: &Method) -> bool {
    matches!(
        method,
        &Method::POST | &Method::PUT | &Method::PATCH | &Method::DELETE
    )
}

fn allows_read_only_access(method: &Method, path: &str) -> bool {
    !is_mutating_method(method)
        || (method == Method::POST && path_matches(path, "/auth/forms/logout"))
}

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
/// Credentials are capped at `MAX_CREDENTIAL_BYTES` before comparison to prevent
/// a DoS attack where an attacker sends a very long credential to force large allocations.
/// Both length and content comparisons run in constant time via `subtle::ConstantTimeEq`.
const MAX_CREDENTIAL_BYTES: usize = 256;

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    // Truncate inputs to cap allocation and CPU work regardless of attacker-controlled length.
    let a = &a[..a.len().min(MAX_CREDENTIAL_BYTES)];
    let b = &b[..b.len().min(MAX_CREDENTIAL_BYTES)];

    // Pad both slices to the expected (b) length so the comparison always iterates the same
    // number of bytes, preventing the attacker from learning the expected credential length.
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

fn permission_allows_request(
    permission_level: PermissionLevel,
    method: &Method,
    path: &str,
) -> bool {
    match permission_level {
        PermissionLevel::Admin => true,
        PermissionLevel::ReadOnly => allows_read_only_access(method, path),
    }
}

/// Authentication middleware supporting API key and optional HTTP Basic auth.
pub async fn auth_middleware(
    State(state): State<chorrosion_application::AppState>,
    request: Request,
    next: Next,
) -> Response {
    // Extract only the auth config fields needed, then drop the AppState clone immediately.
    let basic_username_opt = state.config.auth.basic_username.clone();
    let basic_password_opt = state.config.auth.basic_password.clone();

    let path = request.uri().path().to_string();
    let method = request.method().clone();

    let basic_configured = basic_username_opt
        .as_ref()
        .is_some_and(|v| !v.trim().is_empty())
        && basic_password_opt
            .as_ref()
            .is_some_and(|v| !v.trim().is_empty());

    // Bootstrap bypass: allow POST /api/v1/auth/api-keys only when no keys exist yet,
    // so the first key can be created without requiring prior authentication.
    if method == Method::POST && path_matches(&path, "/auth/api-keys") && api_key_count().await == 0
    {
        debug!(target: "auth", %path, "auth bootstrap: no keys exist, allowing first key creation");
        return next.run(request).await;
    }

    // Forms-login bypass: allow POST /api/v1/auth/forms/login without prior auth.
    if method == Method::POST && path_matches(&path, "/auth/forms/login") {
        debug!(target: "auth", %path, "auth forms-login bypass");
        return next.run(request).await;
    }

    if basic_configured {
        if let Some((username, password)) = extract_basic_credentials(request.headers()) {
            let expected_username = basic_username_opt.as_deref().unwrap_or_default();
            let expected_password = basic_password_opt.as_deref().unwrap_or_default();

            if constant_time_eq(username.as_bytes(), expected_username.as_bytes())
                && constant_time_eq(password.as_bytes(), expected_password.as_bytes())
            {
                if !permission_allows_request(
                    state.config.auth.basic_permission_level,
                    &method,
                    &path,
                ) {
                    debug!(target: "auth", %path, "basic authentication denied by permission level");
                    return permission_denied_response().into_response();
                }
                debug!(target: "auth", %path, "basic authentication successful");
                return next.run(request).await;
            }
            debug!(target: "auth", %path, "basic authentication failed");
            return unauthorized_response().into_response();
        }
    }

    if let Some(api_key) = extract_api_key(request.headers()) {
        if let Some(permission_level) = validate_api_key_and_touch(&api_key).await {
            if !permission_allows_request(permission_level, &method, &path) {
                debug!(target: "auth", %path, "API key authentication denied by permission level");
                return permission_denied_response().into_response();
            }
            debug!(target: "auth", %path, "API key authentication successful");
            return next.run(request).await;
        }
        debug!(target: "auth", %path, "API key authentication failed");
        return unauthorized_response().into_response();
    }

    if let Some(token) = extract_form_session_token(request.headers()) {
        if let Some(permission_level) = validate_form_session_and_touch(&token).await {
            if !permission_allows_request(permission_level, &method, &path) {
                debug!(target: "auth", %path, "forms session authentication denied by permission level");
                return permission_denied_response().into_response();
            }
            debug!(target: "auth", %path, "forms session authentication successful");
            return next.run(request).await;
        }
        debug!(target: "auth", %path, "forms session authentication failed");
        return unauthorized_response().into_response();
    }

    debug!(
        target: "auth",
        %path,
        "missing authentication credentials (expected Basic auth, API key, or forms session cookie)"
    );
    unauthorized_response().into_response()
}

#[cfg(test)]
mod tests {
    use super::{
        constant_time_eq, extract_api_key, extract_basic_credentials, extract_form_session_token,
        permission_allows_request, validate_form_session_and_touch, MAX_CREDENTIAL_BYTES,
    };
    use axum::{
        body::Body,
        extract::State,
        http::{HeaderMap, HeaderValue, Method, Request, StatusCode},
        Json,
    };
    use chorrosion_application::AppState;
    use chorrosion_config::{AppConfig, PermissionLevel};
    use chorrosion_infrastructure::sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
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

    #[test]
    fn constant_time_eq_returns_true_for_equal_slices() {
        assert!(constant_time_eq(b"secret-key", b"secret-key"));
    }

    #[test]
    fn constant_time_eq_returns_false_for_different_content() {
        assert!(!constant_time_eq(b"secret-key-1", b"secret-key-2"));
    }

    #[test]
    fn constant_time_eq_returns_false_for_different_lengths() {
        assert!(!constant_time_eq(b"short", b"shorter"));
    }

    #[test]
    fn constant_time_eq_handles_empty_slices() {
        assert!(constant_time_eq(b"", b""));
        assert!(!constant_time_eq(b"", b"nonempty"));
        assert!(!constant_time_eq(b"nonempty", b""));
    }

    #[test]
    fn constant_time_eq_truncates_oversized_input() {
        // Inputs longer than MAX_CREDENTIAL_BYTES are truncated.
        // Two inputs that are identical in the first MAX_CREDENTIAL_BYTES bytes but differ
        // after the cutoff must compare as equal (the tail is ignored).
        let mut long_a: Vec<u8> = vec![b'x'; MAX_CREDENTIAL_BYTES + 10];
        let mut long_b: Vec<u8> = vec![b'x'; MAX_CREDENTIAL_BYTES + 20];
        // Make sure they differ after MAX_CREDENTIAL_BYTES but are identical up to it.
        long_a[MAX_CREDENTIAL_BYTES + 5] = b'A';
        long_b[MAX_CREDENTIAL_BYTES + 15] = b'B';
        assert!(constant_time_eq(&long_a, &long_b));

        // Inputs that differ BEFORE MAX_CREDENTIAL_BYTES must still compare as unequal.
        let mut diff_a: Vec<u8> = vec![b'x'; MAX_CREDENTIAL_BYTES + 5];
        let mut diff_b: Vec<u8> = vec![b'x'; MAX_CREDENTIAL_BYTES + 5];
        diff_a[10] = b'A';
        diff_b[10] = b'B';
        assert!(!constant_time_eq(&diff_a, &diff_b));
    }

    #[test]
    fn read_only_permission_blocks_mutating_requests_except_logout() {
        assert!(permission_allows_request(
            PermissionLevel::ReadOnly,
            &Method::GET,
            "/api/v1/system/status"
        ));
        assert!(!permission_allows_request(
            PermissionLevel::ReadOnly,
            &Method::POST,
            "/api/v1/artists"
        ));
        assert!(permission_allows_request(
            PermissionLevel::ReadOnly,
            &Method::POST,
            "/api/v1/auth/forms/logout"
        ));
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

    #[tokio::test]
    async fn middleware_allows_valid_forms_session_cookie_when_configured() {
        let _lock = crate::handlers::auth::auth_test_mutex().lock().await;
        crate::handlers::auth::clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let login_request = Request::builder()
            .uri("/api/v1/auth/forms/login")
            .method("POST")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from("username=user&password=pass"))
            .expect("login request");

        let login_response = app
            .clone()
            .oneshot(login_request)
            .await
            .expect("login response");
        assert_eq!(login_response.status(), StatusCode::OK);
        let set_cookie = login_response
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .expect("set-cookie should exist");
        let cookie_pair = set_cookie
            .split(';')
            .next()
            .expect("cookie pair should exist");

        let mut cookie_headers = HeaderMap::new();
        cookie_headers.insert(
            "Cookie",
            HeaderValue::from_str(cookie_pair).expect("cookie"),
        );
        let token = extract_form_session_token(&cookie_headers).expect("token from cookie");
        assert_eq!(
            validate_form_session_and_touch(&token).await,
            Some(PermissionLevel::Admin),
            "token from login should exist in session store"
        );

        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .header("Cookie", cookie_pair)
            .body(Body::empty())
            .expect("request");

        assert_eq!(
            extract_form_session_token(request.headers()).as_deref(),
            Some(token.as_str())
        );

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_rejects_invalid_forms_session_cookie() {
        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .header("Cookie", "chorrosion_session=invalid")
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn middleware_rejects_mutation_when_basic_auth_is_read_only() {
        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        config.auth.basic_permission_level = PermissionLevel::ReadOnly;
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let request = Request::builder()
            .uri("/api/v1/artists")
            .method("POST")
            .header("Authorization", "Basic dXNlcjpwYXNz")
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"name":"blocked"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn middleware_allows_read_only_api_key_for_get_requests() {
        let _lock = crate::handlers::auth::auth_test_mutex().lock().await;
        crate::handlers::auth::clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("admin".to_string());
        config.auth.basic_password = Some("secret".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state.clone());
        let (status, Json(created)) = crate::handlers::auth::create_api_key(
            State(state),
            Json(crate::handlers::auth::CreateApiKeyRequest {
                name: Some("viewer".to_string()),
                permission_level: Some(PermissionLevel::ReadOnly),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        let request = Request::builder()
            .uri("/api/v1/system/status")
            .method("GET")
            .header("X-Api-Key", created.key)
            .body(Body::empty())
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn middleware_rejects_mutation_when_api_key_is_read_only() {
        let _lock = crate::handlers::auth::auth_test_mutex().lock().await;
        crate::handlers::auth::clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("admin".to_string());
        config.auth.basic_password = Some("secret".to_string());
        let state = make_test_state(config).await;

        let app = crate::router(state.clone());
        let (status, Json(created)) = crate::handlers::auth::create_api_key(
            State(state),
            Json(crate::handlers::auth::CreateApiKeyRequest {
                name: Some("viewer".to_string()),
                permission_level: Some(PermissionLevel::ReadOnly),
            }),
        )
        .await;
        assert_eq!(status, StatusCode::CREATED);

        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            let request = Request::builder()
                .uri("/api/v1/artists")
                .method(method.clone())
                .header("X-Api-Key", created.key.clone())
                .header("Content-Type", "application/json")
                .body(Body::from(r#"{"name":"blocked"}"#))
                .expect("request");

            let response = app.clone().oneshot(request).await.expect("response");
            assert_eq!(
                response.status(),
                StatusCode::FORBIDDEN,
                "{method} should be forbidden for read-only API key"
            );
        }
    }

    #[tokio::test]
    async fn middleware_rejects_mutation_when_forms_session_is_read_only() {
        let _lock = crate::handlers::auth::auth_test_mutex().lock().await;
        crate::handlers::auth::clear_stores_for_tests().await;

        let mut config = AppConfig::default();
        config.auth.basic_username = Some("user".to_string());
        config.auth.basic_password = Some("pass".to_string());
        config.auth.basic_permission_level = PermissionLevel::ReadOnly;
        let state = make_test_state(config).await;

        let app = crate::router(state);
        let login_request = Request::builder()
            .uri("/api/v1/auth/forms/login")
            .method("POST")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(Body::from("username=user&password=pass"))
            .expect("login request");

        let login_response = app
            .clone()
            .oneshot(login_request)
            .await
            .expect("login response");
        assert_eq!(login_response.status(), StatusCode::OK);
        let set_cookie = login_response
            .headers()
            .get("set-cookie")
            .and_then(|v| v.to_str().ok())
            .expect("set-cookie should exist");
        let cookie_pair = set_cookie
            .split(';')
            .next()
            .expect("cookie pair should exist");

        let request = Request::builder()
            .uri("/api/v1/artists")
            .method("POST")
            .header("Cookie", cookie_pair)
            .header("Content-Type", "application/json")
            .body(Body::from(r#"{"name":"blocked"}"#))
            .expect("request");

        let response = app.oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
    }
}

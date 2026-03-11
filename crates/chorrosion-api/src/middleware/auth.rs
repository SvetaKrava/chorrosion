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
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().clone();
    let state = request
        .extensions()
        .get::<chorrosion_application::AppState>()
        .cloned();

    // Bootstrap bypass: allow POST /api/v1/auth/api-keys only when no keys exist yet,
    // so the first key can be created without requiring prior authentication.
    if method == Method::POST
        && path.strip_prefix(API_V1_BASE) == Some("/auth/api-keys")
        && api_key_count().await == 0
    {
        debug!(target: "auth", %path, "auth bootstrap: no keys exist, allowing first key creation");
        return next.run(request).await;
    }

    if let Some(state) = state {
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

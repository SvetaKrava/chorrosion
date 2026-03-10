// SPDX-License-Identifier: GPL-3.0-or-later
use crate::handlers::auth::validate_api_key_and_touch;
use axum::{
    extract::Request,
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::debug;

fn is_auth_bootstrap_path(path: &str) -> bool {
    path == "/api/v1/auth/api-keys" || path.starts_with("/api/v1/auth/api-keys/")
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
            if let Some(rest) = auth_str.strip_prefix("Bearer ") {
                let trimmed = rest.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
    }

    None
}

/// API key authentication middleware.
pub async fn auth_middleware(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_string();
    if is_auth_bootstrap_path(&path) {
        debug!(target: "auth", %path, "auth bootstrap path bypassed");
        return next.run(request).await;
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

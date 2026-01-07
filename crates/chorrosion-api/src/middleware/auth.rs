use axum::{
    extract::Request,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
};
use tracing::debug;

/// Authentication middleware stub - validates API key or bearer token
pub async fn auth_middleware(headers: HeaderMap, request: Request, next: Next) -> Response {
    // Check for API key header or Authorization bearer token
    if let Some(api_key) = headers.get("X-Api-Key") {
        debug!(target: "auth", "API key authentication: {:?}", api_key.to_str().ok());
        // TODO: Validate against stored API keys in database
        return next.run(request).await;
    }

    if let Some(auth_header) = headers.get("Authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str.starts_with("Bearer ") {
                debug!(target: "auth", "Bearer token authentication");
                // TODO: Validate JWT or session token
                return next.run(request).await;
            }
        }
    }

    // For now, allow all requests (stub mode)
    // TODO: Uncomment to enforce authentication
    // (StatusCode::UNAUTHORIZED, "Missing or invalid authentication").into_response()

    debug!(target: "auth", "No authentication provided, allowing request (stub mode)");
    next.run(request).await
}

/// Response for unauthorized requests
pub async fn unauthorized() -> impl IntoResponse {
    (StatusCode::UNAUTHORIZED, "Unauthorized")
}

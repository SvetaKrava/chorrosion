// SPDX-License-Identifier: GPL-3.0-or-later

//! Axum middleware that caches successful GET responses.
//!
//! Only `2xx` responses to `GET` requests are cached.  All other methods and
//! non-success responses are passed through unchanged.  The cache key is the
//! full request URI (path + query string).
//!
//! Wire this inside the auth layer so that only authenticated responses are
//! cached:
//!
//! ```text
//! Request → auth_middleware → response_cache_middleware → handler
//! ```

use axum::{
    body::Body,
    extract::Request,
    http::{Method, StatusCode},
    middleware::Next,
    response::Response,
};
use chorrosion_application::AppState;
use chorrosion_infrastructure::CachedResponse;
use tracing::{debug, error};

fn is_cacheable_get_path(path: &str) -> bool {
    !path.contains("/events") && !path.ends_with("/calendar/ical")
}

/// Returns `true` for methods that mutate server state and should therefore
/// trigger a full cache invalidation on success.
fn is_mutating(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

/// Middleware function — register with
/// `axum_middleware::from_fn_with_state(state.clone(), response_cache_middleware)`.
pub async fn response_cache_middleware(
    axum::extract::State(state): axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    if method != Method::GET {
        let response = next.run(req).await;
        if is_mutating(&method) && state.response_cache.is_enabled() && response.status().is_success() {
            debug!(target: "cache", method = %method, path = %path, "invalidating API response cache after write");
            state.response_cache.invalidate_all();
        }
        return response;
    }

    // Skip the cache entirely when it is disabled.
    if !state.response_cache.is_enabled() || !is_cacheable_get_path(&path) {
        return next.run(req).await;
    }

    let key = {
        let uri = req.uri();
        match uri.query() {
            Some(q) => format!("{}?{}", uri.path(), q),
            None => uri.path().to_string(),
        }
    };

    // --- Cache HIT ---
    if let Some(cached) = state.response_cache.get(&key) {
        debug!(target: "cache", key = %key, "API response cache HIT");
        let status = StatusCode::from_u16(cached.status).unwrap_or(StatusCode::OK);
        let mut builder = Response::builder().status(status);
        for (name, value) in &cached.headers {
            builder = builder.header(name.as_slice(), value.as_slice());
        }
        return builder
            .header("x-cache", "HIT")
            .body(Body::from(cached.body))
            .unwrap_or_else(|_| {
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            });
    }

    // --- Cache MISS: run the handler ---
    let response = next.run(req).await;

    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);

    if response.status().is_success()
        && content_type
            .as_deref()
            .is_some_and(|value| value.starts_with("application/json"))
    {
        let (parts, body) = response.into_parts();

        let body_limit = state.config.cache.api_response_max_body_bytes;

        // If Content-Length is present and exceeds the configured limit, skip caching
        // and pass the response through immediately — the body stream has not been
        // consumed yet so the caller receives the full response unchanged.
        let content_length = parts
            .headers
            .get(axum::http::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<usize>().ok());
        if let Some(content_len) = content_length {
            if content_len > body_limit {
                debug!(
                    target: "cache",
                    key = %key,
                    content_length = content_len,
                    limit = body_limit,
                    "skipping cache: response body exceeds size limit"
                );
                return Response::from_parts(parts, body);
            }
        }

        match axum::body::to_bytes(body, body_limit).await {
            Ok(bytes) => {
                debug!(
                    target: "cache",
                    key = %key,
                    bytes = bytes.len(),
                    "API response cache MISS → stored"
                );
                let headers = parts
                    .headers
                    .iter()
                    .map(|(k, v)| (k.as_str().as_bytes().to_vec(), v.as_bytes().to_vec()))
                    .collect();
                state.response_cache.insert(
                    key,
                    CachedResponse {
                        status: parts.status.as_u16(),
                        headers,
                        body: bytes.clone(),
                    },
                );
                Response::from_parts(parts, Body::from(bytes))
            }
            Err(e) => {
                error!(
                    target: "cache",
                    key = %key,
                    error = %e,
                    "failed to collect response body for caching"
                );
                Response::builder()
                    .status(StatusCode::INTERNAL_SERVER_ERROR)
                    .body(Body::empty())
                    .unwrap()
            }
        }
    } else {
        response
    }
}

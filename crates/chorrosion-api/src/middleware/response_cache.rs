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
use tracing::debug;

fn is_cacheable_get_path(path: &str) -> bool {
    !path.contains("/events") && !path.ends_with("/calendar/ical")
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
        if state.response_cache.is_enabled() && response.status().is_success() {
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
    if let Some(cached_bytes) = state.response_cache.get(&key) {
        debug!(target: "cache", key = %key, "API response cache HIT");
        return Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "application/json")
            .header("x-cache", "HIT")
            .body(Body::from(cached_bytes))
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
        match axum::body::to_bytes(body, 16 * 1024 * 1024).await {
            Ok(bytes) => {
                debug!(
                    target: "cache",
                    key = %key,
                    bytes = bytes.len(),
                    "API response cache MISS → stored"
                );
                state.response_cache.insert(key, bytes.clone());
                Response::from_parts(parts, Body::from(bytes))
            }
            // Body collection failed (shouldn't happen for JSON handlers) — pass
            // through an empty response rather than panicking.
            Err(_) => Response::from_parts(parts, Body::empty()),
        }
    } else {
        response
    }
}

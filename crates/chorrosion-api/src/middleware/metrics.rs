// SPDX-License-Identifier: GPL-3.0-or-later

use axum::{
    body::Body,
    extract::{MatchedPath, Request},
    http::{header, HeaderValue, Method, StatusCode},
    middleware::Next,
    response::Response,
};
use prometheus::{Encoder, HistogramOpts, HistogramVec, IntCounterVec, Registry, TextEncoder};
use std::sync::{Arc, OnceLock};
use std::time::Instant;

struct HttpMetrics {
    registry: Registry,
    request_count: IntCounterVec,
    request_duration_seconds: HistogramVec,
}

impl HttpMetrics {
    fn new() -> Self {
        let registry = Registry::new();
        let request_count = IntCounterVec::new(
            prometheus::Opts::new(
                "chorrosion_http_requests_total",
                "Total number of HTTP requests handled by Chorrosion",
            ),
            &["method", "path", "status"],
        )
        .expect("request counter should be created");
        let request_duration_seconds = HistogramVec::new(
            HistogramOpts::new(
                "chorrosion_http_request_duration_seconds",
                "HTTP request duration in seconds for Chorrosion endpoints",
            ),
            &["method", "path", "status"],
        )
        .expect("request duration histogram should be created");

        registry
            .register(Box::new(request_count.clone()))
            .expect("request counter should be registered");
        registry
            .register(Box::new(request_duration_seconds.clone()))
            .expect("request duration histogram should be registered");

        Self {
            registry,
            request_count,
            request_duration_seconds,
        }
    }

    fn observe(&self, method: &Method, path: &str, status: StatusCode, duration_seconds: f64) {
        let labels = [method.as_str(), path, status.as_str()];
        self.request_count.with_label_values(&labels).inc();
        self.request_duration_seconds
            .with_label_values(&labels)
            .observe(duration_seconds);
    }

    fn render(&self) -> Result<String, StatusCode> {
        let metric_families = self.registry.gather();
        let mut buffer = Vec::new();
        TextEncoder::new()
            .encode(&metric_families, &mut buffer)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        String::from_utf8(buffer).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

fn metrics() -> &'static Arc<HttpMetrics> {
    static METRICS: OnceLock<Arc<HttpMetrics>> = OnceLock::new();
    METRICS.get_or_init(|| Arc::new(HttpMetrics::new()))
}

pub async fn metrics_handler() -> Response {
    match metrics().render() {
        Ok(body) => Response::builder()
            .status(StatusCode::OK)
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain; version=0.0.4; charset=utf-8"),
            )
            .body(Body::from(body))
            .expect("metrics response should be buildable"),
        Err(status) => Response::builder()
            .status(status)
            .body(Body::from("failed to render metrics"))
            .expect("error response should be buildable"),
    }
}

pub async fn metrics_middleware(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req
        .extensions()
        .get::<MatchedPath>()
        .map(MatchedPath::as_str)
        .unwrap_or_else(|| req.uri().path())
        .to_owned();
    let started_at = Instant::now();

    let response = next.run(req).await;
    let status = response.status();
    let duration_seconds = started_at.elapsed().as_secs_f64();

    metrics().observe(&method, &path, status, duration_seconds);

    response
}

#[cfg(test)]
mod tests {
    use super::{metrics_handler, metrics_middleware};
    use axum::{body::to_bytes, http::Request, routing::get, Router};
    use tower::util::ServiceExt;

    async fn ok_handler() -> &'static str {
        "ok"
    }

    #[tokio::test]
    async fn metrics_endpoint_returns_prometheus_text() {
        // Ensure at least one observation exists before scraping; the Prometheus
        // text encoder only emits metric families that have at least one sample,
        // so the test must not rely on other (potentially parallel) tests having
        // populated the global registry first.
        let setup_app = Router::new()
            .route("/probe", get(ok_handler))
            .route_layer(axum::middleware::from_fn(metrics_middleware));
        setup_app
            .oneshot(
                Request::builder()
                    .uri("/probe")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        let response = metrics_handler().await;
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        let content_type = response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        assert!(content_type.starts_with("text/plain; version=0.0.4"));

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("metrics body should be readable");
        let text = String::from_utf8(body.to_vec()).expect("metrics body should be utf-8");
        assert!(text.contains("chorrosion_http_requests_total"));
        assert!(text.contains("chorrosion_http_request_duration_seconds"));
    }

    #[tokio::test]
    async fn middleware_records_metrics_for_matched_route() {
        let app = Router::new()
            .route("/metrics-test", get(ok_handler))
            .route_layer(axum::middleware::from_fn(metrics_middleware));

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics-test")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let metrics_response = metrics_handler().await;
        let body = to_bytes(metrics_response.into_body(), usize::MAX)
            .await
            .expect("metrics body should be readable");
        let text = String::from_utf8(body.to_vec()).expect("metrics body should be utf-8");

        assert!(text.contains(
            "chorrosion_http_requests_total{method=\"GET\",path=\"/metrics-test\",status=\"200\"}"
        ));
        assert!(
            text.contains("chorrosion_http_request_duration_seconds_bucket{method=\"GET\",path=\"/metrics-test\",status=\"200\"")
        );
    }

    #[tokio::test]
    async fn middleware_uses_matched_path_template_for_parameterized_routes() {
        async fn item_handler() -> &'static str {
            "item"
        }

        let app = Router::new()
            .route("/items/:id", get(item_handler))
            .route_layer(axum::middleware::from_fn(metrics_middleware));

        // Hit a concrete URL; the label must use the template, not the concrete value.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/items/42")
                    .method("GET")
                    .body(axum::body::Body::empty())
                    .expect("request should build"),
            )
            .await
            .expect("request should succeed");

        assert_eq!(response.status(), axum::http::StatusCode::OK);

        let metrics_response = metrics_handler().await;
        let body = to_bytes(metrics_response.into_body(), usize::MAX)
            .await
            .expect("metrics body should be readable");
        let text = String::from_utf8(body.to_vec()).expect("metrics body should be utf-8");

        // The label path must be the route template ("/items/:id"), not the concrete path ("/items/42").
        assert!(
            text.contains("path=\"/items/:id\""),
            "expected templated path label but got:\n{text}"
        );
        assert!(
            !text.contains("path=\"/items/42\""),
            "concrete path must not appear as a label:\n{text}"
        );
    }
}

// SPDX-License-Identifier: GPL-3.0-or-later

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
    Router,
};
use chorrosion_api::router;
use chorrosion_application::AppState;
use chorrosion_config::AppConfig;
use chorrosion_domain::Artist;
use chorrosion_infrastructure::{
    init_database,
    repositories::Repository,
    sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteDuplicateRepository, SqliteIndexerDefinitionRepository,
        SqliteMetadataProfileRepository, SqliteQualityProfileRepository,
        SqliteSmartPlaylistRepository, SqliteTagRepository, SqliteTaggedEntityRepository,
        SqliteTrackRepository,
    },
    ResponseCache,
};
use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;
use tower::util::ServiceExt;

const BASIC_AUTH_HEADER: &str = "Basic YWRtaW46c2VjcmV0";

async fn setup_pool() -> SqlitePool {
    let mut config = AppConfig::default();
    config.database.url = "sqlite://:memory:".to_string();
    config.database.pool_max_size = 1;

    init_database(&config)
        .await
        .expect("init in-memory sqlite with migrations and PRAGMA foreign_keys = ON")
}

fn make_state(pool: SqlitePool) -> AppState {
    let mut config = AppConfig::default();
    config.auth.basic_username = Some("admin".to_string());
    config.auth.basic_password = Some("secret".to_string());

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
        Arc::new(SqliteSmartPlaylistRepository::new(pool.clone())),
        Arc::new(SqliteDuplicateRepository::new(pool.clone())),
        ResponseCache::new(1_000, 0),
    )
}

async fn request_json(app: Router, method: Method, uri: &str, body: Option<Value>) -> StatusCode {
    let body_bytes = body
        .map(|payload| serde_json::to_vec(&payload).expect("serialize payload"))
        .unwrap_or_default();

    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", BASIC_AUTH_HEADER)
        .header("content-type", "application/json")
        .body(Body::from(body_bytes))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");
    let status = response.status();

    let _ = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");

    status
}

async fn request_json_value(app: Router, method: Method, uri: &str, body: Option<Value>) -> Value {
    let body_bytes = body
        .map(|payload| serde_json::to_vec(&payload).expect("serialize payload"))
        .unwrap_or_default();

    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("Authorization", BASIC_AUTH_HEADER)
        .header("content-type", "application/json")
        .body(Body::from(body_bytes))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");

    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");

    if status != StatusCode::CREATED {
        panic!(
            "expected 201 CREATED but got {status} with body: {}",
            String::from_utf8_lossy(&bytes)
        );
    }

    serde_json::from_slice(&bytes).expect("response should be valid JSON")
}

fn benchmark_health_endpoint(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let app = runtime.block_on(async {
        let pool = setup_pool().await;
        let state = make_state(pool);
        router(state)
    });

    c.bench_function("api/health", |b| {
        b.to_async(&runtime).iter(|| async {
            let status = request_json(app.clone(), Method::GET, "/health", None).await;
            assert_eq!(status, StatusCode::OK);
        });
    });
}

fn benchmark_list_artists_with_seed_data(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");
    let app = runtime.block_on(async {
        let pool = setup_pool().await;

        let artist_repo = SqliteArtistRepository::new(pool.clone());
        for i in 0..100 {
            let artist = Artist::new(format!("Bench Artist {i:03}"));
            artist_repo.create(artist).await.expect("seed artist");
        }

        let state = make_state(pool);
        router(state)
    });

    c.bench_function("api/list_artists_100_seeded", |b| {
        b.to_async(&runtime).iter(|| async {
            let status = request_json(
                app.clone(),
                Method::GET,
                "/api/v1/artists?limit=50&offset=0",
                None,
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        });
    });
}

fn benchmark_create_artist_album_track_workflow(c: &mut Criterion) {
    let runtime = Runtime::new().expect("tokio runtime");

    let mut group = c.benchmark_group("api/create_workflow");
    group.bench_with_input(
        BenchmarkId::new("artist_album_track", "sequential"),
        &0,
        |b, _| {
            b.to_async(&runtime).iter_custom(|iters| async move {
                let mut total = Duration::ZERO;

                for _ in 0..iters {
                    // Setup (not measured): fresh DB + router per iteration to prevent
                    // unbounded table growth from skewing later-iteration timings.
                    let pool = setup_pool().await;
                    let state = make_state(pool);
                    let app = router(state);

                    let start = Instant::now();

                    let artist_payload = request_json_value(
                        app.clone(),
                        Method::POST,
                        "/api/v1/artists",
                        Some(json!({ "name": "Bench Workflow Artist" })),
                    )
                    .await;
                    let artist_id = artist_payload["id"]
                        .as_str()
                        .expect("artist id should exist")
                        .to_string();

                    let album_payload = request_json_value(
                        app.clone(),
                        Method::POST,
                        "/api/v1/albums",
                        Some(json!({
                            "artist_id": artist_id,
                            "title": "Bench Album"
                        })),
                    )
                    .await;
                    let album_id = album_payload["id"]
                        .as_str()
                        .expect("album id should exist")
                        .to_string();

                    let track_status = request_json(
                        app.clone(),
                        Method::POST,
                        "/api/v1/tracks",
                        Some(json!({
                            "album_id": album_id,
                            "artist_id": artist_id,
                            "title": "Bench Track"
                        })),
                    )
                    .await;
                    assert_eq!(track_status, StatusCode::CREATED);

                    total += start.elapsed();
                }

                total
            });
        },
    );
    group.finish();
}

criterion_group!(
    api_benches,
    benchmark_health_endpoint,
    benchmark_list_artists_with_seed_data,
    benchmark_create_artist_album_track_workflow
);
criterion_main!(api_benches);

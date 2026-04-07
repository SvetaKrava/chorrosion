// SPDX-License-Identifier: GPL-3.0-or-later

use axum::{
    body::{to_bytes, Body},
    http::{Method, Request, StatusCode},
};
use chorrosion_api::router;
use chorrosion_application::AppState;
use chorrosion_config::AppConfig;
use chorrosion_infrastructure::{
    sqlite_adapters::{
        SqliteAlbumRepository, SqliteArtistRepository, SqliteDownloadClientDefinitionRepository,
        SqliteIndexerDefinitionRepository, SqliteMetadataProfileRepository,
        SqliteQualityProfileRepository, SqliteTrackRepository,
    },
    ResponseCache,
};
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::sync::Arc;
use tower::util::ServiceExt;

async fn setup_pool() -> SqlitePool {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");

    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrate");

    pool
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
        Arc::new(SqliteDownloadClientDefinitionRepository::new(pool)),
        ResponseCache::new(100, 60),
    )
}

async fn request_json(
    app: axum::Router,
    method: Method,
    uri: &str,
    body: Option<Value>,
    authorization: Option<&str>,
) -> (StatusCode, Value) {
    let body_bytes = body
        .map(|payload| serde_json::to_vec(&payload).expect("serialize payload"))
        .unwrap_or_default();

    let mut builder = Request::builder().method(method).uri(uri);
    if authorization.is_some() {
        builder = builder.header(
            "Authorization",
            authorization.expect("authorization header present"),
        );
    }

    let request = builder
        .header("content-type", "application/json")
        .body(Body::from(body_bytes))
        .expect("request should build");

    let response = app.oneshot(request).await.expect("request should complete");
    let status = response.status();
    let bytes = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body should be readable");

    let payload = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice::<Value>(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).to_string()))
    };

    (status, payload)
}

#[tokio::test]
async fn full_artist_album_track_api_workflow() {
    let pool = setup_pool().await;
    let state = make_state(pool);
    let app = router(state);
    let basic_auth = "Basic YWRtaW46c2VjcmV0";

    let (status, artist_payload) = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/artists",
        Some(json!({ "name": "Workflow Artist" })),
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let artist_id = artist_payload["id"]
        .as_str()
        .expect("artist id should be present")
        .to_string();

    let (status, album_payload) = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/albums",
        Some(json!({
            "artist_id": artist_id,
            "title": "Workflow Album"
        })),
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let album_id = album_payload["id"]
        .as_str()
        .expect("album id should be present")
        .to_string();

    let (status, track_payload) = request_json(
        app.clone(),
        Method::POST,
        "/api/v1/tracks",
        Some(json!({
            "album_id": album_id,
            "artist_id": artist_payload["id"],
            "title": "Workflow Track"
        })),
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let track_id = track_payload["id"]
        .as_str()
        .expect("track id should be present")
        .to_string();

    let (status, fetched_artist) = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/artists/{artist_id}"),
        None,
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched_artist["name"], json!("Workflow Artist"));

    let (status, fetched_album) = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/albums/{album_id}"),
        None,
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched_album["title"], json!("Workflow Album"));

    let (status, fetched_track) = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/tracks/{track_id}"),
        None,
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(fetched_track["title"], json!("Workflow Track"));

    let (status, albums_by_artist) = request_json(
        app.clone(),
        Method::GET,
        &format!("/api/v1/artists/{artist_id}/albums"),
        None,
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(albums_by_artist["total"], json!(1));

    let (status, tracks_by_album) = request_json(
        app,
        Method::GET,
        &format!("/api/v1/albums/{album_id}/tracks"),
        None,
        Some(basic_auth),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(tracks_by_album["total"], json!(1));
}

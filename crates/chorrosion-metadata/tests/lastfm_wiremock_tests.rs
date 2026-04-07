// SPDX-License-Identifier: GPL-3.0-or-later

use chorrosion_metadata::lastfm::{LastFmClient, LastFmError};
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn fetch_artist_metadata_from_wiremock_server() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "artist": {
            "name": "Test Artist",
            "bio": {
                "summary": "Test artist bio"
            },
            "tags": {
                "tag": [
                    { "name": "rock" },
                    { "name": "indie" }
                ]
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/2.0/"))
        .and(query_param("method", "artist.getinfo"))
        .and(query_param("artist", "Test Artist"))
        .and(query_param("api_key", "test_api_key"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some(format!("{}/2.0/", server.uri())),
    );
    let metadata = client
        .fetch_artist_metadata("Test Artist")
        .await
        .expect("artist metadata should parse");

    assert_eq!(metadata.name, "Test Artist");
    assert_eq!(metadata.bio.as_deref(), Some("Test artist bio"));
    assert_eq!(
        metadata.tags,
        Some(vec!["rock".to_string(), "indie".to_string()])
    );
}

#[tokio::test]
async fn fetch_album_metadata_from_wiremock_server() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "album": {
            "name": "Test Album",
            "artist": "Test Artist",
            "tracks": {
                "track": [
                    { "name": "Track 1" },
                    { "name": "Track 2" },
                    { "name": "Track 3" }
                ]
            }
        }
    });

    Mock::given(method("GET"))
        .and(path("/2.0/"))
        .and(query_param("method", "album.getinfo"))
        .and(query_param("artist", "Test Artist"))
        .and(query_param("album", "Test Album"))
        .and(query_param("api_key", "test_api_key"))
        .and(query_param("format", "json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some(format!("{}/2.0/", server.uri())),
    );
    let metadata = client
        .fetch_album_metadata("Test Artist", "Test Album")
        .await
        .expect("album metadata should parse");

    assert_eq!(metadata.title, "Test Album");
    assert_eq!(metadata.artist, "Test Artist");
    assert_eq!(
        metadata.tracks,
        Some(vec![
            "Track 1".to_string(),
            "Track 2".to_string(),
            "Track 3".to_string()
        ])
    );
}

#[tokio::test]
async fn fetch_artist_metadata_maps_lastfm_api_error() {
    let server = MockServer::start().await;

    let body = serde_json::json!({
        "error": 6,
        "message": "Artist not found"
    });

    Mock::given(method("GET"))
        .and(path("/2.0/"))
        .and(query_param("method", "artist.getinfo"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some(format!("{}/2.0/", server.uri())),
    );
    let error = client
        .fetch_artist_metadata("Missing Artist")
        .await
        .expect_err("api error should be returned");

    match error {
        LastFmError::Api { code, message } => {
            assert_eq!(code, 6);
            assert_eq!(message, "Artist not found");
        }
        other => panic!("expected LastFmError::Api, got {other}"),
    }
}

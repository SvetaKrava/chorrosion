use chorrosion_metadata::lyrics::{LyricsClient, LyricsError};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_fetch_lyrics_success() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Nirvana/Smells%20Like%20Teen%20Spirit"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "lyrics": "Load up on guns, bring your friends"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client
        .fetch_lyrics("Nirvana", "Smells Like Teen Spirit")
        .await;

    assert!(result.is_ok());
    let metadata = result.unwrap();
    assert_eq!(metadata.artist, "Nirvana");
    assert_eq!(metadata.title, "Smells Like Teen Spirit");
    assert_eq!(metadata.lyrics, "Load up on guns, bring your friends");
}

#[tokio::test]
async fn test_fetch_lyrics_caches_result() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Daft%20Punk/One%20More%20Time"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "lyrics": "One more time"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());

    let first = client.fetch_lyrics("Daft Punk", "One More Time").await;
    let second = client.fetch_lyrics("Daft Punk", "One More Time").await;

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(first.unwrap().lyrics, "One more time");
    assert_eq!(second.unwrap().lyrics, "One more time");
}

#[tokio::test]
async fn test_fetch_lyrics_handles_http_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Test%20Artist/Test%20Song"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client.fetch_lyrics("Test Artist", "Test Song").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LyricsError::HttpStatus { status, body } => {
            assert_eq!(status.as_u16(), 429);
            assert!(body.contains("rate limited"));
        }
        other => panic!("expected HttpStatus error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fetch_lyrics_handles_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Unknown%20Artist/Unknown%20Song"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "No lyrics found"
        })))
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client.fetch_lyrics("Unknown Artist", "Unknown Song").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LyricsError::Api { .. }));
}

#[tokio::test]
async fn test_fetch_lyrics_handles_invalid_json() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Invalid/Json"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{ invalid json"))
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client.fetch_lyrics("Invalid", "Json").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        LyricsError::Deserialization(_)
    ));
}

#[tokio::test]
async fn test_fetch_lyrics_missing_lyrics_field() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Empty%20Artist/Empty%20Song"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client.fetch_lyrics("Empty Artist", "Empty Song").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        LyricsError::MissingField("lyrics")
    ));
}

#[tokio::test]
async fn test_fetch_lyrics_blank_lyrics_field() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/v1/Blank%20Artist/Blank%20Song"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "lyrics": "   "
        })))
        .mount(&server)
        .await;

    let client = LyricsClient::new_with_base_url(server.uri());
    let result = client.fetch_lyrics("Blank Artist", "Blank Song").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        LyricsError::MissingField("lyrics")
    ));
}

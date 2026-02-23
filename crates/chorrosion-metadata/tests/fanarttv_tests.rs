use chorrosion_metadata::fanarttv::{FanartTvClient, FanartTvError};
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_fetch_artist_artwork() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/artist-mbid-1"))
        .and(header("api-key", "fanart-api-key"))
        .and(header("client-key", "fanart-client-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "artistbackground": [{ "url": "https://img/bg1.jpg", "likes": "12" }],
            "hdmusiclogo": [{ "url": "https://img/logo1.png", "likes": "4" }],
            "artistthumb": [{ "url": "https://img/thumb1.jpg" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = FanartTvClient::new(
        "fanart-api-key".to_string(),
        Some("fanart-client-key".to_string()),
        Some(server.uri()),
    );

    let artwork = client.fetch_artist_artwork("artist-mbid-1").await;
    assert!(artwork.is_ok());

    let artwork = artwork.unwrap();
    assert_eq!(artwork.backgrounds.len(), 1);
    assert_eq!(artwork.backgrounds[0].url, "https://img/bg1.jpg");
    assert_eq!(artwork.backgrounds[0].likes, Some(12));
    assert_eq!(artwork.logos.len(), 1);
    assert_eq!(artwork.logos[0].url, "https://img/logo1.png");
    assert_eq!(artwork.logos[0].likes, Some(4));
    assert_eq!(artwork.thumbs.len(), 1);
}

#[tokio::test]
async fn test_fetch_album_artwork() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/release-group-mbid-1"))
        .and(header("api-key", "fanart-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "albumcover": [{ "url": "https://img/cover1.jpg", "likes": "8" }],
            "cdart": [{ "url": "https://img/cdart1.png" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = FanartTvClient::new("fanart-api-key".to_string(), None, Some(server.uri()));

    let artwork = client.fetch_album_artwork("release-group-mbid-1").await;
    assert!(artwork.is_ok());

    let artwork = artwork.unwrap();
    assert_eq!(artwork.covers.len(), 1);
    assert_eq!(artwork.covers[0].url, "https://img/cover1.jpg");
    assert_eq!(artwork.covers[0].likes, Some(8));
    assert_eq!(artwork.cdarts.len(), 1);
    assert_eq!(artwork.cdarts[0].url, "https://img/cdart1.png");
}

#[tokio::test]
async fn test_fetch_artist_artwork_caches_result() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/cache-artist-mbid"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "artistbackground": [{ "url": "https://img/cache-bg.jpg" }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = FanartTvClient::new("fanart-api-key".to_string(), None, Some(server.uri()));

    let first = client.fetch_artist_artwork("cache-artist-mbid").await;
    let second = client.fetch_artist_artwork("cache-artist-mbid").await;

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(first.unwrap().backgrounds.len(), 1);
    assert_eq!(second.unwrap().backgrounds.len(), 1);
}

#[tokio::test]
async fn test_fetch_album_artwork_handles_http_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/release-group-mbid-error"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = FanartTvClient::new("fanart-api-key".to_string(), None, Some(server.uri()));

    let result = client.fetch_album_artwork("release-group-mbid-error").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        FanartTvError::HttpStatus { status, body } => {
            assert_eq!(status.as_u16(), 429);
            assert!(body.contains("rate limited"));
        }
        other => panic!("expected HttpStatus error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fetch_artist_artwork_handles_api_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/artist-mbid-error"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "error": "not found"
        })))
        .mount(&server)
        .await;

    let client = FanartTvClient::new("fanart-api-key".to_string(), None, Some(server.uri()));

    let result = client.fetch_artist_artwork("artist-mbid-error").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), FanartTvError::Api { .. }));
}

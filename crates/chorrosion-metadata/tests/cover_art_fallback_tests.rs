use chorrosion_metadata::cover_art_fallback::{
    CoverArtFallbackClient, CoverArtFallbackError, CoverArtProvider,
};
use chorrosion_metadata::fanarttv::FanartTvClient;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_fetch_album_cover_uses_fanart_first() {
    let fanart_server = MockServer::start().await;
    let cover_art_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/rg-1"))
        .and(header("api-key", "fanart-api-key"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "albumcover": [{ "url": "https://fanart.example/cover.jpg", "likes": "12" }],
            "cdart": []
        })))
        .expect(1)
        .mount(&fanart_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release-group/rg-1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": [{ "image": "https://coverart.example/front.jpg", "front": true }]
        })))
        .expect(0)
        .mount(&cover_art_server)
        .await;

    let fanart_client = FanartTvClient::new(
        "fanart-api-key".to_string(),
        None,
        Some(fanart_server.uri()),
    );

    let client = CoverArtFallbackClient::new(
        Some(fanart_client),
        Some(cover_art_server.uri()),
    );

    let result = client.fetch_album_cover("rg-1").await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.provider, CoverArtProvider::FanartTv);
    assert_eq!(result.image_url, "https://fanart.example/cover.jpg");
}

#[tokio::test]
async fn test_fetch_album_cover_falls_back_to_cover_art_archive() {
    let fanart_server = MockServer::start().await;
    let cover_art_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/rg-2"))
        .respond_with(ResponseTemplate::new(500).set_body_string("fanart down"))
        .expect(1)
        .mount(&fanart_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release-group/rg-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": [{
                "image": "https://coverart.example/front-full.jpg",
                "front": true,
                "thumbnails": {
                    "500": "https://coverart.example/front-500.jpg"
                }
            }]
        })))
        .expect(1)
        .mount(&cover_art_server)
        .await;

    let fanart_client = FanartTvClient::new(
        "fanart-api-key".to_string(),
        None,
        Some(fanart_server.uri()),
    );

    let client = CoverArtFallbackClient::new(
        Some(fanart_client),
        Some(cover_art_server.uri()),
    );

    let result = client.fetch_album_cover("rg-2").await;

    assert!(result.is_ok());
    let result = result.unwrap();
    assert_eq!(result.provider, CoverArtProvider::CoverArtArchive);
    assert_eq!(result.image_url, "https://coverart.example/front-500.jpg");
}

#[tokio::test]
async fn test_fetch_album_cover_result_is_cached() {
    let fanart_server = MockServer::start().await;
    let cover_art_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/rg-cache"))
        .respond_with(ResponseTemplate::new(500).set_body_string("fanart down"))
        .expect(1)
        .mount(&fanart_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release-group/rg-cache"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "images": [{ "image": "https://coverart.example/cache.jpg", "front": true }]
        })))
        .expect(1)
        .mount(&cover_art_server)
        .await;

    let fanart_client = FanartTvClient::new(
        "fanart-api-key".to_string(),
        None,
        Some(fanart_server.uri()),
    );

    let client = CoverArtFallbackClient::new(
        Some(fanart_client),
        Some(cover_art_server.uri()),
    );

    let first = client.fetch_album_cover("rg-cache").await;
    let second = client.fetch_album_cover("rg-cache").await;

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(first.unwrap().provider, CoverArtProvider::CoverArtArchive);
    assert_eq!(second.unwrap().provider, CoverArtProvider::CoverArtArchive);
}

#[tokio::test]
async fn test_fetch_album_cover_returns_error_when_all_providers_fail() {
    let fanart_server = MockServer::start().await;
    let cover_art_server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/music/albums/rg-fail"))
        .respond_with(ResponseTemplate::new(500).set_body_string("fanart down"))
        .expect(1)
        .mount(&fanart_server)
        .await;

    Mock::given(method("GET"))
        .and(path("/release-group/rg-fail"))
        .respond_with(ResponseTemplate::new(503).set_body_string("cover archive down"))
        .expect(1)
        .mount(&cover_art_server)
        .await;

    let fanart_client = FanartTvClient::new(
        "fanart-api-key".to_string(),
        None,
        Some(fanart_server.uri()),
    );

    let client = CoverArtFallbackClient::new(
        Some(fanart_client),
        Some(cover_art_server.uri()),
    );

    let result = client.fetch_album_cover("rg-fail").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        CoverArtFallbackError::ProvidersFailed(errors) => {
            assert_eq!(errors.len(), 2);
            assert!(errors.iter().any(|error| error.provider == CoverArtProvider::FanartTv));
            assert!(errors
                .iter()
                .any(|error| error.provider == CoverArtProvider::CoverArtArchive));
        }
        other => panic!("expected ProvidersFailed error, got: {other:?}"),
    }
}

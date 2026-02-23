use chorrosion_metadata::discogs::{DiscogsClient, DiscogsError};
use serde_json::json;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

#[tokio::test]
async fn test_fetch_artist_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .and(query_param("type", "artist"))
        .and(query_param("q", "Nirvana"))
        .and(header("authorization", "Discogs token=test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{
                "id": 42,
                "title": "Nirvana",
                "genre": ["Rock"],
                "style": ["Grunge"]
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/artists/42"))
        .and(header("authorization", "Discogs token=test-token"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Nirvana",
            "profile": "American rock band"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = DiscogsClient::new(Some("test-token".to_string()), Some(server.uri()));
    let metadata = client.fetch_artist_metadata("Nirvana").await;

    assert!(metadata.is_ok());
    let metadata = metadata.unwrap();
    assert_eq!(metadata.name, "Nirvana");
    assert_eq!(metadata.profile.as_deref(), Some("American rock band"));
    assert_eq!(metadata.genres.as_deref(), Some(&["Rock".to_string()][..]));
    assert_eq!(
        metadata.styles.as_deref(),
        Some(&["Grunge".to_string()][..])
    );
}

#[tokio::test]
async fn test_fetch_album_metadata() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .and(query_param("type", "release"))
        .and(query_param("artist", "Nirvana"))
        .and(query_param("release_title", "Nevermind"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{
                "id": 99,
                "title": "Nevermind",
                "year": 1991,
                "genre": ["Rock"],
                "style": ["Alternative Rock"],
                "artists": [{"name": "Nirvana"}]
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/releases/99"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "title": "Nevermind",
            "year": 1991,
            "genres": ["Rock"],
            "styles": ["Grunge"],
            "artists": [{"name": "Nirvana"}]
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));
    let metadata = client.fetch_album_metadata("Nirvana", "Nevermind").await;

    assert!(metadata.is_ok());
    let metadata = metadata.unwrap();
    assert_eq!(metadata.title, "Nevermind");
    assert_eq!(metadata.artist, "Nirvana");
    assert_eq!(metadata.year, Some(1991));
    assert_eq!(metadata.genres.as_deref(), Some(&["Rock".to_string()][..]));
    assert_eq!(
        metadata.styles.as_deref(),
        Some(&["Grunge".to_string()][..])
    );
}

#[tokio::test]
async fn test_fetch_artist_metadata_caches_result() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{
                "id": 777,
                "title": "Boards of Canada"
            }]
        })))
        .expect(1)
        .mount(&server)
        .await;

    Mock::given(method("GET"))
        .and(path("/artists/777"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "name": "Boards of Canada",
            "profile": "Scottish electronic music duo"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));

    let first = client.fetch_artist_metadata("Boards of Canada").await;
    let second = client.fetch_artist_metadata("Boards of Canada").await;

    assert!(first.is_ok());
    assert!(second.is_ok());
    assert_eq!(first.unwrap().name, "Boards of Canada");
    assert_eq!(second.unwrap().name, "Boards of Canada");
}

#[tokio::test]
async fn test_fetch_artist_metadata_handles_http_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(429).set_body_string("rate limited"))
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));
    let result = client.fetch_artist_metadata("Nirvana").await;

    assert!(result.is_err());
    match result.unwrap_err() {
        DiscogsError::HttpStatus { status, body } => {
            assert_eq!(status.as_u16(), 429);
            assert!(body.contains("rate limited"));
        }
        other => panic!("expected HttpStatus error, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_fetch_artist_metadata_empty_search_results() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": []
        })))
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));
    let result = client.fetch_artist_metadata("Unknown Artist").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DiscogsError::MissingField("results[0]")
    ));
}

#[tokio::test]
async fn test_fetch_artist_metadata_missing_id_in_result() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "results": [{ "title": "Some Artist" }]
        })))
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));
    let result = client.fetch_artist_metadata("Some Artist").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DiscogsError::MissingField("results[0].id")
    ));
}

#[tokio::test]
async fn test_fetch_artist_metadata_discogs_api_error_message() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "message": "Invalid consumer token."
        })))
        .mount(&server)
        .await;

    let client = DiscogsClient::new(Some("bad-token".to_string()), Some(server.uri()));
    let result = client.fetch_artist_metadata("Any Artist").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), DiscogsError::Api { .. }));
}

#[tokio::test]
async fn test_fetch_artist_metadata_deserialization_error() {
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/database/search"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{ invalid json"))
        .mount(&server)
        .await;

    let client = DiscogsClient::new(None, Some(server.uri()));
    let result = client.fetch_artist_metadata("Any Artist").await;

    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        DiscogsError::Deserialization(_)
    ));
}


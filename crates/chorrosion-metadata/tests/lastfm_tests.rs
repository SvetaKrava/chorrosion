//! Integration tests for the Last.fm API client

use chorrosion_metadata::lastfm::LastFmClient;
use tokio;
// ...existing code...

#[tokio::test]
async fn test_fetch_artist_metadata() {
    // Assume mock server is already running on 127.0.0.1:3030
    let client = LastFmClient::new("test_api_key".to_string(), Some("http://127.0.0.1:3030".to_string()));
    let artist_name = "Test Artist";
    let result = client.fetch_artist_metadata(artist_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.name, "Test Artist");
        // Optionally check other fields if mock server returns them
    }
}

#[tokio::test]
async fn test_fetch_album_metadata() {
    // Assume mock server is already running on 127.0.0.1:3030
    let client = LastFmClient::new_with_limits("test_api_key".to_string(), 5);
    let artist_name = "Test Artist";
    let album_name = "Test Album";
    let result = client.fetch_album_metadata(artist_name, album_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        // Optionally check other fields if mock server returns them
    }
}

#[tokio::test]
async fn test_artist_metadata_with_mock() {
    // Assume mock server is already running on 127.0.0.1:3030
    let client = LastFmClient::new("test_api_key".to_string(), Some("http://127.0.0.1:3030".to_string()));
    let artist_metadata = client.fetch_artist_metadata("Test Artist").await.unwrap();
    assert_eq!(artist_metadata.name, "Test Artist");
    // Optionally check other fields if mock server returns them
}

#[tokio::test]
async fn test_fetch_artist_metadata_with_query_params() {
    // Assume mock server is already running on 127.0.0.1:3030
    let client = LastFmClient::new("test_api_key".to_string(), Some("http://127.0.0.1:3030".to_string()));
    let artist_name = "Test Artist";
    let result = client.fetch_artist_metadata(artist_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.name, "Test Artist");
        // Optionally check other fields if mock server returns them
    }
}

#[tokio::test]
async fn test_fetch_album_metadata_with_query_params() {
    // Assume mock server is already running on 127.0.0.1:3030
    let client = LastFmClient::new("test_api_key".to_string(), Some("http://127.0.0.1:3030".to_string()));
    let artist_name = "Test Artist";
    let album_name = "Test Album";
    let result = client.fetch_album_metadata(artist_name, album_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        // Optionally check other fields if mock server returns them
    }
}
//! Integration tests for the Last.fm API client
//!
//! These tests require the mock server to be running on 127.0.0.1:3030.
//! All integration tests wait for server readiness before executing.

use chorrosion_metadata::lastfm::LastFmClient;

mod test_helpers;

use test_helpers::wait_for_mock_server_ready;

#[tokio::test]
#[ignore = "requires mock server on 127.0.0.1:3030 (run via test-with-mock-server.sh)"]
async fn test_fetch_artist_metadata() {
    // Wait for mock server to be ready
    wait_for_mock_server_ready(
        "http://127.0.0.1:3030/2.0/?method=artist.getinfo&artist=Ready&api_key=test&format=json",
        10,
    )
    .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some("http://127.0.0.1:3030/2.0/".to_string()),
    );
    let artist_name = "Test Artist";
    let result = client.fetch_artist_metadata(artist_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.name, "Test Artist");
        assert_eq!(metadata.bio.as_deref(), Some("Test artist bio"));
        assert_eq!(
            metadata.tags.as_deref(),
            Some(&["rock".to_string(), "indie".to_string()][..])
        );
    }
}

#[tokio::test]
#[ignore = "requires mock server on 127.0.0.1:3030 (run via test-with-mock-server.sh)"]
async fn test_fetch_album_metadata() {
    // Wait for mock server to be ready
    wait_for_mock_server_ready(
        "http://127.0.0.1:3030/2.0/?method=album.getinfo&artist=Test&album=Album&api_key=test&format=json",
        10,
    )
    .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some("http://127.0.0.1:3030/2.0/".to_string()),
    );
    let artist_name = "Test Artist";
    let album_name = "Test Album";
    let result = client.fetch_album_metadata(artist_name, album_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        assert_eq!(
            metadata.tracks.as_deref(),
            Some(
                &["Track 1".to_string(), "Track 2".to_string(), "Track 3".to_string()][..]
            )
        );
    }
}

#[tokio::test]
#[ignore = "requires mock server on 127.0.0.1:3030 (run via test-with-mock-server.sh)"]
async fn test_artist_metadata_with_mock() {
    // Wait for mock server to be ready
    wait_for_mock_server_ready(
        "http://127.0.0.1:3030/2.0/?method=artist.getinfo&artist=Ready&api_key=test&format=json",
        10,
    )
    .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some("http://127.0.0.1:3030/2.0/".to_string()),
    );
    let artist_metadata = client.fetch_artist_metadata("Test Artist").await.unwrap();
    assert_eq!(artist_metadata.name, "Test Artist");
    assert_eq!(artist_metadata.bio.as_deref(), Some("Test artist bio"));
}

#[tokio::test]
#[ignore = "requires mock server on 127.0.0.1:3030 (run via test-with-mock-server.sh)"]
async fn test_fetch_artist_metadata_with_query_params() {
    // Wait for mock server to be ready
    wait_for_mock_server_ready(
        "http://127.0.0.1:3030/2.0/?method=artist.getinfo&artist=Ready&api_key=test&format=json",
        10,
    )
    .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some("http://127.0.0.1:3030/2.0/".to_string()),
    );
    let artist_name = "Test Artist";
    let result = client.fetch_artist_metadata(artist_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.name, "Test Artist");
        assert_eq!(metadata.bio.as_deref(), Some("Test artist bio"));
    }
}

#[tokio::test]
#[ignore = "requires mock server on 127.0.0.1:3030 (run via test-with-mock-server.sh)"]
async fn test_fetch_album_metadata_with_query_params() {
    // Wait for mock server to be ready
    wait_for_mock_server_ready(
        "http://127.0.0.1:3030/2.0/?method=album.getinfo&artist=Test&album=Album&api_key=test&format=json",
        10,
    )
    .await;

    let client = LastFmClient::new(
        "test_api_key".to_string(),
        Some("http://127.0.0.1:3030/2.0/".to_string()),
    );
    let artist_name = "Test Artist";
    let album_name = "Test Album";
    let result = client.fetch_album_metadata(artist_name, album_name).await;
    assert!(result.is_ok());
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        assert_eq!(
            metadata.tracks.as_deref(),
            Some(
                &["Track 1".to_string(), "Track 2".to_string(), "Track 3".to_string()][..]
            )
        );
    }
}
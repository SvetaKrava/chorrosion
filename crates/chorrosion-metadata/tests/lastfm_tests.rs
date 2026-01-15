//! Integration tests for the Last.fm API client

use chorrosion_metadata::lastfm::{LastFmClient, ArtistMetadata, AlbumMetadata};
use tokio;

#[tokio::test]
async fn test_fetch_artist_metadata() {
    let api_key = "test_api_key".to_string();
    let client = LastFmClient::new_with_limits(api_key, 5);

    // Mock artist name
    let artist_name = "Test Artist";

    // Fetch artist metadata
    let result = client.fetch_artist_metadata(artist_name).await;

    // Assert the result is Ok
    assert!(result.is_ok());

    // Assert the metadata fields
    if let Ok(metadata) = result {
        assert_eq!(metadata.name, "Test Artist");
        assert!(metadata.bio.is_some());
        assert!(metadata.tags.is_some());
    }
}

#[tokio::test]
async fn test_fetch_album_metadata() {
    let api_key = "test_api_key".to_string();
    let client = LastFmClient::new_with_limits(api_key, 5);

    // Mock artist and album names
    let artist_name = "Test Artist";
    let album_name = "Test Album";

    // Fetch album metadata
    let result = client.fetch_album_metadata(artist_name, album_name).await;

    // Assert the result is Ok
    assert!(result.is_ok());

    // Assert the metadata fields
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        assert!(metadata.tracks.is_some());
    }
}
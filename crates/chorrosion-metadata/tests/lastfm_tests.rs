//! Integration tests for the Last.fm API client

use chorrosion_metadata::lastfm::LastFmClient;
use tokio;
use rand::Rng;
use std::collections::HashMap;
use warp::Filter; // Ensure the necessary traits for `and` are imported

#[tokio::test]
async fn test_fetch_artist_metadata() {
    let port = 3030 + rand::random::<u16>() % 1000; // Dynamically assign a unique port
    let base_url = format!("http://127.0.0.1:{}", port); // Use the dynamic port in the base URL
    println!("Mock server base URL: {}", base_url);

    // Start a mock server
    let mock_server = warp::serve(
        warp::path("2.0")
            .and(warp::query::<HashMap<String, String>>())
            .map(|query_params: HashMap<String, String>| {
                println!("Mock server received request with params: {:?}", query_params);
                warp::reply::json(&serde_json::json!({
                    "artist": {
                        "name": "Test Artist",
                        "listeners": "12345",
                        "playcount": "67890"
                    }
                }))
            })
    );

    println!("Starting mock server on port {}", port);
    tokio::spawn(mock_server.run(([127, 0, 0, 1], port)));

    // Add a longer delay to ensure the server is fully initialized
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    println!("Mock server initialized. Running test case.");
    let client = LastFmClient::new("test_api_key".to_string(), Some(base_url));

    // Mock artist name
    let artist_name = "Test Artist";

    // Fetch artist metadata
    println!("Sending request to fetch artist metadata.");
    let result = client.fetch_artist_metadata(artist_name).await;

    // Log the result
    println!("Test result: {:?}", result);

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
    let client = LastFmClient::new_with_limits("test_api_key".to_string(), 5);

    // Mock artist and album names
    let artist_name = "Test Artist";
    let album_name = "Test Album";

    // Start a mock server
    let port = 3030 + rand::random::<u16>() % 1000; // Dynamically assign a unique port
    let mock_server = warp::serve(
        warp::path("2.0")
            .and(warp::query::<HashMap<String, String>>())
            .map(|query_params: HashMap<String, String>| {
                println!("Mock server received request with params: {:?}", query_params);
                warp::reply::json(&serde_json::json!({
                    "album": {
                        "title": "Test Album",
                        "artist": "Test Artist",
                        "tracks": ["Track 1", "Track 2"]
                    }
                }))
            })
    );

    println!("Starting mock server on port {}", port);
    tokio::spawn(mock_server.run(([127, 0, 0, 1], port)));

    // Add a longer delay to ensure the server is fully initialized
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // Fetch album metadata
    println!("Sending request to fetch album metadata.");
    let result = client.fetch_album_metadata(artist_name, album_name).await;

    // Log the result
    println!("Test result: {:?}", result);

    // Assert the result is Ok
    assert!(result.is_ok());

    // Assert the metadata fields
    if let Ok(metadata) = result {
        assert_eq!(metadata.title, "Test Album");
        assert_eq!(metadata.artist, "Test Artist");
        assert!(metadata.tracks.is_some());
    }
}

#[tokio::test]
async fn test_artist_metadata_with_mock() {
    let mock_server = warp::serve(
        warp::path("2.0")
            .and(warp::query::<HashMap<String, String>>())
            .map(|query_params: HashMap<String, String>| {
                println!("Mock server received request with params: {:?}", query_params);
                warp::reply::json(&serde_json::json!({
                    "artist": {
                        "name": "Test Artist",
                        "bio": "Test Bio",
                        "tags": ["rock", "pop"]
                    }
                }))
            })
    );

    tokio::spawn(mock_server.run(([127, 0, 0, 1], 3030)));

    // Add a short delay to ensure the server is fully initialized
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let client = LastFmClient::new("test_api_key".to_string(), Some("http://127.0.0.1:3030".to_string()));
    let artist_metadata = client.fetch_artist_metadata("Test Artist").await.unwrap();

    println!("Mock server URI: {}", "http://127.0.0.1:3030");

    assert_eq!(artist_metadata.name, "Test Artist");
    assert!(artist_metadata.bio.is_some());
    assert!(artist_metadata.tags.unwrap_or_default().contains(&"rock".to_string()));
}

#[tokio::test]
async fn test_fetch_artist_metadata_with_query_params() {
    let _api_key = "test_api_key".to_string();

    // Dynamically assign a port
    let port = rand::thread_rng().gen_range(3000..4000);
    let mock_server_url = format!("http://127.0.0.1:{}", port);

    // Start a simplified mock server
    let mock_server = warp::serve(
        warp::path("2.0")
            .and(warp::query::<HashMap<String, String>>())
            .map(|query_params: HashMap<String, String>| {
                println!("Mock server received request with params: {:?}", query_params);

                let response = serde_json::json!({
                    "artist": {
                        "name": "Test Artist",
                        "bio": "Test bio",
                        "tags": ["rock", "pop"]
                    }
                });
                println!("Returning simplified artist metadata response: {:?}", response);
                warp::reply::json(&response)
            })
    );

    println!("Starting mock server on port {}", port);
    tokio::spawn(mock_server.run(([127, 0, 0, 1], port)));

    // Add a longer delay to ensure the server is fully initialized
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    println!("Mock server initialized. Running test case.");
    let client = LastFmClient::new("test_api_key".to_string(), Some(mock_server_url.clone()));

    // Mock artist name
    let artist_name = "Test Artist";

    // Fetch artist metadata with query parameters
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
async fn test_fetch_album_metadata_with_query_params() {
    let _api_key = "test_api_key".to_string();

    // Dynamically assign a port
    let port = rand::thread_rng().gen_range(3000..4000);
    let mock_server_url = format!("http://127.0.0.1:{}", port);

    // Start a simplified mock server
    let mock_server = warp::serve(
        warp::path("2.0")
            .and(warp::query::<HashMap<String, String>>())
            .map(|query_params: HashMap<String, String>| {
                println!("Mock server received request with params: {:?}", query_params);

                let response = serde_json::json!({
                    "album": {
                        "title": "Test Album",
                        "artist": "Test Artist",
                        "tracks": ["Track 1", "Track 2"]
                    }
                });
                println!("Returning simplified album metadata response: {:?}", response);
                warp::reply::json(&response)
            })
    );

    println!("Starting mock server on port {}", port);
    tokio::spawn(mock_server.run(([127, 0, 0, 1], port)));

    // Add a longer delay to ensure the server is fully initialized
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    println!("Mock server initialized. Running test case.");
    let client = LastFmClient::new("test_api_key".to_string(), Some(mock_server_url.clone()));

    // Mock artist and album names
    let artist_name = "Test Artist";
    let album_name = "Test Album";

    // Fetch album metadata with query parameters
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